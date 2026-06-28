use std::sync::Arc;

use clawhive_model_router::types::{group_models_by_family, ModelFamily, ModelProfile};
use clawhive_store::StoreExt;
use crate::app::{CommandMode, ModelSelectionStep, TuiApp};

impl TuiApp {
    /// Scan config file, env vars, and KV store for every known provider.
    /// Supports the ZeroClaw-style alias system from `clawhive.toml`.
    pub(crate) async fn register_all_providers(&mut self) {
        use clawhive_model_router::config::{discover_config, resolve_providers};
        use clawhive_model_router::provider::ModelRegistry;
        use clawhive_store::StoreExt;

        let mut registry = ModelRegistry::new();

        // Pre-load KV store entries for all provider API keys
        let builtin = clawhive_model_router::providers::provider_configs();
        let mut kv_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for slot in &builtin {
            let store_key = format!("config:{}_api_key", slot.name);
            if let Ok(Some(val)) = self.state.kv_store.get::<String>(&store_key).await {
                let trimmed = val.trim().to_string();
                if !trimmed.is_empty() {
                    kv_map.insert(store_key, trimmed);
                }
            }
        }

        let config = discover_config();

        if let Some(ref cfg) = config {
            let kv_get = |key: &str| kv_map.get(key).cloned();
            let (resolved, errors) = resolve_providers(Some(cfg), builtin, kv_get);
            for e in &errors {
                tracing::warn!("config error: {e:?}");
            }
            registry.register_resolved_providers(resolved);
        } else {
            // No config file — fallback: env var → KV store for each built-in provider
            for config in clawhive_model_router::providers::provider_configs() {
                let key = match std::env::var(config.api_key_env) {
                    Ok(k) if !k.is_empty() => Some(k),
                    _ => {
                        let store_key = format!("config:{}_api_key", config.name);
                        self.state.kv_store.get::<String>(&store_key).await
                            .ok()
                            .flatten()
                            .filter(|k| !k.trim().is_empty())
                            .map(|k| k.trim().to_string())
                    }
                };
                if let Some(key) = key {
                    registry.register(Box::new(
                        clawhive_model_router::openai_compat::OpenAiCompatibleProvider::with_config(
                            config.name,
                            config.base_url,
                            key,
                            config.models.clone(),
                        ),
                    ));
                }
            }
        }

        let total = registry.list_profiles().len();
        if total > 0 {
            let router = Arc::new(clawhive_model_router::router::ModelRouter::new(registry));
            if let Some(first) = router.registry().list_profiles().first() {
                self.active_model = first.model_name.clone();
            }
            self.state.model_router = Some(router);
        }
    }

