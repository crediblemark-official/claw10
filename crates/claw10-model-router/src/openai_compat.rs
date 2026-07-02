use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::provider::ModelProvider;
use crate::types::{
    ChatRequest, ChatResponse, FinishReason, MessageRole, ModelMessage, ModelProfile, StreamEvent,
    StreamHandle, UsageInfo,
};

/// Response from GET /v1/models (OpenAI-compatible).
#[derive(Deserialize)]
struct ModelsListResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    #[allow(dead_code)]
    object: Option<String>,
    #[allow(dead_code)]
    created: Option<i64>,
    #[allow(dead_code)]
    owned_by: Option<String>,
}

/// Generic provider for any OpenAI-compatible chat completions API.
///
/// Supports providers like OpenRouter, Together, DeepSeek, Groq, Fireworks,
/// Alibaba, Moonshot, Mistral, xAI, and many more.
pub struct OpenAiCompatibleProvider {
    name: String,
    base_url: String,
    api_key: String,
    models: Vec<ModelProfile>,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct CompatTool {
    r#type: String,
    function: CompatFunction,
}

#[derive(Serialize)]
struct CompatFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
struct CompatToolCall {
    id: String,
    r#type: String,
    function: CompatFunctionResp,
}

#[derive(Deserialize, Clone)]
struct CompatFunctionResp {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct CompatRequest {
    model: String,
    messages: Vec<CompatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<CompatTool>>,
}

#[derive(Serialize)]
struct CompatMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<CompatToolCallReq>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Serialize)]
struct CompatToolCallReq {
    id: String,
    r#type: String,
    function: CompatFunctionReq,
}

#[derive(Serialize)]
struct CompatFunctionReq {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct CompatResponse {
    #[allow(dead_code)]
    id: Option<String>,
    model: Option<String>,
    choices: Vec<CompatChoice>,
    usage: Option<CompatUsage>,
}

#[derive(Deserialize)]
struct CompatChoice {
    message: Option<CompatMessageResp>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct CompatMessageResp {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<CompatToolCall>>,
}

#[derive(Deserialize)]
struct CompatUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

// ── SSE streaming types ──────────────────────────────

/// A single chunk in the OpenAI SSE stream.
#[derive(Deserialize)]
struct StreamChunk {
    #[allow(dead_code)]
    id: Option<String>,
    #[allow(dead_code)]
    object: Option<String>,
    #[allow(dead_code)]
    created: Option<i64>,
    #[allow(dead_code)]
    model: Option<String>,
    choices: Vec<StreamChoice>,
    #[allow(dead_code)]
    usage: Option<CompatUsage>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[allow(dead_code)]
    index: Option<usize>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamDelta {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<String>,
    #[allow(dead_code)]
    tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Deserialize)]
struct StreamToolCall {
    index: Option<usize>,
    id: Option<String>,
    #[allow(dead_code)]
    r#type: Option<String>,
    function: Option<StreamFunction>,
}

#[derive(Deserialize)]
struct StreamFunction {
    name: Option<String>,
    arguments: Option<String>,
}

impl OpenAiCompatibleProvider {
    #[must_use]
    pub fn new(name: &str, base_url: &str, api_key_env: &str) -> Self {
        let api_key = std::env::var(api_key_env).unwrap_or_default();
        Self::with_config(name, base_url, api_key, Vec::new())
    }

    #[must_use]
    pub fn with_config(
        name: &str,
        base_url: &str,
        api_key: String,
        models: Vec<ModelProfile>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("reqwest client should build");
        Self {
            name: name.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            models,
            client,
        }
    }

    #[must_use]
    pub fn with_models(mut self, models: Vec<ModelProfile>) -> Self {
        self.models = models;
        self
    }

    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn map_finish_reason(fr: &str) -> FinishReason {
        match fr {
            "stop" => FinishReason::Stop,
            "length" => FinishReason::Length,
            "tool_calls" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            _ => FinishReason::Error,
        }
    }

    fn map_role(role: &str) -> MessageRole {
        match role {
            "system" => MessageRole::System,
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => MessageRole::User,
        }
    }

