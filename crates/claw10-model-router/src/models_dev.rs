//! Dynamic provider catalog from models.dev.
//!
//! Fetches the provider registry from `https://models.dev/api.json` and converts
//! it into our internal `ProviderConfig` format. Includes a local file cache
//! with a 24-hour TTL to avoid redundant network calls.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::providers::ProviderConfig;
use crate::types::ModelProfile;

/// models.dev API endpoint for the full provider catalog.
const MODELS_DEV_API_URL: &str = "https://models.dev/api.json";

/// Cache TTL: 24 hours.
#[allow(dead_code)]
const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

// ── models.dev JSON structures ──────────────────────────

/// Top-level: `{ [provider_id: string]: ProviderEntry }`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelsDevApi(HashMap<String, ProviderEntry>);

/// A single provider from models.dev.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub api: Option<String>,
    #[serde(default)]
    pub doc: Option<String>,
    #[serde(default)]
    pub npm: Option<String>,
    #[serde(default)]
    pub models: HashMap<String, ModelEntry>,
}

/// A single model within a provider from models.dev.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub tool_call: bool,
    #[serde(default)]
    pub structured_output: Option<bool>,
    #[serde(default)]
    pub temperature: bool,
    #[serde(default)]
    pub limit: Option<ModelLimit>,
    #[serde(default)]
    pub cost: Option<ModelCost>,
    #[serde(default)]
    pub modalities: Option<ModelModalities>,
    #[serde(default)]
    pub open_weights: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelLimit {
    #[serde(default)]
    pub context: Option<u32>,
    #[serde(default)]
    pub input: Option<u32>,
    #[serde(default)]
    pub output: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelCost {
    #[serde(default)]
    pub input: Option<f64>,
    #[serde(default)]
    pub output: Option<f64>,
    #[serde(default)]
    pub cache_read: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelModalities {
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
}

// ── Cache structures ────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
#[allow(dead_code)]
struct CacheEntry {
    timestamp: u64,
    data: ModelsDevApi,
}

#[derive(Debug, Deserialize, Serialize)]
struct ModelsDevApiSerializable(Vec<(String, ProviderEntry)>);

// ── Public API ──────────────────────────────────────────

/// Fetch the provider catalog from models.dev, using a local cache when fresh.
///
/// Returns providers that have an `api` field (OpenAI-compatible endpoints).
/// Native providers (Anthropic, Google, etc.) are skipped since they need
/// their own SDK adapters.
pub async fn fetch_providers() -> Result<Vec<ProviderConfig>, ModelsDevError> {
    let api = fetch_with_cache().await?;
    Ok(convert_to_configs(api))
}

// ── Fetching + caching ──────────────────────────────────

async fn fetch_with_cache() -> Result<ModelsDevApi, ModelsDevError> {
    // Try cache first
    if let Some(cached) = read_cache() {
        tracing::debug!("Using cached models.dev data");
        return Ok(cached);
    }

    // Fetch fresh data
    tracing::info!("Fetching provider catalog from models.dev");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("claw10/1.0")
        .build()
        .map_err(|e| ModelsDevError::Http(e.to_string()))?;

    let resp = client
        .get(MODELS_DEV_API_URL)
        .send()
        .await
        .map_err(|e| ModelsDevError::Http(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(ModelsDevError::Http(format!(
            "HTTP {} from models.dev",
            resp.status()
        )));
    }

    let api: ModelsDevApi = resp
        .json()
        .await
        .map_err(|e| ModelsDevError::Parse(e.to_string()))?;

    // Write cache
    write_cache(&api);

    Ok(api)
}

fn cache_path() -> PathBuf {
    let dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("claw10");
    std::fs::create_dir_all(&dir).ok();
    dir.join("models_dev_cache.json")
}

fn read_cache() -> Option<ModelsDevApi> {
    let path = cache_path();
    let content = std::fs::read_to_string(&path).ok()?;

    // Parse as our serializable format
    let serializable: ModelsDevApiSerializable = serde_json::from_str(&content).ok()?;

    let mut map = HashMap::new();
    for (k, v) in serializable.0 {
        map.insert(k, v);
    }

    Some(ModelsDevApi(map))
}

fn write_cache(api: &ModelsDevApi) {
    // Convert to serializable format
    let vec: Vec<(String, ProviderEntry)> = api.0.clone().into_iter().collect();
    let serializable = ModelsDevApiSerializable(vec);

    if let Ok(json) = serde_json::to_string(&serializable) {
        let _ = std::fs::write(cache_path(), json);
    }
}

// ── Conversion ──────────────────────────────────────────

/// Convert models.dev providers into our `ProviderConfig` format.
fn convert_to_configs(api: ModelsDevApi) -> Vec<ProviderConfig> {
    let mut configs: Vec<ProviderConfig> = Vec::new();

    for (provider_id, entry) in api.0 {
        // Only include providers with an `api` field (OpenAI-compatible)
        let base_url = match &entry.api {
            Some(url) => resolve_template_url(url),
            None => continue, // Skip native providers
        };

        // Skip providers with template URLs we can't resolve
        if base_url.contains("${") {
            tracing::debug!(
                "Skipping provider {} — template URL requires env vars: {}",
                provider_id,
                base_url
            );
            continue;
        }

        // Determine API key env var (use first one)
        let api_key_env = entry
            .env
            .first()
            .cloned()
            .unwrap_or_else(|| "API_KEY".to_string());

        // Convert models
        let models: Vec<ModelProfile> = entry
            .models
            .values()
            .map(|m| convert_model(&provider_id, m))
            .collect();

        if models.is_empty() {
            continue;
        }

        let notes = entry.doc.unwrap_or(entry.name);

        configs.push(ProviderConfig {
            name: provider_id,
            base_url,
            api_key_env,
            notes,
            models,
        });
    }

    // Sort by provider name for deterministic ordering
    configs.sort_by(|a, b| a.name.cmp(&b.name));

    tracing::info!(
        "Loaded {} OpenAI-compatible providers from models.dev",
        configs.len()
    );

    configs
}

/// Convert a single models.dev model entry into our `ModelProfile`.
pub(crate) fn convert_model(provider_id: &str, entry: &ModelEntry) -> ModelProfile {
    let context = entry
        .limit
        .as_ref()
        .and_then(|l| l.context)
        .unwrap_or(128_000);

    let max_output = entry
        .limit
        .as_ref()
        .and_then(|l| l.output)
        .unwrap_or(8_192);

    let cost_in = entry
        .cost
        .as_ref()
        .and_then(|c| c.input)
        .unwrap_or(0.0);

    let cost_out = entry
        .cost
        .as_ref()
        .and_then(|c| c.output)
        .unwrap_or(0.0);

    // Build suitable_for tags from capabilities
    let mut suitable = Vec::new();
    suitable.push("general".to_string());

    if entry.reasoning {
        suitable.push("reasoning".to_string());
    }
    if entry.tool_call {
        suitable.push("coding".to_string());
    }
    if let Some(ref mods) = entry.modalities {
        if mods.input.contains(&"image".to_string())
            || mods.output.contains(&"image".to_string())
        {
            suitable.push("vision".to_string());
        }
        if mods.output.contains(&"video".to_string()) {
            suitable.push("video".to_string());
        }
        if mods.output.contains(&"audio".to_string())
            || mods.input.contains(&"audio".to_string())
        {
            suitable.push("audio".to_string());
        }
    }
    if let Some(true) = entry.open_weights {
        suitable.push("open".to_string());
    }

    ModelProfile {
        id: entry.id.clone(),
        provider: provider_id.to_string(),
        model_name: entry.name.clone(),
        context_window: context,
        max_output_tokens: max_output,
        cost_per_1m_input: cost_in,
        cost_per_1m_output: cost_out,
        suitable_for: suitable,
    }
}

/// Resolve template URLs by replacing `${VAR}` with env var values.
/// If the env var is not set, the template variable is left as-is
/// (callers should skip such URLs).
pub(crate) fn resolve_template_url(url: &str) -> String {
    let mut result = url.to_string();

    // Find all ${...} patterns
    while let Some(start) = result.find("${") {
        let end = match result[start + 2..].find('}') {
            Some(e) => e,
            None => break,
        };

        let var_name = &result[start + 2..start + 2 + end];
        let replacement = std::env::var(var_name).unwrap_or_default();

        result = format!(
            "{}{}{}",
            &result[..start],
            replacement,
            &result[start + 2 + end + 1..]
        );
    }

    result
}

// ── Errors ──────────────────────────────────────────────

#[derive(Debug)]
pub enum ModelsDevError {
    Http(String),
    Parse(String),
}

impl std::fmt::Display for ModelsDevError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "HTTP error fetching models.dev: {e}"),
            Self::Parse(e) => write!(f, "Parse error reading models.dev data: {e}"),
        }
    }
}

impl std::error::Error for ModelsDevError {}

#[cfg(test)]
#[path = "models_dev_test.rs"]
mod tests;