    /// Called after `model_sel_provider` is set: fetch models and advance to family/variant list.
    pub(crate) async fn advance_to_model_list(&mut self) {
        let provider_name = self.model_sel_provider.clone();
        self.model_sel_search.clear();
        self.model_sel_index = 0;
        self.command_mode = CommandMode::ModelSelection;

        // Clone Arc agar bisa digunakan bebas tanpa borrow conflict di blok async
        let router_arc = self.state.model_router.clone();

        if let Some(router) = &router_arc {
            if let Ok(provider) = router.registry().get_provider(&provider_name) {
                match provider.fetch_models().await {
                    Ok(models) => {
                        // Inject semua model yang di-fetch ke registry agar bisa di-route
                        router.inject_profiles(models.clone());
                        let families = group_models_by_family(models);
                        self.model_sel_families = families;
                        self.model_sel_step = ModelSelectionStep::SelectFamily;
                    }
                    Err(_) => {
                        // Fallback: gunakan profile yang sudah diregistrasi untuk provider ini
                        let profiles: Vec<_> = router
                            .registry()
                            .list_profiles()
                            .into_iter()
                            .filter(|p| p.provider == provider_name)
                            .collect();
                        if profiles.is_empty() {
                            self.status_message =
                                format!("No models available for {provider_name}");
                            self.reset_model_selection();
                            self.command_mode = CommandMode::None;
                        } else {
                            let families = group_models_by_family(profiles);
                            self.model_sel_families = families;
                            self.model_sel_step = ModelSelectionStep::SelectFamily;
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn reset_model_selection(&mut self) {
        self.model_sel_step = ModelSelectionStep::SelectProvider;
        self.model_sel_provider.clear();
        self.model_sel_families.clear();
        self.model_sel_variants.clear();
        self.model_sel_search.clear();
        self.model_sel_index = 0;
        self.model_sel_pending_provider = None;
    }

    /// Number of items in the current model-selection list (filtered by search).
    pub(crate) fn active_model_list_len(&self) -> usize {
        let search = self.model_sel_search.to_lowercase();
        match self.model_sel_step {
            ModelSelectionStep::SelectProvider => {
                let all = self.all_catalog_providers();
                if search.is_empty() {
                    all.len()
                } else {
                    all.iter()
                        .filter(|(name, _)| name.to_lowercase().contains(&search))
                        .count()
                }
            }
            ModelSelectionStep::SelectFamily => {
                if search.is_empty() {
                    self.model_sel_families.len()
                } else {
                    self.model_sel_families
                        .iter()
                        .filter(|f| f.name.to_lowercase().contains(&search))
                        .count()
                }
            }
            ModelSelectionStep::SelectVariant => {
                if search.is_empty() {
                    self.model_sel_variants.len()
                } else {
                    self.model_sel_variants
                        .iter()
                        .filter(|v| {
                            v.id.to_lowercase().contains(&search)
                                || v.model_name.to_lowercase().contains(&search)
                                || v.suitable_for.iter().any(|t| t.to_lowercase().contains(&search))
                        })
                        .count()
                }
            }
        }
    }

    /// Handle Enter key during model selection.
    pub(crate) async fn handle_model_selection_enter(&mut self) {
        match self.model_sel_step {
            ModelSelectionStep::SelectProvider => {
                // Resolve the selected provider name from the filtered list
                let providers: Vec<String> = {
                    let search = self.model_sel_search.to_lowercase();
                    let all: Vec<String> = self
                        .all_catalog_providers()
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect();
                    if search.is_empty() {
                        all
                    } else {
                        all.into_iter()
                            .filter(|p| p.to_lowercase().contains(&search))
                            .collect()
                    }
                };
                if self.model_sel_index >= providers.len() {
                    return;
                }
                let provider_name = providers[self.model_sel_index].clone();
                self.model_sel_provider = provider_name.clone();
                self.model_sel_search.clear();
                self.model_sel_index = 0;

                // Check if provider is actually registered (has API key)
                if !self.provider_is_configured(&provider_name) {
                    // Provider not configured — ask for API key first
                    self.model_sel_pending_provider = Some(provider_name.clone());
                    self.command_mode = CommandMode::ApiKeyInput {
                        key_input: String::new(),
                        error_message: String::new(),
                    };
                    return;
                }

                // Fetch models from the provider
                self.advance_to_model_list().await;
            }
            ModelSelectionStep::SelectFamily => {
                let search = self.model_sel_search.to_lowercase();
                let filtered: Vec<&ModelFamily> = if search.is_empty() {
                    self.model_sel_families.iter().collect()
                } else {
                    self.model_sel_families
                        .iter()
                        .filter(|f| f.name.to_lowercase().contains(&search))
                        .collect()
                };
                if self.model_sel_index >= filtered.len() {
                    return;
                }
                let variants = filtered[self.model_sel_index].variants.clone();
                self.model_sel_search.clear();
                self.model_sel_index = 0;
                if variants.len() == 1 {
                    // Only one variant — select it directly
                    self.active_model = variants[0].id.clone();
                    self.status_message = format!(
                        "Active model: {} ({})",
                        self.active_model,
                        variants[0].provider,
                    );
                    self.reset_model_selection();
                    self.command_mode = CommandMode::None;
                } else {
                    self.model_sel_variants = variants;
                    self.model_sel_step = ModelSelectionStep::SelectVariant;
                }
            }
            ModelSelectionStep::SelectVariant => {
                let search = self.model_sel_search.to_lowercase();
                let filtered: Vec<&ModelProfile> = if search.is_empty() {
                    self.model_sel_variants.iter().collect()
                } else {
                    self.model_sel_variants
                        .iter()
                        .filter(|v| {
                            v.id.to_lowercase().contains(&search)
                                || v.model_name.to_lowercase().contains(&search)
                                || v.suitable_for.iter().any(|t| t.to_lowercase().contains(&search))
                        })
                        .collect()
                };
                if self.model_sel_index >= filtered.len() {
                    return;
                }
                let selected = filtered[self.model_sel_index].clone();

                // Inject profile ke router agar model hasil auto-fetch bisa ditemukan
                // saat routing (fix: "model not available" untuk model dinamis)
                if let Some(router) = &self.state.model_router {
                    router.inject_profile(selected.clone());
                }

                self.active_model = selected.id.clone();
                self.status_message = format!(
                    "Model: {} (via {})",
                    selected.id,
                    selected.provider
                );
                self.command_mode = CommandMode::None;
                self.reset_model_selection();
            }
        }
    }

    /// Collect provider names from the model router (only configured/registered ones).
    pub fn configured_providers(&self) -> Vec<String> {
        self.state
            .model_router
            .as_ref()
            .map(|r| {
                r.registry()
                    .list_profiles()
                    .iter()
                    .map(|p| p.provider.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// All known providers from the built-in catalog (regardless of registration).
    /// Used for model selection so users can pick a provider and set API key.
    pub fn all_catalog_providers(&self) -> Vec<(String, bool)> {
        let catalog = clawhive_model_router::providers::provider_configs();
        let configured: std::collections::BTreeSet<String> = self
            .state
            .model_router
            .as_ref()
            .map(|r| {
                r.registry()
                    .list_profiles()
                    .iter()
                    .map(|p| p.provider.clone())
                    .collect()
            })
            .unwrap_or_default();
        catalog
            .into_iter()
            .map(|c| {
                let is_cfg = configured.contains(c.name);
                (c.name.to_string(), is_cfg)
            })
            .collect()
    }

    /// Check if a provider name is registered in the router.
    pub fn provider_is_configured(&self, name: &str) -> bool {
        self.state.model_router.as_ref().is_some_and(|r| {
            r.registry().get_provider(name).is_ok()
        })
    }

    pub(crate) async fn persist_api_key(&self, provider: &str, api_key: &str) {
        let key = format!("config:{}_api_key", provider);
        let _ = self
            .state
            .kv_store
            .set::<String>(&key, &api_key.to_string())
            .await;
    }

    pub(crate) async fn load_saved_api_key(&mut self) {
        if self.state.model_router.is_some() {
            return;
        }
        self.register_all_providers().await;
    }
}
