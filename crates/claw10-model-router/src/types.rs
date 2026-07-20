use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A content part within a multi-part message (used for vision/image inputs).
/// Mirrors the OpenAI chat completions content part format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlContent },
}

/// URL content for an image in a vision request.
/// The `url` field accepts either a standard HTTPS URL or a data URI
/// (e.g. `data:image/png;base64,iVBORw0KGgo…`).
/// `detail` controls how the model processes the image (`"high"`, `"low"`, `"auto"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrlContent {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: MessageRole,
    pub content: String,
    /// Optional multi-part content (text + images) for vision requests.
    /// When present, `content` is used as the text part description
    /// and this field provides the full multi-part payload.
    pub content_parts: Option<Vec<ContentPart>>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ModelMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub stop: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ModelMessage,
    pub finish_reason: FinishReason,
    pub usage: UsageInfo,
    pub model_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: String,
    pub provider: String,
    pub model_name: String,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub cost_per_1m_input: f64,
    pub cost_per_1m_output: f64,
    pub suitable_for: Vec<String>,
}

/// A single event from a streaming chat response.
/// Mirrors the OpenAI SSE stream format (text deltas, tool calls, usage).
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Incremental text content delta.
    TextDelta(String),
    /// Tool call fragment — accumulated across multiple deltas.
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: String,
    },
    /// Final usage information (sent once at stream end).
    Usage(UsageInfo),
    /// Stream complete.
    Done,
    /// Stream encountered an error.
    Error(String),
}

/// A handle to consume streaming responses via a channel.
/// Clones share the same underlying receiver.
#[derive(Debug, Clone)]
pub struct StreamHandle {
    rx: Arc<std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<StreamEvent>>>>,
}

impl StreamHandle {
    pub(crate) fn new(rx: tokio::sync::mpsc::UnboundedReceiver<StreamEvent>) -> Self {
        Self {
            rx: Arc::new(std::sync::Mutex::new(Some(rx))),
        }
    }

    /// Receive the next stream event. Returns `None` when the stream is exhausted.
    pub async fn recv(&self) -> Option<StreamEvent> {
        let mut rx = self.rx.lock().unwrap().take()?;
        let event = rx.recv().await;
        self.rx.lock().unwrap().replace(rx);
        event
    }

    /// Collect all remaining events into a vector.
    pub async fn collect(&self) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.recv().await {
            match event {
                StreamEvent::Done => break,
                StreamEvent::Error(_) => {
                    events.push(event);
                    break;
                }
                _ => events.push(event),
            }
        }
        events
    }
}

/// A model family (e.g. "Gemini 2.0") containing specific variants (Flash, Pro, …).
#[derive(Debug, Clone)]
pub struct ModelFamily {
    pub name: String,
    pub variants: Vec<ModelProfile>,
}

fn load_priority_models(provider: &str) -> Vec<String> {
    let path = format!("models/{}.json", provider.to_lowercase());
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!("No priority model file at {path}: {e}");
            return Vec::new();
        }
    };
    match serde_json::from_str::<Vec<String>>(&content) {
        Ok(list) => list.into_iter().map(|s| s.to_lowercase()).collect(),
        Err(e) => {
            tracing::warn!("Failed to parse priority models from {path}: {e}");
            Vec::new()
        }
    }
}

/// Group models into families using a heuristic that strips trailing variant/version
/// segments from model IDs. Models that share a common stem are grouped together.
///
/// Stems are derived by removing known variant suffixes (`mini`, `flash`, `pro`, `turbo`,
/// version numbers, date stamps, etc.) and checking what remains.
pub fn group_models_by_family(models: Vec<ModelProfile>) -> Vec<ModelFamily> {
    use std::collections::HashMap;

    let provider_name = models.first().map(|m| m.provider.clone()).unwrap_or_else(|| "nvidia".to_string());
    let priority_list = load_priority_models(&provider_name);

    let mut families: HashMap<String, Vec<ModelProfile>> = HashMap::new();

    for model in models {
        let stem = extract_model_stem(&model.id);
        let label = stem
            .split('/')
            .next_back()
            .unwrap_or(&stem)
            .replace('-', " ")
            .split_whitespace()
            .map(|w| {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().to_string() + c.as_str()).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");

        families.entry(label).or_default().push(model);
    }

    let mut result: Vec<ModelFamily> = families
        .into_iter()
        .map(|(name, variants)| ModelFamily { name, variants })
        .collect();

    // Sort: Berdasarkan skor prioritas dari berkas JSON (descending), lalu alfabetis jika skor sama
    let get_priority_score = |name: &str| -> i32 {
        let name_lower = name.to_lowercase();
        
        // Cari posisi model dari priority list dinamis
        if let Some(pos) = priority_list.iter().position(|m| name_lower.contains(m)) {
            // Score tertinggi didapatkan oleh index terawal:
            // 1000 untuk index 0, dan berkurang 10 di setiap index berikutnya
            return 1000 - (pos as i32 * 10);
        }

        // Keyword umum jika tidak cocok spesifik
        if name_lower.contains("deepseek") { return 100; }
        if name_lower.contains("kimi") { return 90; }
        if name_lower.contains("llama") || name_lower.contains("nemotron") { return 80; }
        if name_lower.contains("mistral") || name_lower.contains("codestral") { return 70; }
        if name_lower.contains("qwen") { return 60; }
        if name_lower.contains("gemma") { return 50; }
        if name_lower.contains("phi") { return 20; }
        0
    };

    result.sort_by(|a, b| {
        let a_score = get_priority_score(&a.name);
        let b_score = get_priority_score(&b.name);
        if a_score != b_score {
            b_score.cmp(&a_score) // Descending score
        } else {
            a.name.cmp(&b.name) // Alfabetis jika score sama
        }
    });
    result
}

/// Remove trailing variant/version segments from a model ID to find its stem.
///
/// Examples:
/// - `gemini-2.0-flash`       → `gemini-2.0`
/// - `gemini-2.0-flash-lite`  → `gemini-2.0`
/// - `gpt-4o-mini`            → `gpt-4o`
/// - `claude-sonnet-4-20250514` → `claude-sonnet-4`
fn extract_model_stem(model_id: &str) -> String {
    let id = model_id.split('/').next_back().unwrap_or(model_id);

    let variant_suffixes = [
        "mini", "micro", "nano", "small", "medium", "large",
        "flash", "pro", "turbo", "lite", "ultra", "max", "plus",
        "sonnet", "haiku", "opus",
        "instruct", "chat", "code", "reasoning", "thinking",
        "preview", "exp", "experimental", "latest", "stable",
        "vision", "audio", "realtime",
    ];

    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() <= 2 {
        return id.to_string();
    }

    // Walk backwards and strip known variant/version segments
    let mut end = parts.len();
    for i in (1..parts.len()).rev() {
        let segment = parts[i];
        // Version number or date
        if segment.chars().all(|c| c.is_ascii_digit() || c == '.') {
            end = i;
            continue;
        }
        // Known variant suffix
        if variant_suffixes.contains(&segment) {
            end = i;
            continue;
        }
        break;
    }

    parts[..end].join("-")
}
