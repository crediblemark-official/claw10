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

    /// Mulai background task untuk meng-autofetch model dari provider terdaftar secara asinkron.
    pub fn start_auto_fetch(self: &std::sync::Arc<Self>) {
        let router_clone = std::sync::Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            let providers = router_clone.registry().list_providers();
            for provider_name in providers {
                if let Ok(provider) = router_clone.registry().get_provider(&provider_name) {
                    tracing::info!("Auto-fetching models untuk provider '{}'...", provider_name);
                    match provider.fetch_models().await {
                        Ok(fetched_models) => {
                            tracing::info!(
                                "Berhasil auto-fetch {} model untuk provider '{}'",
                                fetched_models.len(),
                                provider_name
                            );
                            save_models_to_cache(&provider_name, &fetched_models);
                            router_clone.registry().inject_profiles(fetched_models);
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Auto-fetch model untuk provider '{}' gagal atau tidak didukung: {}",
                                provider_name,
                                e
                            );
                        }
                    }
                }
            }
        });
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
        // We must clone the request for the preferred try if fallbacks exist.
        // It's tricky to satisfy the borrow checker to move it conditionally,
        // so we'll just clone it here. The main optimization is in the fallback loop.
        match self.route_chat(preferred_profile, request.clone()).await {
            Ok(response) => Ok(response),
            Err(e) => {
                tracing::warn!(
                    "preferred profile '{}' failed: {}, trying fallbacks",
                    preferred_profile,
                    e
                );

                if let Some((last_fallback, rest)) = fallback_profiles.split_last() {
                    for fallback in rest {
                        match self.route_chat(fallback, request.clone()).await {
                            Ok(response) => return Ok(response),
                            Err(e2) => {
                                tracing::warn!("fallback '{}' also failed: {}", fallback, e2);
                            }
                        }
                    }
                    match self.route_chat(last_fallback, request.clone()).await {
                        Ok(response) => return Ok(response),
                        Err(e2) => {
                            tracing::warn!("fallback '{}' also failed: {}", last_fallback, e2);
                        }
                    }
                }

                // Coba dynamic fallbacks jika ada
                let dynamic_fallbacks = self.get_dynamic_fallbacks(preferred_profile, fallback_profiles);
                if !dynamic_fallbacks.is_empty() {
                    tracing::info!("mencoba dynamic fallback karena preferred dan manual fallbacks gagal/kosong");
                    if let Some((last_dyn, rest_dyn)) = dynamic_fallbacks.split_last() {
                        for fallback in rest_dyn {
                            match self.route_chat(fallback, request.clone()).await {
                                Ok(response) => return Ok(response),
                                Err(e2) => {
                                    tracing::warn!("dynamic fallback '{}' also failed: {}", fallback, e2);
                                }
                            }
                        }
                        match self.route_chat(last_dyn, request).await {
                            Ok(response) => return Ok(response),
                            Err(e2) => {
                                tracing::warn!("dynamic fallback '{}' also failed: {}", last_dyn, e2);
                            }
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
        if let Some((last_candidate, rest)) = candidates.split_last() {
            for candidate in rest {
                // Extract the actual profile ID for fallback_models (prefixed with `model@profile`)
                let actual = if let Some(stripped) = candidate.strip_suffix(&format!("@{primary_profile}")) {
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

            let actual = if let Some(stripped) = last_candidate.strip_suffix(&format!("@{primary_profile}")) {
                stripped
            } else {
                last_candidate.as_str()
            };

            match self.route_chat(actual, request).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    tracing::warn!("fallback candidate '{}' failed: {}", last_candidate, e);
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

        if let Some((last_candidate, rest)) = candidates.split_last() {
            for candidate in rest {
                match self.route_chat_stream(candidate, request.clone()).await {
                    Ok(handle) => return Ok(handle),
                    Err(e) => {
                        tracing::warn!("fallback stream candidate '{}' failed: {}", candidate, e);
                    }
                }
            }

            match self.route_chat_stream(last_candidate, request).await {
                Ok(handle) => return Ok(handle),
                Err(e) => {
                    tracing::warn!("fallback stream candidate '{}' failed: {}", last_candidate, e);
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

    /// Dapatkan daftar model fallback dinamis secara cerdas dari model registry
    /// jika list fallback eksplisit kosong atau semuanya gagal.
    fn get_dynamic_fallbacks(&self, preferred_profile: &str, fallback_profiles: &[String]) -> Vec<String> {
        let mut candidates = Vec::new();
        let preferred_provider = self.registry.get_profile(preferred_profile).map(|p| p.provider.clone());
        let all_profiles = self.registry.list_profiles();

        // Kumpulkan semua profil yang unik dan bukan preferred atau fallback yang sudah ada
        let mut available_profiles = Vec::new();
        for p in all_profiles {
            if p.id != preferred_profile && !fallback_profiles.contains(&p.id) {
                available_profiles.push(p);
            }
        }

        // Bagi menjadi kategori same provider dan other providers
        let mut same_provider = Vec::new();
        let mut other_providers = Vec::new();

        for p in available_profiles {
            if preferred_provider.as_ref() == Some(&p.provider) {
                same_provider.push(p);
            } else {
                other_providers.push(p);
            }
        }

        // Fungsi scoring sederhana untuk memprioritaskan model chat/instruct/popular
        let score_model = |id: &str| -> i32 {
            let id_lower = id.to_lowercase();
            let mut score = 0;
            if id_lower.contains("instruct") || id_lower.contains("chat") || id_lower.contains("it") {
                score += 10;
            }
            if id_lower.contains("nemotron") || id_lower.contains("llama") || id_lower.contains("qwen") || id_lower.contains("gpt") || id_lower.contains("gemma") {
                score += 5;
            }
            if id_lower.contains("free") {
                // Model free mungkin rate-limited, tapi masih sangat bagus dicoba
                score += 1;
            }
            score
        };

        same_provider.sort_by_key(|p| -score_model(&p.id));
        other_providers.sort_by_key(|p| -score_model(&p.id));

        // Ambil maksimal 3 model dari provider yang sama
        for p in same_provider.iter().take(3) {
            candidates.push(p.id.clone());
        }
        // Ambil maksimal 3 model dari provider berbeda
        for p in other_providers.iter().take(3) {
            candidates.push(p.id.clone());
        }

        candidates
    }
}

fn save_models_to_cache(provider_name: &str, models: &[ModelProfile]) {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let cache_dir = std::path::PathBuf::from(&home).join(".claw10");
    let cache_file = cache_dir.join("models.json");

    let _ = std::fs::create_dir_all(&cache_dir);

    let mut cache_data: serde_json::Value = if cache_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        }
    } else {
        serde_json::json!({})
    };

    let model_names: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
    let clean_provider = provider_name.split('.').next().unwrap_or(provider_name);

    if let Some(obj) = cache_data.as_object_mut() {
        obj.insert(clean_provider.to_string(), serde_json::json!(model_names));
    }

    if let Ok(serialized) = serde_json::to_string_pretty(&cache_data) {
        let _ = std::fs::write(&cache_file, serialized);
    }
}

#[cfg(test)]
#[path = "router_test.rs"]
mod tests;
