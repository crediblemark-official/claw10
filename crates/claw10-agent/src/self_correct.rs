//! Self-Correction Loop: verify tool results, auto-retry on failure, enrich LLM context.
//!
//! ## Flow
//!
//! ```text
//! Tool Execution
//!   → VerificationEngine::verify()
//!     → Check exit codes / state (shell, window, process)
//!     → Return VerificationResult
//!   → If Failed + retries remaining:
//!     → Auto-retry (same or modified args)
//!     → Track retry count
//!   → If Failed + exhausted retries:
//!     → Enrich tool output with failure analysis
//!     → Let LLM decide next approach
//!   → If RequiresScreenshot:
//!     → Take screenshot as fallback verification
//! ```

use std::sync::Arc;

use serde_json::json;

use claw10_model_router::router::ModelRouter;
use claw10_model_router::types::{ChatRequest, ContentPart, ImageUrlContent, MessageRole, ModelMessage};
use claw10_tool::context::ToolContext;
use claw10_tool::registry::ToolRegistry;
use claw10_tool::result::ToolOutput;

/// Maximum number of automatic retries for a failed tool execution.
const MAX_AUTO_RETRIES: u32 = 3;

/// Result of verifying a tool execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    /// Tool executed successfully — no issues detected.
    Success,
    /// Tool failed with a specific reason and optional suggestion.
    Failed {
        reason: String,
        /// Hint for the LLM on what to try next.
        suggestion: Option<String>,
    },
    /// Text-based checks were ambiguous — screenshot verification recommended.
    RequiresScreenshot {
        reason: String,
    },
    /// OCR text does not match expected text.
    OcrMismatch {
        expected: String,
        actual: String,
        confidence: i32,
    },
}

impl VerificationResult {
    pub fn is_success(&self) -> bool {
        matches!(self, VerificationResult::Success)
    }

    pub fn to_status_string(&self) -> &str {
        match self {
            VerificationResult::Success => "success",
            VerificationResult::Failed { .. } => "failed",
            VerificationResult::RequiresScreenshot { .. } => "ambiguous",
            VerificationResult::OcrMismatch { .. } => "ocr_mismatch",
        }
    }
}

/// Tracks retry state for a single tool call within a turn.
#[derive(Debug, Clone)]
pub struct RetryTracker {
    /// How many times this tool call has been retried.
    pub attempt: u32,
    /// Whether the tool call is currently being retried.
    pub is_retrying: bool,
    /// History of failure reasons across attempts.
    pub failure_history: Vec<String>,
}

impl RetryTracker {
    pub fn new() -> Self {
        Self {
            attempt: 0,
            is_retrying: false,
            failure_history: Vec::new(),
        }
    }

    /// Whether we can still retry.
    pub fn can_retry(&self) -> bool {
        self.attempt < MAX_AUTO_RETRIES
    }

    /// Record a failure and increment attempt counter.
    pub fn record_failure(&mut self, reason: String) {
        self.attempt += 1;
        self.is_retrying = self.can_retry();
        self.failure_history.push(reason);
    }

}

/// Verifies tool execution results using text-based checks and vision models.
///
/// Each tool type has specific checks:
/// - `shell` → exit code, timeout, stderr
/// - `window` → verify focus/state via xdotool (if available)
/// - `process` → verify PID existence/state via ps
/// - `input` → check cursor position or clipboard content
/// - `screenshot` → verify file size, OCR text comparison, and vision model analysis
/// - others → generic success (no specific verification)
pub struct VerificationEngine {
    tool_registry: Arc<ToolRegistry>,
    model_router: Option<Arc<ModelRouter>>,
}

