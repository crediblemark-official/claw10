//! Config-driven provider system — ZeroClaw-style slot/alias TOML config.
//!
//! Config file discovery: `./claw10.toml`, `~/.config/claw10/config.toml`,
//! or `$CLAW10_CONFIG`.
//!
//! # Format
//! ```toml
//! [alias.gpt4]
//! slot = "openai"
//! model = "gpt-4o"
//! api_key = "$OPENAI_API_KEY"
//!
//! [alias.haiku]
//! slot = "anthropic"
//! model = "claude-3.5-haiku"
//! api_key = "$ANTHROPIC_API_KEY"
//!
//! [custom.my-llm]
//! base_url = "https://my-llm.example.com/v1"
//! api_key = "$MY_LLM_KEY"
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::providers::ProviderConfig;
use crate::types::ModelProfile;

// ── Config Structs ─────────────────────────────────────

/// Top-level config file.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct Claw10Config {
    /// Named aliases against built-in provider slots.
    #[serde(default)]
    pub alias: HashMap<String, ProviderAlias>,

    /// Fully custom OpenAI-compatible providers not in the built-in catalog.
    #[serde(default)]
    pub custom: HashMap<String, CustomProvider>,
}

/// An alias referencing a built-in provider slot with a specific model.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderAlias {
    /// Built-in provider slot name (e.g. "openai", "anthropic", "groq").
    pub slot: String,

    /// Specific model ID to use (e.g. "gpt-4o", "claude-3.5-haiku").
    pub model: String,

    /// API key — literal value or `$ENV_VAR` reference.
    pub api_key: String,

    /// Optional temperature override.
    #[serde(default)]
    pub temperature: Option<f64>,

    /// Optional max tokens override.
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// Fallback alias name (within the same config scope) if this provider fails.
    /// Example: `"openai.haiku"` — the router tries this alias next.
    #[serde(default)]
    pub fallback: Option<String>,

    /// Fallback models within the same provider slot, tried before `fallback` alias.
    /// Example: `["gpt-4o-mini"]` — the router tries these models on the same provider.
    #[serde(default)]
    pub fallback_models: Vec<String>,
}

/// A custom OpenAI-compatible provider not in the built-in catalog.
#[derive(Debug, Clone, Deserialize)]
pub struct CustomProvider {
    /// Base URL for the API (e.g. "https://my-llm.example.com/v1").
    pub base_url: String,

    /// API key — literal value or `$ENV_VAR` reference.
    pub api_key: String,

    /// Model IDs this custom provider supports (fetched from API if empty).
    #[serde(default)]
    pub models: Vec<String>,

    /// Optional metadata overrides per model ID.
    #[serde(default)]
    pub model_meta: HashMap<String, CustomModelMeta>,
}

/// Metadata overrides for a custom provider model.
#[derive(Debug, Clone, Deserialize)]
pub struct CustomModelMeta {
    #[serde(default = "default_context_window")]
    pub context_window: u32,
    #[serde(default = "default_max_output")]
    pub max_output_tokens: u32,
    #[serde(default)]
    pub cost_per_1m_input: f64,
    #[serde(default)]
    pub cost_per_1m_output: f64,
    #[serde(default)]
    pub suitable_for: Vec<String>,
}

fn default_context_window() -> u32 {
    128_000
}
fn default_max_output() -> u32 {
    8_192
}

// ── Resolved provider descriptor (ready to instantiate) ──

/// A fully resolved provider descriptor — all fields ready to create
/// an `OpenAiCompatibleProvider`.
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    /// Registry name (e.g. "openai.gpt4" for aliases, or "openai" for bare slots).
    pub name: String,

    /// Base URL for the API.
    pub base_url: String,

    /// Resolved API key string.
    pub api_key: String,

    /// Model profiles this provider should serve.
    pub models: Vec<ModelProfile>,
}

// ── Config discovery ───────────────────────────────────

/// Find and load the config file, returning `None` if no file is found.
pub fn discover_config() -> Option<Claw10Config> {
    let paths = config_file_candidates();
    for path in &paths {
        if path.exists() {
            let content = std::fs::read_to_string(path).ok()?;
            return toml::from_str(&content).ok();
        }
    }
    None
}

/// Ordered list of candidate config file paths.
fn config_file_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // 1. Env var override
    if let Ok(path) = std::env::var("CLAW10_CONFIG") {
        candidates.push(PathBuf::from(path));
    }

    // 2. Current working directory
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("claw10.toml"));
    }

    // 3. XDG config directory (~/.config/claw10/config.toml)
    if let Some(config_dir) = dirs::config_dir() {
        candidates.push(config_dir.join("claw10").join("config.toml"));
    }

    // 4. Home directory (~/.claw10.toml)
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claw10.toml"));
        candidates.push(home.join(".claw10").join("config.toml"));
    }

    candidates
}

// ── Resolution ─────────────────────────────────────────

/// Error type for config resolution.
#[derive(Debug)]
pub enum ConfigError {
    SlotNotFound(String),
    ModelNotFound { slot: String, model: String },
    NoApiKey(String),
}

