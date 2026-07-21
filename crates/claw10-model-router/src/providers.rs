//! Known AI provider configurations — dynamically loaded from models.dev.
//!
//! The provider catalog is fetched from `https://models.dev/api.json` at first
//! access and cached in memory for the process lifetime. A file cache with a
//! 24-hour TTL avoids redundant network calls.
//!
//! Users can add custom providers via the `[custom]` section in `claw10.toml`.

use std::sync::OnceLock;

use crate::models_dev;
use crate::types::ModelProfile;

/// A descriptor for creating an OpenAI-compatible provider.
#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key_env: String,
    pub notes: String,
    pub models: Vec<ModelProfile>,
}

static PROVIDERS: OnceLock<Vec<ProviderConfig>> = OnceLock::new();

/// Pre-fetch provider catalog from models.dev. Call at startup before any
/// `provider_configs()` call. Safe to call multiple times (only the first
/// call does the actual fetch).
pub async fn init_providers() {
    let _ = PROVIDERS.get_or_init(|| {
        tracing::info!("Fetching provider catalog from models.dev");
        // Check if we're inside a tokio runtime. If so, we can't create a new one.
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // Inside a tokio runtime — can't block, use fallback
                tracing::info!("Inside tokio runtime, using fallback providers");
                fallback_providers()
            }
            Err(_) => {
                // Not inside a tokio runtime — create one and fetch
                match tokio::runtime::Runtime::new() {
                    Ok(rt) => {
                        let providers = rt.block_on(models_dev::fetch_providers());
                        match providers {
                            Ok(p) if !p.is_empty() => {
                                tracing::info!(
                                    "Successfully loaded {} providers from models.dev",
                                    p.len()
                                );
                                p
                            }
                            Ok(_) => {
                                tracing::warn!("models.dev returned empty list, using fallback");
                                fallback_providers()
                            }
                            Err(e) => {
                                tracing::warn!("Failed to fetch from models.dev: {e}, using fallback");
                                fallback_providers()
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create tokio runtime: {e}, using fallback");
                        fallback_providers()
                    }
                }
            }
        }
    });
}

/// Fetch providers from models.dev synchronously. Can be called from outside
/// a tokio runtime (e.g. from a CLI subcommand that doesn't use async).
pub fn init_providers_sync() {
    let _ = PROVIDERS.get_or_init(|| {
        tracing::info!("Fetching provider catalog from models.dev (sync)");
        match tokio::runtime::Runtime::new() {
            Ok(rt) => {
                let providers = rt.block_on(models_dev::fetch_providers());
                match providers {
                    Ok(p) if !p.is_empty() => {
                        tracing::info!(
                            "Successfully loaded {} providers from models.dev",
                            p.len()
                        );
                        eprintln!("[claw10] Loaded {} providers from models.dev", p.len());
                        p
                    }
                    Ok(_) => {
                        tracing::warn!("models.dev returned empty list, using fallback");
                        eprintln!("[claw10] models.dev returned empty list, using fallback (15 providers)");
                        fallback_providers()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch from models.dev: {e}, using fallback");
                        eprintln!("[claw10] Failed to fetch from models.dev: {e}, using fallback (15 providers)");
                        fallback_providers()
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to create tokio runtime: {e}, using fallback");
                eprintln!("[claw10] Failed to create tokio runtime: {e}, using fallback (15 providers)");
                fallback_providers()
            }
        }
    });
}

/// Return configurations for every known OpenAI-compatible provider.
pub fn provider_configs() -> Vec<ProviderConfig> {
    get_all_configs().clone()
}

/// Look up a single provider slot by name (e.g. "openai", "groq").
/// Returns `None` if the slot is unknown.
pub fn get_provider_slot(name: &str) -> Option<ProviderConfig> {
    get_all_configs()
        .iter()
        .find(|c| c.name == name)
        .cloned()
}

/// Get all provider configs. If `init_providers()` was called, returns the
/// full catalog. Otherwise falls back to a minimal set.
fn get_all_configs() -> &'static Vec<ProviderConfig> {
    PROVIDERS.get_or_init(|| {
        // Called before init_providers() — use fallback
        tracing::warn!("provider_configs() called before init_providers(), using fallback");
        eprintln!("[claw10] WARNING: provider_configs() called before init_providers(), using fallback (15 providers)");
        fallback_providers()
    })
}

/// Minimal fallback providers when models.dev is unreachable.
/// These cover the most common providers so the system still works offline.
fn fallback_providers() -> Vec<ProviderConfig> {
    vec![
        fallback_config("openai", "https://api.openai.com/v1", "OPENAI_API_KEY", "OpenAI"),
        fallback_config("anthropic", "https://api.anthropic.com/v1", "ANTHROPIC_API_KEY", "Anthropic"),
        fallback_config("google-gemini", "https://generativelanguage.googleapis.com/v1beta/openai", "GEMINI_API_KEY", "Google Gemini"),
        fallback_config("deepseek", "https://api.deepseek.com", "DEEPSEEK_API_KEY", "DeepSeek"),
        fallback_config("openrouter", "https://openrouter.ai/api/v1", "OPENROUTER_API_KEY", "OpenRouter"),
        fallback_config("groq", "https://api.groq.com/openai/v1", "GROQ_API_KEY", "Groq"),
        fallback_config("mistral", "https://api.mistral.ai/v1", "MISTRAL_API_KEY", "Mistral"),
        fallback_config("cohere", "https://api.cohere.ai/v1", "COHERE_API_KEY", "Cohere"),
        fallback_config("xai", "https://api.x.ai/v1", "XAI_API_KEY", "xAI"),
        fallback_config("together", "https://api.together.xyz/v1", "TOGETHER_API_KEY", "Together AI"),
        fallback_config("fireworks", "https://api.fireworks.ai/inference/v1", "FIREWORKS_API_KEY", "Fireworks AI"),
        fallback_config("perplexity", "https://api.perplexity.ai", "PERPLEXITY_API_KEY", "Perplexity"),
        fallback_config("nvidia", "https://integrate.api.nvidia.com/v1", "NVIDIA_API_KEY", "NVIDIA NIM"),
        fallback_config("ollama", "http://localhost:11434/v1", "OLLAMA_API_KEY", "Ollama (local)"),
        fallback_config("lm-studio", "http://localhost:1234/v1", "LM_STUDIO_API_KEY", "LM Studio (local)"),
    ]
}

fn fallback_config(
    name: &str,
    base_url: &str,
    api_key_env: &str,
    notes: &str,
) -> ProviderConfig {
    ProviderConfig {
        name: name.to_string(),
        base_url: base_url.to_string(),
        api_key_env: api_key_env.to_string(),
        notes: notes.to_string(),
        models: vec![], // Models loaded dynamically; empty means "use any model"
    }
}
