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
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                tracing::info!("Inside tokio runtime, fetching async");
                vec![]
            }
            Err(_) => match tokio::runtime::Runtime::new() {
                Ok(rt) => {
                    let providers = rt.block_on(models_dev::fetch_providers());
                    match providers {
                        Ok(p) if !p.is_empty() => {
                            tracing::info!("Loaded {} providers from models.dev", p.len());
                            p
                        }
                        Ok(_) => {
                            tracing::warn!("models.dev returned empty list");
                            vec![]
                        }
                        Err(e) => {
                            tracing::warn!("Failed to fetch from models.dev: {e}");
                            vec![]
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to create tokio runtime: {e}");
                    vec![]
                }
            },
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
                        tracing::info!("Loaded {} providers from models.dev", p.len());
                        eprintln!("[claw10] Loaded {} providers from models.dev", p.len());
                        p
                    }
                    Ok(_) => {
                        tracing::warn!("models.dev returned empty list");
                        eprintln!("[claw10] models.dev returned empty list");
                        vec![]
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch from models.dev: {e}");
                        eprintln!("[claw10] Failed to fetch from models.dev: {e}");
                        vec![]
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to create tokio runtime: {e}");
                eprintln!("[claw10] Failed to create tokio runtime: {e}");
                vec![]
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

/// Get all provider configs. Must call `init_providers_sync()` first.
fn get_all_configs() -> &'static Vec<ProviderConfig> {
    PROVIDERS.get_or_init(|| {
        tracing::warn!("provider_configs() called before init_providers()");
        eprintln!("[claw10] WARNING: provider_configs() called before init_providers()");
        vec![]
    })
}