/// Resolve all configured providers (aliases + custom) into a list of
/// `ResolvedProvider` descriptors ready for registration.
///
/// Also returns bare slot registrations for any built-in providers whose
/// env vars / KV keys are present but have no explicit alias.
pub fn resolve_providers(
    config: Option<&Claw10Config>,
    builtin: Vec<ProviderConfig>,
    kv_get: impl Fn(&str) -> Option<String>,
) -> (Vec<ResolvedProvider>, Vec<ConfigError>) {
    let mut resolved = Vec::new();
    let mut errors = Vec::new();

    // Build a slot lookup from built-in catalog
    let slot_map: HashMap<&str, &ProviderConfig> = builtin.iter().map(|c| (c.name, c)).collect();

    let mut accounted_slots: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();

    // 1. Resolve aliases from config
    if let Some(cfg) = config {
        for (alias_name, alias) in &cfg.alias {
            let registry_name = format!("{}.{}", alias.slot, alias_name);

            let slot = match slot_map.get(alias.slot.as_str()) {
                Some(s) => s,
                None => {
                    errors.push(ConfigError::SlotNotFound(alias.slot.clone()));
                    continue;
                }
            };

            // Native providers (e.g. Bedrock) are registered directly by the runtime
            // and do not resolve to an OpenAI-compatible descriptor.
            if slot.factory.is_some() {
                accounted_slots.insert(alias.slot.as_str());
                continue;
            }

            // Resolve API key: try alias.api_key first (with $ENV expansion),
            // then fall through to slot's env var / KV
            let api_key = resolve_api_key(&alias.api_key, &slot.api_key_env, &kv_get);
            let api_key = match api_key {
                Some(k) => k,
                None => {
                    errors.push(ConfigError::NoApiKey(registry_name));
                    continue;
                }
            };

            // Find the model profile from the slot's built-in models
            let profile = slot
                .models
                .iter()
                .find(|m| m.id == alias.model || m.model_name == alias.model)
                .cloned()
                .unwrap_or_else(|| {
                    // Create a minimal profile if the model isn't in the catalog
                    ModelProfile {
                        id: alias.model.clone(),
                        provider: registry_name.clone(),
                        model_name: alias.model.clone(),
                        context_window: 128_000,
                        max_output_tokens: alias.max_tokens.unwrap_or(8_192),
                        cost_per_1m_input: 0.0,
                        cost_per_1m_output: 0.0,
                        suitable_for: vec!["general".to_string()],
                    }
                });

            resolved.push(ResolvedProvider {
                name: registry_name,
                base_url: slot.base_url.to_string(),
                api_key,
                models: vec![profile],
            });

            accounted_slots.insert(alias.slot.as_str());
        }

        // 2. Resolve custom providers from config
        for (custom_name, custom) in &cfg.custom {
            let api_key = resolve_api_key(&custom.api_key, "", &kv_get);
            let api_key = match api_key {
                Some(k) => k,
                None => {
                    errors.push(ConfigError::NoApiKey(format!("custom.{custom_name}")));
                    continue;
                }
            };

            let models: Vec<ModelProfile> = if custom.models.is_empty() {
                // No models listed — fetch via API at runtime
                Vec::new()
            } else {
                custom
                    .models
                    .iter()
                    .map(|m| {
                        let meta = custom.model_meta.get(m);
                        ModelProfile {
                            id: m.clone(),
                            provider: format!("custom.{custom_name}"),
                            model_name: m.clone(),
                            context_window: meta
                                .map(|m| m.context_window)
                                .unwrap_or_else(default_context_window),
                            max_output_tokens: meta
                                .map(|m| m.max_output_tokens)
                                .unwrap_or_else(default_max_output),
                            cost_per_1m_input: meta.map(|m| m.cost_per_1m_input).unwrap_or(0.0),
                            cost_per_1m_output: meta.map(|m| m.cost_per_1m_output).unwrap_or(0.0),
                            suitable_for: meta
                                .map(|m| m.suitable_for.clone())
                                .unwrap_or_default()
                                .into_iter()
                                .chain(std::iter::once("general".to_string()))
                                .collect(),
                        }
                    })
                    .collect()
            };

            resolved.push(ResolvedProvider {
                name: format!("custom.{custom_name}"),
                base_url: custom.base_url.clone(),
                api_key,
                models,
            });
        }
    }

    // 3. Register bare slots (no alias defined) if they have API keys.
    // Native providers are skipped here; they register themselves outside config resolution.
    for slot in &builtin {
        if accounted_slots.contains(slot.name) || slot.factory.is_some() {
            continue;
        }
        let api_key = resolve_api_key("", slot.api_key_env, &kv_get);
        if let Some(key) = api_key {
            resolved.push(ResolvedProvider {
                name: slot.name.to_string(),
                base_url: slot.base_url.to_string(),
                api_key: key,
                models: slot.models.clone(),
            });
        }
    }

    (resolved, errors)
}

/// Resolve an API key from various sources.
/// Priority: inline literal > `$ENV_VAR` expansion > slot env var > KV fallback.
fn resolve_api_key(
    inline_or_ref: &str,
    slot_env: &str,
    kv_get: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    // 1. Try inline value or $ENV_VAR reference
    if !inline_or_ref.is_empty() {
        if let Some(env_name) = inline_or_ref.strip_prefix('$') {
            if let Ok(val) = std::env::var(env_name) {
                if !val.is_empty() {
                    return Some(val);
                }
            }
            // Fall through: check KV store with this env name as key
            let store_key = format!("config:{}_api_key", env_name.to_lowercase());
            if let Some(val) = kv_get(&store_key) {
                if !val.is_empty() {
                    return Some(val);
                }
            }
        } else {
            // Literal inline key
            return Some(inline_or_ref.to_string());
        }
    }

    // 2. Try slot's default env var
    if !slot_env.is_empty() {
        if let Ok(val) = std::env::var(slot_env) {
            if !val.is_empty() {
                return Some(val);
            }
        }
        // 3. Try KV store with slot name
        let store_key = format!(
            "config:{}_api_key",
            slot_env.to_lowercase().trim_end_matches("_API_KEY")
        );
        if let Some(val) = kv_get(&store_key) {
            if !val.is_empty() {
                return Some(val);
            }
        }
    }

    None
}

#[cfg(test)]
#[path = "config_test.rs"]
mod tests;
