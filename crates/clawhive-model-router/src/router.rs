use crate::error::ModelError;
use crate::provider::ModelRegistry;
use crate::types::{ChatRequest, ChatResponse, ModelProfile, StreamHandle};

pub struct ModelRouter {
    registry: ModelRegistry,
}

impl ModelRouter {
    #[must_use]
    pub fn new(registry: ModelRegistry) -> Self {
        Self { registry }
    }

    #[must_use]
    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }

    pub async fn route_chat(
        &self,
        profile_id: &str,
        mut request: ChatRequest,
    ) -> Result<ChatResponse, ModelError> {
        let profile = self
            .registry
            .get_profile(profile_id)
            .ok_or_else(|| ModelError::ModelNotAvailable(profile_id.to_string()))?;

        request.model = profile.id.clone();

        let provider = self.registry.get_provider(&profile.provider)?;
        let result = provider.chat(request).await?;

        Ok(result)
    }

    pub async fn route_chat_stream(
        &self,
        profile_id: &str,
        mut request: ChatRequest,
    ) -> Result<StreamHandle, ModelError> {
        let profile = self
            .registry
            .get_profile(profile_id)
            .ok_or_else(|| ModelError::ModelNotAvailable(profile_id.to_string()))?;

        request.model = profile.id.clone();

        let provider = self.registry.get_provider(&profile.provider)?;
        let handle = provider.chat_stream(request).await?;

        Ok(handle)
    }

    /// Inject satu profile model secara dinamis ke registry.
    /// Berguna setelah auto-fetch model dari API provider.
    pub fn inject_profile(&self, profile: ModelProfile) {
        self.registry.inject_profile(profile);
    }

    /// Inject banyak profile model sekaligus ke registry.
    pub fn inject_profiles(&self, profiles: Vec<ModelProfile>) {
        self.registry.inject_profiles(profiles);
    }

    pub async fn route_with_fallback(
        &self,
        preferred_profile: &str,
        fallback_profiles: &[String],
        request: ChatRequest,
    ) -> Result<ChatResponse, ModelError> {
        match self.route_chat(preferred_profile, request.clone()).await {
            Ok(response) => Ok(response),
            Err(e) => {
                tracing::warn!(
                    "preferred profile '{}' failed: {}, trying fallbacks",
                    preferred_profile,
                    e
                );
                for fallback in fallback_profiles {
                    match self.route_chat(fallback, request.clone()).await {
                        Ok(response) => return Ok(response),
                        Err(e2) => {
                            tracing::warn!("fallback '{}' also failed: {}", fallback, e2);
                        }
                    }
                }
                Err(ModelError::AllFallbacksExhausted)
            }
        }
    }

    /// Depth-first fallback routing following the ZeroClaw pattern:
    /// Attempt each candidate in order; no recursion to keep async fn size bounded.
    pub async fn route_with_full_fallback(
        &self,
        primary_profile: &str,
        fallback_models: &[String],
        fallback_aliases: &[String],
        request: ChatRequest,
    ) -> Result<ChatResponse, ModelError> {
        let mut candidates: Vec<String> = Vec::new();

        // Build ordered candidate list (max depth 3)
        candidates.push(primary_profile.to_string());

        // Add fallback models on the same provider
        let primary_provider = self
            .registry
            .get_profile(primary_profile)
            .map(|p| p.provider.clone());
        for fb_model in fallback_models {
            let exists = primary_provider.as_ref().is_some_and(|prov| {
                self.registry.list_profiles().iter().any(|p| {
                    p.provider == *prov && (p.id == *fb_model || p.model_name == *fb_model)
                })
            });
            if exists {
                candidates.push(format!("{}@{}", fb_model, primary_profile));
            }
        }

        // Add fallback aliases (flat, no recursion)
        for fb_alias in fallback_aliases {
            if candidates.len() >= 4 {
                break; // max depth
            }
            candidates.push(fb_alias.clone());
        }

        // Try each candidate
        for candidate in &candidates {
            // Extract the actual profile ID for fallback_models (prefixed with `model@profile`)
            let actual = if let Some(stripped) = candidate.strip_suffix(&format!("@{primary_profile}"))
            {
                stripped
            } else {
                candidate.as_str()
            };

            match self.route_chat(actual, request.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    tracing::warn!("fallback candidate '{}' failed: {}", candidate, e);
                }
            }
        }

        Err(ModelError::AllFallbacksExhausted)
    }

    /// Streaming version — tries candidates in order.
    pub async fn route_stream_with_full_fallback(
        &self,
        primary_profile: &str,
        fallback_models: &[String],
        fallback_aliases: &[String],
        request: ChatRequest,
    ) -> Result<StreamHandle, ModelError> {
        let mut candidates: Vec<String> = Vec::new();
        candidates.push(primary_profile.to_string());

        let primary_provider = self
            .registry
            .get_profile(primary_profile)
            .map(|p| p.provider.clone());
        for fb_model in fallback_models {
            let exists = primary_provider.as_ref().is_some_and(|prov| {
                self.registry.list_profiles().iter().any(|p| {
                    p.provider == *prov && (p.id == *fb_model || p.model_name == *fb_model)
                })
            });
            if exists {
                candidates.push(fb_model.clone());
            }
        }

        for fb_alias in fallback_aliases {
            if candidates.len() >= 4 {
                break;
            }
            candidates.push(fb_alias.clone());
        }

        for candidate in &candidates {
            match self.route_chat_stream(candidate, request.clone()).await {
                Ok(handle) => return Ok(handle),
                Err(e) => {
                    tracing::warn!("fallback stream candidate '{}' failed: {}", candidate, e);
                }
            }
        }

        Err(ModelError::AllFallbacksExhausted)
    }

    #[must_use]
    pub fn find_optimal_profile(
        &self,
        task_type: &str,
        context_tokens: u32,
    ) -> Option<ModelProfile> {
        let candidates = self.registry.find_profiles_by_suitability(task_type);
        candidates
            .into_iter()
            .filter(|p| p.context_window >= context_tokens)
            .min_by(|a, b| {
                let a_cost = a.cost_per_1m_input + a.cost_per_1m_output;
                let b_cost = b.cost_per_1m_input + b.cost_per_1m_output;
                a_cost
                    .partial_cmp(&b_cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}
