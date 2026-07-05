use std::sync::Arc;

use crate::app::{CommandMode, ModelSelectionStep, TuiApp};
use claw10_model_router::types::{ModelProfile, group_models_by_family};
use claw10_store::StoreExt;

impl TuiApp {
    /// Scan config file, env vars, and KV store for every known provider.
    /// Supports the ZeroClaw-style alias system from `claw10.toml`.
    pub(crate) async fn register_all_providers(&mut self) {
        use claw10_model_router::config::{discover_config, resolve_providers};
        use claw10_model_router::provider::ModelRegistry;
        use claw10_store::StoreExt;

        let mut registry = ModelRegistry::new();

        // Pre-load KV store entries for all provider API keys dari global_store
        let builtin = claw10_model_router::providers::provider_configs();
        let mut kv_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for slot in &builtin {
            let store_key = format!("config:{}_api_key", slot.name);
            if let Ok(Some(val)) = self.global_store.get::<String>(&store_key).await {
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
            // No config file — fallback: env var → KV store (global_store) for each built-in provider
            for config in claw10_model_router::providers::provider_configs() {
                // Native providers (e.g. Bedrock) are registered via their factory.
                if let Some(factory) = config.factory {
                    let name = config.name.to_string();
                    if !registry.list_providers().contains(&name) {
                        registry.register(factory());
                    }
                    continue;
                }

                let key = match std::env::var(config.api_key_env) {
                    Ok(k) if !k.is_empty() => Some(k),
                    _ => {
                        let store_key = format!("config:{}_api_key", config.name);
                        self.global_store
                            .get::<String>(&store_key)
                            .await
                            .ok()
                            .flatten()
                            .filter(|k| !k.trim().is_empty())
                            .map(|k| k.trim().to_string())
                    }
                };
                if let Some(key) = key {
                    registry.register(Box::new(
                        claw10_model_router::openai_compat::OpenAiCompatibleProvider::with_config(
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
            let router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));
            if let Some(first) = router.registry().list_profiles().first() {
                self.set_active_model(first.id.clone());
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
                    Ok(mut models) => {
                        let priority_names = load_cached_models(&provider_name);

                        for name in priority_names {
                            let name_lower = name.to_lowercase();
                            let exists = models.iter().any(|m| {
                                m.id.to_lowercase().contains(&name_lower) 
                                    || name_lower.contains(&m.id.to_lowercase())
                            });
                            if !exists {
                                models.push(claw10_model_router::types::ModelProfile {
                                    id: name.clone(),
                                    provider: provider_name.clone(),
                                    model_name: name,
                                    context_window: 128_000,
                                    max_output_tokens: 8_192,
                                    cost_per_1m_input: 0.0,
                                    cost_per_1m_output: 0.0,
                                    suitable_for: vec!["general".to_string()],
                                });
                            }
                        }

                        // Inject semua model yang di-fetch & dari JSON ke registry agar bisa di-route
                        router.inject_profiles(models.clone());
                        let families = group_models_by_family(models);
                        self.model_sel_families = families;
                        self.model_sel_step = ModelSelectionStep::SelectFamily;
                    }
                    Err(_) => {
                        let priority_names = load_cached_models(&provider_name);

                        let mut profiles: Vec<claw10_model_router::types::ModelProfile> = Vec::new();
                        for name in priority_names {
                            let name_lower = name.to_lowercase();
                            let exists = profiles.iter().any(|p| {
                                p.id.to_lowercase().contains(&name_lower)
                                    || name_lower.contains(&p.id.to_lowercase())
                            });
                            if !exists {
                                profiles.push(claw10_model_router::types::ModelProfile {
                                    id: name.clone(),
                                    provider: provider_name.clone(),
                                    model_name: name,
                                    context_window: 128_000,
                                    max_output_tokens: 8_192,
                                    cost_per_1m_input: 0.0,
                                    cost_per_1m_output: 0.0,
                                    suitable_for: vec!["general".to_string()],
                                });
                            }
                        }

                        if profiles.is_empty() {
                            self.status_message =
                                format!("No models available for {provider_name}");
                            self.reset_model_selection();
                            self.command_mode = CommandMode::None;
                            return;
                        }

                        // Inject profiles fallback
                        router.inject_profiles(profiles.clone());
                        let families = group_models_by_family(profiles);
                        self.model_sel_families = families;
                        self.model_sel_step = ModelSelectionStep::SelectFamily;
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
                let search = self.model_sel_search.to_lowercase();
                let base_len = if search.is_empty() {
                    self.model_sel_families.len()
                } else {
                    self.model_sel_families
                        .iter()
                        .filter(|f| f.name.to_lowercase().contains(&search))
                        .count()
                };
                base_len + 1 // Ditambah 1 karena ada opsi "< Tambah Model Manual >"
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
                                || v.suitable_for
                                    .iter()
                                    .any(|t| t.to_lowercase().contains(&search))
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
                let mut options: Vec<String> = if search.is_empty() {
                    self.model_sel_families.iter().map(|f| f.name.clone()).collect()
                } else {
                    self.model_sel_families
                        .iter()
                        .filter(|f| f.name.to_lowercase().contains(&search))
                        .map(|f| f.name.clone())
                        .collect()
                };
                options.insert(0, "< Tambah Model Manual >".to_string());

                if self.model_sel_index >= options.len() {
                    return;
                }

                let selected_item = options[self.model_sel_index].clone();
                if selected_item == "< Tambah Model Manual >" {
                    self.command_mode = CommandMode::ManualModelInput {
                        model_input: String::new(),
                        error_message: String::new(),
                    };
                    return;
                }

                let family_name = selected_item;
                let family = self.model_sel_families.iter().find(|f| f.name == family_name).unwrap();
                let variants = family.variants.clone();
                self.model_sel_search.clear();
                self.model_sel_index = 0;
                if variants.len() == 1 {
                    // Only one variant — select it directly
                    self.set_active_model(variants[0].id.clone());
                    self.status_message = format!(
                        "Active model: {} ({})",
                        self.active_model, variants[0].provider,
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
                                || v.suitable_for
                                    .iter()
                                    .any(|t| t.to_lowercase().contains(&search))
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

                self.set_active_model(selected.id.clone());
                self.status_message = format!("Model: {} (via {})", selected.id, selected.provider);
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
        let catalog = claw10_model_router::providers::provider_configs();
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
        self.state
            .model_router
            .as_ref()
            .is_some_and(|r| r.registry().get_provider(name).is_ok())
    }

    pub(crate) async fn persist_api_key(&self, provider: &str, api_key: &str) {
        let key = format!("config:{}_api_key", provider);
        let _ = self
            .global_store
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

fn load_cached_models(provider_name: &str) -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let cache_file = std::path::PathBuf::from(&home).join(".claw10").join("models.json");
    let clean_provider = provider_name.split('.').next().unwrap_or(provider_name).to_lowercase();

    if cache_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(arr) = json.get(&clean_provider).and_then(|v| v.as_array()) {
                    return arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                }
            }
        }
    }
    Vec::new()
}
