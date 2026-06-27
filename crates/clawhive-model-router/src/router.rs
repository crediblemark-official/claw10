use crate::error::ModelError;
use crate::provider::ModelRegistry;
use crate::types::{ChatRequest, ChatResponse, ModelProfile};

pub struct ModelRouter {
    registry: ModelRegistry,
}

impl ModelRouter {
    pub fn new(registry: ModelRegistry) -> Self {
        Self { registry }
    }

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

        request.model = profile.model_name.clone();

        let provider = self.registry.get_provider(&profile.provider)?;
        let result = provider.chat(request).await?;

        Ok(result)
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

    pub fn find_optimal_profile(
        &self,
        task_type: &str,
        context_tokens: u32,
    ) -> Option<&ModelProfile> {
        let candidates = self.registry.find_profiles_by_suitability(task_type);
        candidates
            .into_iter()
            .filter(|p| p.context_window >= context_tokens)
            .min_by(|a, b| {
                let a_cost = a.cost_per_1k_input + a.cost_per_1k_output;
                let b_cost = b.cost_per_1k_input + b.cost_per_1k_output;
                a_cost
                    .partial_cmp(&b_cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}
