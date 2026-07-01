use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;

use crate::config::ResolvedProvider;
use crate::error::ModelError;
use crate::openai_compat::OpenAiCompatibleProvider;
use crate::types::{ChatRequest, ChatResponse, ModelProfile, StreamEvent, StreamHandle};

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn name(&self) -> &str;

    fn supported_models(&self) -> Vec<&str>;

    fn get_profile(&self, model_name: &str) -> Option<ModelProfile>;

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ModelError>;

    /// Fetch available models from the provider's API.
    /// Default implementation returns an error (not supported).
    async fn fetch_models(&self) -> Result<Vec<ModelProfile>, ModelError> {
        Err(ModelError::Other("model listing not supported by this provider".to_string()))
    }

    /// Stream a chat response via SSE. Returns a handle to consume events incrementally.
    /// Default implementation falls back to non-streaming `chat()`.
    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<StreamHandle, ModelError> {
        // Default: collect full response and emit it as a single event
        let response = self.chat(request).await?;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let content = response.message.content;
        if !content.is_empty() {
            let _ = tx.send(StreamEvent::TextDelta(content));
        }
        let _ = tx.send(StreamEvent::Usage(response.usage));
        let _ = tx.send(StreamEvent::Done);
        Ok(StreamHandle::new(rx))
    }
}

pub struct ModelRegistry {
    providers: HashMap<String, Box<dyn ModelProvider>>,
    /// Daftar semua profile model yang diregistrasi (termasuk yang di-inject dinamis).
    /// Dibungkus RwLock agar bisa dimutasi dari Arc<ModelRegistry>.
    profiles: RwLock<Vec<ModelProfile>>,
}

impl ModelRegistry {
    #[must_use]
    pub fn new() -> Self {
        let mut profiles = Vec::new();
        
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let cache_file = std::path::PathBuf::from(&home).join(".claw10").join("models.json");
        
        if cache_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&cache_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(obj) = json.as_object() {
                        for (provider_name, models_arr) in obj {
                            if let Some(arr) = models_arr.as_array() {
                                for val in arr {
                                    if let Some(model_id) = val.as_str() {
                                        profiles.push(ModelProfile {
                                            id: model_id.to_string(),
                                            provider: provider_name.clone(),
                                            model_name: model_id.to_string(),
                                            context_window: 128_000,
                                            max_output_tokens: 8_192,
                                            cost_per_1m_input: 0.0,
                                            cost_per_1m_output: 0.0,
                                            suitable_for: vec!["general".to_string()],
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Self {
            providers: HashMap::new(),
            profiles: RwLock::new(profiles),
        }
    }

    pub fn register(&mut self, provider: Box<dyn ModelProvider>) {
        {
            let mut profiles = self.profiles.write().expect("profiles RwLock poisoned");
            for model in provider.supported_models() {
                if let Some(profile) = provider.get_profile(model) {
                    profiles.push(profile);
                }
            }
        }
        self.providers.insert(provider.name().to_string(), provider);
    }

    /// Inject satu profile model secara dinamis (misal: dari hasil auto-fetch API).
    /// Idempoten — tidak menambah duplikat berdasarkan `id`.
    pub fn inject_profile(&self, profile: ModelProfile) {
        let mut profiles = self.profiles.write().expect("profiles RwLock poisoned");
        if !profiles.iter().any(|p| p.id == profile.id && p.provider == profile.provider) {
            profiles.push(profile);
        }
    }

    /// Inject banyak profile sekaligus.
    pub fn inject_profiles(&self, new_profiles: Vec<ModelProfile>) {
        let mut profiles = self.profiles.write().expect("profiles RwLock poisoned");
        for profile in new_profiles {
            if !profiles.iter().any(|p| p.id == profile.id && p.provider == profile.provider) {
                profiles.push(profile);
            }
        }
    }

    /// Register a resolved provider descriptor (from config or env-var discovery).
    /// Creates an `OpenAiCompatibleProvider` from the descriptor.
    pub fn register_resolved(&mut self, resolved: ResolvedProvider) {
        let provider = OpenAiCompatibleProvider::with_config(
            &resolved.name,
            &resolved.base_url,
            resolved.api_key,
            resolved.models,
        );
        self.register(Box::new(provider));
    }

    /// Register multiple resolved providers at once.
    pub fn register_resolved_providers(&mut self, providers: Vec<ResolvedProvider>) {
        for p in providers {
            self.register_resolved(p);
        }
    }

    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub fn get_provider(&self, name: &str) -> Result<&dyn ModelProvider, ModelError> {
        if let Some(provider) = self.providers.get(name) {
            return Ok(provider.as_ref());
        }

        let prefix = format!("{}.", name);
        for (k, v) in &self.providers {
            if k.starts_with(&prefix) {
                return Ok(v.as_ref());
            }
        }

        Err(ModelError::ProviderNotFound(name.to_string()))
    }

    /// Cari profile berdasarkan model_id. Mengembalikan clone agar thread-safe.
    #[must_use]
    pub fn get_profile(&self, model_id: &str) -> Option<ModelProfile> {
        self.profiles
            .read()
            .expect("profiles RwLock poisoned")
            .iter()
            .find(|p| p.id == model_id)
            .cloned()
    }

    /// Ambil semua profile (clone untuk thread-safety).
    #[must_use]
    pub fn list_profiles(&self) -> Vec<ModelProfile> {
        self.profiles
            .read()
            .expect("profiles RwLock poisoned")
            .clone()
    }

    #[must_use]
    pub fn find_profiles_by_suitability(&self, task: &str) -> Vec<ModelProfile> {
        self.profiles
            .read()
            .expect("profiles RwLock poisoned")
            .iter()
            .filter(|p| p.suitable_for.iter().any(|s| s == task))
            .cloned()
            .collect()
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}
