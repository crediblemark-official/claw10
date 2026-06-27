use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::ModelError;
use crate::types::{ChatRequest, ChatResponse, ModelProfile};

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn name(&self) -> &str;

    fn supported_models(&self) -> Vec<&str>;

    fn get_profile(&self, model_name: &str) -> Option<ModelProfile>;

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ModelError>;
}

pub struct ModelRegistry {
    providers: HashMap<String, Box<dyn ModelProvider>>,
    profiles: Vec<ModelProfile>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            profiles: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn ModelProvider>) {
        for model in provider.supported_models() {
            if let Some(profile) = provider.get_profile(model) {
                self.profiles.push(profile);
            }
        }
        self.providers.insert(provider.name().to_string(), provider);
    }

    pub fn get_provider(&self, name: &str) -> Result<&Box<dyn ModelProvider>, ModelError> {
        self.providers
            .get(name)
            .ok_or_else(|| ModelError::ProviderNotFound(name.to_string()))
    }

    pub fn get_profile(&self, model_id: &str) -> Option<&ModelProfile> {
        self.profiles.iter().find(|p| p.id == model_id)
    }

    pub fn list_profiles(&self) -> &[ModelProfile] {
        &self.profiles
    }

    pub fn find_profiles_by_suitability(&self, task: &str) -> Vec<&ModelProfile> {
        self.profiles
            .iter()
            .filter(|p| p.suitable_for.iter().any(|s| s == task))
            .collect()
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}