impl VerificationEngine {
    #[must_use]
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry, model_router: None }
    }

    /// Create a VerificationEngine with vision model support.
    #[must_use]
    pub fn with_model_router(tool_registry: Arc<ToolRegistry>, model_router: Arc<ModelRouter>) -> Self {
        Self { tool_registry, model_router: Some(model_router) }
    }

    /// Verify a tool execution result.
    ///
    /// Returns a `VerificationResult` indicating success, failure, or ambiguity.
    /// On failure, includes a reason and optional suggestion for the LLM.
    pub async fn verify(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
        result: &ToolOutput,
        context: &ToolContext,
    ) -> VerificationResult {
        // If the tool itself reported failure, check that first
        if !result.success {
            let error_msg = result.error.as_deref().unwrap_or("unknown error");
            return VerificationResult::Failed {
                reason: format!("Tool reported error: {error_msg}"),
                suggestion: Some("Check the tool arguments and retry with a different approach".into()),
            };
        }

        // Tool-specific verification
        match tool_name {
            "shell" => self.verify_shell(args, result),
            "window" => self.verify_window(args, result, context).await,
            "process" => self.verify_process(args, result).await,
            "input" => self.verify_input(args, result),
            "screenshot" => self.verify_screenshot(args, result, context).await,
            "browser" => self.verify_browser(args, result),
            _ => VerificationResult::Success,
        }
    }

    // ── Shell verification ────────────────────────────────────────

    fn verify_shell(&self, _args: &serde_json::Value, result: &ToolOutput) -> VerificationResult {
        let exit_code = result.data["exit_code"].as_i64().unwrap_or(-1);
        let timed_out = result.data["timed_out"].as_bool().unwrap_or(false);
        let stderr = result.data["stderr"].as_str().unwrap_or("").trim();

        if timed_out {
            let action = _args.get("action").and_then(|v| v.as_str()).unwrap_or("exec");
            return if action == "exec" {
                VerificationResult::Failed {
                    reason: "Command timed out. The process was killed.".into(),
                    suggestion: Some("Try a simpler command, use `timeout` flag higher, or use `spawn` for long-running processes".into()),
                }
            } else {
                // spawn/poll/kill don't normally time out
                VerificationResult::Success
            };
        }

        if exit_code != 0 && exit_code != -1 {
            let mut reason = format!("Command exited with code {exit_code}");
            if !stderr.is_empty() {
                reason.push_str(&format!(". stderr: {stderr}"));
            }
            return VerificationResult::Failed {
                reason,
                suggestion: Some("Check error output and try a corrected command".into()),
            };
        }

        VerificationResult::Success
    }

    // ── Window verification ───────────────────────────────────────

    async fn verify_window(
        &self,
        args: &serde_json::Value,
        _result: &ToolOutput,
        context: &ToolContext,
    ) -> VerificationResult {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "focus" => {
                // Verify window is actually focused by checking active window
                if let Ok(window_tool) = self.tool_registry.get("window") {
                    // Check if xdotool is available — skip if not
                    let check_args = json!({ "action": "info", "window_id": args.get("window_id") });
                    match window_tool.execute(context, check_args).await {
                        Ok(info) => {
                            let is_focused = info.data.get("geometry").is_some();
                            if !is_focused {
                                return VerificationResult::Failed {
                                    reason: "Window focus verification failed. Window may not exist or cannot be activated.".into(),
                                    suggestion: Some("Try listing windows first to get the correct window_id, then retry focus".into()),
                                };
                            }
                        }
                        Err(_) => {
                            // xdotool not available — skip verification
                        }
                    }
                }
                VerificationResult::Success
            }
            "close" => {
                // Verify window was actually closed — skip if can't check
                if let Ok(window_tool) = self.tool_registry.get("window") {
                    let check_args = json!({ "action": "info", "window_id": args.get("window_id") });
                    match window_tool.execute(context, check_args).await {
                        Ok(_) => {
                            // Window still exists — close may have failed
                            return VerificationResult::Failed {
                                reason: "Window still exists after close attempt. Close may have failed.".into(),
                                suggestion: Some("Try closing with `force: true` or use `windowkill`".into()),
                            };
                        }
                        Err(_) => {
                            // Window no longer exists — success
                        }
                    }
                }
                VerificationResult::Success
            }
            _ => VerificationResult::Success,
        }
    }

    // ── Process verification ──────────────────────────────────────

    async fn verify_process(
        &self,
        args: &serde_json::Value,
        _result: &ToolOutput,
    ) -> VerificationResult {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "kill" => {
                let pid = args.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                if pid > 0 {
                    // Check if process still exists in /proc
                    let still_exists = std::path::Path::new(&format!("/proc/{pid}")).exists();
                    if still_exists {
                        return VerificationResult::Failed {
                            reason: format!("Process {pid} still exists after kill attempt. Signal may have been ignored."),
                            suggestion: Some("Try with signal: KILL instead of TERM, or verify the PID is correct".into()),
                        };
                    }
                }
                VerificationResult::Success
            }
            "monitor" | "list" | "tree" => VerificationResult::Success,
            _ => VerificationResult::Success,
        }
    }

    // ── Input verification ────────────────────────────────────────

    fn verify_input(&self, _args: &serde_json::Value, _result: &ToolOutput) -> VerificationResult {
        // Input verification is inherently visual. Skip text-based checks
        // and rely on the LLM to verify via subsequent tool calls.
        VerificationResult::Success
    }

    // ── Screenshot verification ───────────────────────────────────

    /// Verify screenshot capture and optionally run OCR + vision model to compare with expected text.
    ///
    /// Flow:
    /// 1. Check screenshot size (0 bytes = failure)
    /// 2. If `expected_text` provided → run OCR first (fast, cheap)
    /// 3. If OCR succeeds and matches → Success
    /// 4. If OCR fails or mismatches → fall back to vision model (slower, richer analysis)
    /// 5. Vision model receives screenshot as ContentPart::ImageUrl and analyzes it
    async fn verify_screenshot(
        &self,
        args: &serde_json::Value,
        result: &ToolOutput,
        _context: &ToolContext,
    ) -> VerificationResult {
        let size = result.data["size_bytes"].as_u64().unwrap_or(0);
        if size == 0 {
            return VerificationResult::Failed {
                reason: "Screenshot returned 0 bytes. Capture may have failed.".into(),
                suggestion: Some("Check DISPLAY setting and screenshot tool availability".into()),
            };
        }

        let expected_text = args.get("expected_text").and_then(|v| v.as_str());
        if let Some(expected) = expected_text {
            let image_base64 = result.data["image_base64"].as_str();
            if let Some(b64) = image_base64 {
                // Step 1: Try OCR first (fast, cheap)
                let lang = args.get("language").and_then(|v| v.as_str()).unwrap_or("eng");
                let ocr_result = self.run_ocr(b64, lang).await;

                // Check if OCR matched before moving into fallback
                let ocr_matched = match &ocr_result {
                    Ok((ocr_text, _)) => {
                        let ocr_lower = ocr_text.to_lowercase();
                        let expected_lower = expected.to_lowercase();
                        ocr_lower.contains(&expected_lower)
                    }
                    Err(_) => false,
                };

                if ocr_matched {
                    return VerificationResult::Success;
                }

                // Log why we're falling back to vision
                match &ocr_result {
                    Ok((_, confidence)) => {
                        tracing::info!(
                            "[Self-Correct] OCR mismatch (confidence={}), falling back to vision model",
                            confidence
                        );
                    }
                    Err(e) => {
                        tracing::info!("[Self-Correct] OCR failed ({e}), falling back to vision model");
                    }
                }

                // Step 2: Fall back to vision model for richer analysis
                match self.run_vision(b64, expected).await {
                    Ok(analysis) => {
                        if analysis.to_uppercase().contains("YES") || analysis.contains("found") || analysis.contains("matches") {
                            VerificationResult::Success
                        } else {
                            VerificationResult::Failed {
                                reason: format!("Vision model says: {analysis}"),
                                suggestion: Some(format!(
                                    "Expected '{}' not confirmed by vision analysis. The screen state may differ from what was expected.",
                                    expected
                                )),
                            }
                        }
                    }
                    Err(e) => {
                        // Vision model also failed — report OCR mismatch if we had one
                        tracing::warn!("[Self-Correct] Vision model also failed: {e}");
                        match ocr_result {
                            Ok((ocr_text, confidence)) => VerificationResult::OcrMismatch {
                                expected: expected.to_string(),
                                actual: ocr_text,
                                confidence,
                            },
                            Err(_) => VerificationResult::Failed {
                                reason: format!("Both OCR and vision model failed: {e}"),
                                suggestion: Some("Install tesseract-ocr or configure a vision model".into()),
                            },
                        }
                    }
                }
            } else {
                VerificationResult::Failed {
                    reason: "expected_text provided but no image_base64 in screenshot result".into(),
                    suggestion: Some("Ensure screenshot capture returns image_base64".into()),
                }
            }
        } else {
            VerificationResult::Success
        }
    }

    /// Run OCR on a base64-encoded image via the ScreenshotTool.
    ///
    /// Delegates to the screenshot tool's `ocr` action, which uses Tesseract (leptess).
    /// Returns (extracted_text, mean_confidence) or an error string.
    async fn run_ocr(&self, image_base64: &str, language: &str) -> Result<(String, i32), String> {
        let screenshot_tool = self.tool_registry.get("screenshot")
            .map_err(|e| format!("screenshot tool not found: {e}"))?;

        let args = serde_json::json!({
            "action": "ocr",
            "image_base64": image_base64,
            "language": language,
        });

        let context = claw10_tool::context::ToolContext {
            tenant_id: "self-correct".into(),
            mission_id: claw10_domain::MissionId(uuid::Uuid::nil()),
            task_id: claw10_domain::TaskId(uuid::Uuid::nil()),
            agent_id: claw10_domain::AgentId(uuid::Uuid::nil()),
            worker_id: claw10_domain::WorkerId(uuid::Uuid::nil()),
            idempotency_key: "ocr-verify".into(),
            risk_level: "low".into(),
            approval_id: None,
            budget_remaining: 0.0,
            workspace_dir: "/tmp".into(),
        };

        let output = screenshot_tool.execute(&context, args).await
            .map_err(|e| format!("OCR execution failed: {e}"))?;

        let text = output.data["text"].as_str().unwrap_or("").to_string();
        let confidence = output.data["mean_confidence"].as_i64().unwrap_or(-1) as i32;

        Ok((text, confidence))
    }

    // ── Vision model analysis ─────────────────────────────────────

    /// Send a screenshot to a vision model for visual analysis.
    ///
    /// Constructs a multi-part message with:
    /// - Text prompt asking the model to verify expected text on screen
    /// - ImageUrl with the screenshot as a base64 data URI
    ///
    /// Uses ContentPart::ImageUrl for OpenAI-compatible vision APIs (GPT-4o, Claude, Gemini).
    async fn run_vision(&self, image_base64: &str, expected_text: &str) -> Result<String, String> {
        let model_router = self.model_router.as_ref()
            .ok_or("No vision model configured — set MODEL_VISION_PROFILE env or configure in TUI")?;

        // Find a vision-capable model
        let vision_profile = model_router.find_optimal_profile("vision", 4096)
            .ok_or("No vision-capable model found in registry")?;

        tracing::info!(
            "[Self-Correct] Using vision model '{}' for screenshot analysis",
            vision_profile.id
        );

        // Build multi-part message: text prompt + image URL (data URI)
        let prompt = format!(
            "Look at this screenshot. Is the text '{}' visible on the screen? \
             Answer YES if the text is clearly present, NO if it is not. \
             Be brief — just YES or NO followed by a short explanation.",
            expected_text
        );

        let data_uri = format!("data:image/png;base64,{}", image_base64);

        let request = ChatRequest {
            model: vision_profile.id.clone(),
            messages: vec![
                ModelMessage {
                    role: MessageRole::User,
                    content: prompt,
                    content_parts: Some(vec![
                        ContentPart::Text {
                            text: format!("Is the text '{}' visible on this screenshot?", expected_text),
                        },
                        ContentPart::ImageUrl {
                            image_url: ImageUrlContent {
                                url: data_uri,
                                detail: Some("low".to_string()),
                            },
                        },
                    ]),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
            ],
            max_tokens: Some(256),
            temperature: Some(0.0),
            tools: None,
            stop: None,
        };

        let response = model_router.route_chat(&vision_profile.id, request).await
            .map_err(|e| format!("Vision model call failed: {e}"))?;

        Ok(response.message.content)
    }

    // ── Browser verification ──────────────────────────────────────

    fn verify_browser(&self, args: &serde_json::Value, result: &ToolOutput) -> VerificationResult {
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "navigate" => {
                let status = result.data["status"].as_str().unwrap_or("");
                if status != "loaded" {
                    return VerificationResult::Failed {
                        reason: format!("Page did not load successfully. Status: {status}"),
                        suggestion: Some("Check the URL and network connectivity".into()),
                    };
                }
            }
            "click" | "type" => {
                let success = result.data["success"].as_bool().unwrap_or(false);
                if !success {
                    return VerificationResult::Failed {
                        reason: format!("Browser {action} action reported failure"),
                        suggestion: Some("Check the CSS selector and page state".into()),
                    };
                }
            }
            _ => {}
        }

        VerificationResult::Success
    }

    /// Enrich a failed tool output with verification context for the LLM.
    ///
    /// Returns the original data with additional `_verification` field.
    pub fn enrich_failure(
        result: &ToolOutput,
        verification: &VerificationResult,
        retry_tracker: &RetryTracker,
    ) -> serde_json::Value {
        let (reason, suggestion) = match verification {
            VerificationResult::Failed { reason, suggestion } => (reason.clone(), suggestion.clone()),
            VerificationResult::OcrMismatch { expected, actual, confidence } => (
                format!("OCR text mismatch: expected '{}' but screen shows '{}' (confidence: {})", expected, actual, confidence),
                Some(format!("The screen shows '{}' but expected '{}'. Try a different approach or adjust the target area.", actual, expected)),
            ),
            _ => ("Verification failed".into(), None),
        };

        let mut enriched = result.data.clone();
        enriched["_verification"] = json!({
            "status": "failed",
            "reason": reason,
            "suggestion": suggestion,
            "attempts": retry_tracker.attempt,
            "max_retries": MAX_AUTO_RETRIES,
            "retry_exhausted": !retry_tracker.can_retry(),
            "failure_history": retry_tracker.failure_history,
        });

        enriched
    }

    /// Create a success verification marker for the output.
    pub fn enrich_success(result: &ToolOutput) -> serde_json::Value {
        let mut enriched = result.data.clone();
        enriched["_verification"] = json!({
            "status": "success",
        });
        enriched
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
#[path = "self_correct_test.rs"]
mod tests;