    fn build_compat_request(&self, request: &ChatRequest) -> CompatRequest {
        let messages: Vec<CompatMessage> = request
            .messages
            .iter()
            .map(|m| {
                let tool_calls = m.tool_calls.as_ref().map(|calls| {
                    calls
                        .iter()
                        .map(|tc| CompatToolCallReq {
                            id: tc.id.clone(),
                            r#type: "function".to_string(),
                            function: CompatFunctionReq {
                                name: tc.name.clone(),
                                arguments: serde_json::to_string(&tc.arguments).unwrap_or_default(),
                            },
                        })
                        .collect()
                });

                CompatMessage {
                    role: match m.role {
                        MessageRole::System => "system",
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        MessageRole::Tool => "tool",
                    }
                    .to_string(),
                    content: if m.content.is_empty() && tool_calls.is_some() {
                        None
                    } else {
                        Some(m.content.clone())
                    },
                    tool_calls,
                    tool_call_id: m.tool_call_id.clone(),
                    name: m.name.clone(),
                }
            })
            .collect();

        let tools = request.tools.as_ref().map(|list| {
            list.iter()
                .map(|t| CompatTool {
                    r#type: "function".to_string(),
                    function: CompatFunction {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.input_schema.clone(),
                    },
                })
                .collect()
        });

        CompatRequest {
            model: request.model.clone(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: None,
            stream: None,
            tools,
        }
    }

    /// Estimate cost from usage + model profile.
    fn estimate_cost(&self, model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
        self.models
            .iter()
            .find(|m| m.id == model || m.model_name == model)
            .map(|m| {
                (prompt_tokens as f64 / 1_000_000.0 * m.cost_per_1m_input)
                    + (completion_tokens as f64 / 1_000_000.0 * m.cost_per_1m_output)
            })
            .unwrap_or(0.0)
    }

    /// Common HTTP POST for chat completions (used by both streaming and non-streaming).
    async fn post_chat(
        &self,
        compat_req: &CompatRequest,
    ) -> Result<reqwest::Response, ModelError> {
        let url = self.chat_url();
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "application/json")
            .json(compat_req)
            .send()
            .await
            .map_err(|e| ModelError::ApiError(format!("request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            return if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                Err(ModelError::RateLimited(30))
            } else if status == reqwest::StatusCode::UNAUTHORIZED {
                Err(ModelError::ApiError(format!(
                    "{} authentication failed (401): {body}",
                    self.name
                )))
            } else {
                Err(ModelError::ApiError(format!(
                    "{} API error ({}): {body}",
                    self.name,
                    status.as_u16()
                )))
            };
        }

        Ok(response)
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn supported_models(&self) -> Vec<&str> {
        self.models.iter().map(|m| m.id.as_str()).collect()
    }

    fn get_profile(&self, model_name: &str) -> Option<ModelProfile> {
        self.models
            .iter()
            .find(|m| m.id == model_name || m.model_name == model_name)
            .cloned()
    }

    async fn fetch_models(&self) -> Result<Vec<ModelProfile>, ModelError> {
        let url = format!("{}/models", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ModelError::ApiError(format!("fetch models failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ModelError::ApiError(format!(
                "{} models API returned HTTP {}",
                self.name,
                status.as_u16()
            )));
        }

        let list: ModelsListResponse = response
            .json()
            .await
            .map_err(|e| ModelError::ApiError(format!("failed to parse model list: {e}")))?;

        // Merge with known profiles so we keep metadata (context window, cost, etc.)
        let known: std::collections::HashMap<&str, &ModelProfile> = self
            .models
            .iter()
            .map(|m| (m.id.as_str(), m))
            .collect();

        let fetched: Vec<ModelProfile> = list
            .data
            .into_iter()
            .map(|entry| {
                if let Some(known_profile) = known.get(entry.id.as_str()) {
                    (*known_profile).clone()
                } else {
                    ModelProfile {
                        id: entry.id.clone(),
                        provider: self.name.clone(),
                        model_name: entry.id,
                        context_window: 128_000,
                        max_output_tokens: 8_192,
                        cost_per_1m_input: 0.0,
                        cost_per_1m_output: 0.0,
                        suitable_for: vec!["general".to_string()],
                    }
                }
            })
            .collect();

        Ok(fetched)
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ModelError> {
        if self.api_key.is_empty() {
            return Err(ModelError::ApiError(format!(
                "API key not set for provider '{}'. Set the corresponding env var or configure via TUI",
                self.name
            )));
        }

        let mut compat_req = self.build_compat_request(&request);
        compat_req.stream = Some(false);

        let response = self.post_chat(&compat_req).await?;
        let compat_resp: CompatResponse = response
            .json()
            .await
            .map_err(|e| ModelError::ApiError(format!("failed to parse response: {e}")))?;

        let choice = compat_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ModelError::ApiError("no choices in response".to_string()))?;

        let msg = choice.message.unwrap_or(CompatMessageResp {
            role: Some("assistant".to_string()),
            content: None,
            tool_calls: None,
        });

        let content = msg.content.unwrap_or_default();
        let role = msg
            .role
            .as_deref()
            .map(Self::map_role)
            .unwrap_or(MessageRole::Assistant);

        let tool_calls = msg.tool_calls.as_ref().map(|calls| {
            calls.iter().map(|tc| {
                let args_parsed = serde_json::from_str::<serde_json::Value>(&tc.function.arguments)
                    .unwrap_or_else(|_| serde_json::Value::String(tc.function.arguments.clone()));
                crate::types::ToolCall {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    arguments: args_parsed,
                }
            }).collect()
        });

        let finish_reason = choice
            .finish_reason
            .as_deref()
            .map(Self::map_finish_reason)
            .unwrap_or(FinishReason::Stop);

        let usage = compat_resp.usage.unwrap_or(CompatUsage {
            prompt_tokens: Some(0),
            completion_tokens: Some(0),
            total_tokens: Some(0),
        });

        let prompt_tokens = usage.prompt_tokens.unwrap_or(0);
        let completion_tokens = usage.completion_tokens.unwrap_or(0);
        let cost = self.estimate_cost(&request.model, prompt_tokens, completion_tokens);

        Ok(ChatResponse {
            message: ModelMessage {
                role,
                content,
                tool_calls,
                tool_call_id: None,
                name: None,
            },
            finish_reason,
            usage: UsageInfo {
                prompt_tokens,
                completion_tokens,
                total_tokens: usage.total_tokens.unwrap_or(prompt_tokens + completion_tokens),
                cost_usd: cost,
            },
            model_used: compat_resp.model.unwrap_or(request.model),
        })
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<StreamHandle, ModelError> {
        if self.api_key.is_empty() {
            return Err(ModelError::ApiError(format!(
                "API key not set for provider '{}'",
                self.name
            )));
        }

        let mut compat_req = self.build_compat_request(&request);
        compat_req.stream = Some(true);

        let response = self.post_chat(&compat_req).await?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Spawn a task to read the SSE stream
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;

            let body = response.bytes_stream();
            let mut reader =
                tokio_util::io::StreamReader::new(body.map(|r| {
                    r.map_err(std::io::Error::other)
                }));
            let mut line_buf = String::new();

            loop {
                line_buf.clear();
                match reader.read_line(&mut line_buf).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line_buf.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        // Parse SSE line
                        if let Some(data) = trimmed.strip_prefix("data: ") {
                            if data.trim() == "[DONE]" {
                                let _ = tx.send(StreamEvent::Done);
                                break;
                            }
                            match serde_json::from_str::<StreamChunk>(data) {
                                Ok(chunk) => {
                                    // map_finish_reason but we only need to detect done
                                    let has_finish = chunk.choices.iter().any(|c| {
                                        c.finish_reason
                                            .as_deref()
                                            .is_some_and(|fr| !fr.is_empty() && fr != "null")
                                    });
                                    for choice in &chunk.choices {
                                        if let Some(ref content) = choice.delta.content
                                            && !content.is_empty() {
                                                let _ = tx.send(StreamEvent::TextDelta(
                                                    content.clone(),
                                                ));
                                            }
                                        if let Some(ref tcs) = choice.delta.tool_calls {
                                            for tc in tcs {
                                                let idx = tc.index.unwrap_or(0);
                                                let args = tc
                                                    .function
                                                    .as_ref()
                                                    .and_then(|f| f.arguments.clone())
                                                    .unwrap_or_default();
                                                let _ = tx.send(StreamEvent::ToolCallDelta {
                                                    index: idx,
                                                    id: tc.id.clone(),
                                                    name: tc.function
                                                        .as_ref()
                                                        .and_then(|f| f.name.clone()),
                                                    arguments: args,
                                                });
                                            }
                                        }
                                    }
                                    if has_finish {
                                        // Parse usage from final chunk
                                        if let Some(ref usage) = chunk.usage {
                                            let p = usage.prompt_tokens.unwrap_or(0);
                                            let c = usage.completion_tokens.unwrap_or(0);
                                            let t = usage.total_tokens.unwrap_or(p + c);
                                            let _ = tx.send(StreamEvent::Usage(UsageInfo {
                                                prompt_tokens: p,
                                                completion_tokens: c,
                                                total_tokens: t,
                                                cost_usd: 0.0, // cost not estimable without model lookup here
                                            }));
                                        }
                                        let _ = tx.send(StreamEvent::Done);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(StreamEvent::Error(format!(
                                        "failed to parse stream chunk: {e}"
                                    )));
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        if err_msg.contains("decoding response body")
                            || err_msg.contains("connection closed")
                            || err_msg.contains("aborted")
                            || err_msg.contains("premature")
                        {
                            let _ = tx.send(StreamEvent::Done);
                        } else {
                            let _ = tx.send(StreamEvent::Error(format!(
                                "stream read error: {err_msg}"
                            )));
                        }
                        break;
                    }
                }
            }
        });

        Ok(StreamHandle::new(rx))
    }
}
