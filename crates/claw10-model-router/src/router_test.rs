use super::*;
use crate::error::ModelError;
use crate::provider::{ModelProvider, ModelRegistry};
use crate::types::{ChatRequest, ChatResponse, ModelProfile, MessageRole, ModelMessage, FinishReason, UsageInfo};
use async_trait::async_trait;

struct MockProvider {
    name: String,
    supported: Vec<String>,
    should_fail_for: Vec<String>,
}

#[async_trait]
impl ModelProvider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn supported_models(&self) -> Vec<&str> {
        self.supported.iter().map(|s| s.as_str()).collect()
    }

    fn get_profile(&self, model_name: &str) -> Option<ModelProfile> {
        if self.supported.contains(&model_name.to_string()) {
            Some(ModelProfile {
                id: model_name.to_string(),
                provider: self.name.clone(),
                model_name: model_name.to_string(),
                context_window: 4096,
                max_output_tokens: 1024,
                cost_per_1m_input: 0.0,
                cost_per_1m_output: 0.0,
                suitable_for: vec!["general".to_string()],
            })
        } else {
            None
        }
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ModelError> {
        if self.should_fail_for.contains(&request.model) {
            return Err(ModelError::ApiError(format!("Model {} failed intentionally", request.model)));
        }
        Ok(ChatResponse {
            message: ModelMessage {
                role: MessageRole::Assistant,
                content: format!("Hello from {}", request.model),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            finish_reason: FinishReason::Stop,
            usage: UsageInfo {
                prompt_tokens: 10,
                completion_tokens: 10,
                total_tokens: 20,
                cost_usd: 0.0,
            },
            model_used: request.model,
        })
    }
}

#[tokio::test]
async fn test_route_with_fallback_success_first() {
    let mut registry = ModelRegistry::new();
    
    let mock = MockProvider {
        name: "mock-prov".to_string(),
        supported: vec!["model-a".to_string()],
        should_fail_for: vec![],
    };
    registry.register(Box::new(mock));
    
    let router = ModelRouter::new(registry);
    let req = ChatRequest {
        model: "model-a".to_string(),
        messages: vec![],
        max_tokens: None,
        temperature: None,
        tools: None,
        stop: None,
    };
    
    let res = router.route_with_fallback("model-a", &[], req).await.unwrap();
    assert_eq!(res.model_used, "model-a");
    assert_eq!(res.message.content, "Hello from model-a");
}

#[tokio::test]
async fn test_route_with_fallback_manual_fallback() {
    let mut registry = ModelRegistry::new();
    
    let mock = MockProvider {
        name: "mock-prov".to_string(),
        supported: vec!["model-fail".to_string(), "model-fallback".to_string()],
        should_fail_for: vec!["model-fail".to_string()],
    };
    registry.register(Box::new(mock));
    
    let router = ModelRouter::new(registry);
    let req = ChatRequest {
        model: "model-fail".to_string(),
        messages: vec![],
        max_tokens: None,
        temperature: None,
        tools: None,
        stop: None,
    };
    
    let res = router.route_with_fallback("model-fail", &["model-fallback".to_string()], req).await.unwrap();
    assert_eq!(res.model_used, "model-fallback");
    assert_eq!(res.message.content, "Hello from model-fallback");
}

#[tokio::test]
async fn test_route_with_fallback_dynamic_fallback() {
    let mut registry = ModelRegistry::new();
    
    // Register mock provider dengan beberapa model
    let mock = MockProvider {
        name: "mock-prov".to_string(),
        supported: vec![
            "model-fail".to_string(),
            "normal-llama".to_string(),
            "instruct-chat".to_string(),
        ],
        should_fail_for: vec!["model-fail".to_string()],
    };
    registry.register(Box::new(mock));
    
    let router = ModelRouter::new(registry);
    let req = ChatRequest {
        model: "model-fail".to_string(),
        messages: vec![],
        max_tokens: None,
        temperature: None,
        tools: None,
        stop: None,
    };
    
    // Fallback manual kosong, tapi registry punya "normal-llama" dan "instruct-chat".
    // Keduanya harus terdeteksi sebagai dynamic fallback.
    let res = router.route_with_fallback("model-fail", &[], req).await.unwrap();
    
    // "instruct-chat" harus punya prioritas lebih tinggi karena scoring kecocokan kata kunci.
    assert_eq!(res.model_used, "instruct-chat");
    assert_eq!(res.message.content, "Hello from instruct-chat");
}
