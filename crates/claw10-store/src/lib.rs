use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::RwLock;

pub mod namespaced;
pub use namespaced::NamespacedStore;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

/// Object-safe core store trait (no generics).
#[async_trait]
pub trait Store: Send + Sync {
    async fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError>;
    async fn set_raw(&self, key: &str, value: Vec<u8>) -> Result<(), StoreError>;
    async fn delete(&self, key: &str) -> Result<(), StoreError>;
    async fn exists(&self, key: &str) -> Result<bool, StoreError>;
    async fn scan_prefix_raw(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StoreError>;
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, StoreError>;
    async fn clear(&self) -> Result<(), StoreError>;
}

/// Typed convenience methods (auto-implemented for all `Store` types, including `dyn Store`).
#[async_trait]
pub trait StoreExt: Store {
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>, StoreError> {
        match self.get_raw(key).await? {
            Some(bytes) => {
                let value =
                    serde_json::from_slice(&bytes).map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set<T: Serialize + Send + Sync>(&self, key: &str, value: &T) -> Result<(), StoreError> {
        let bytes =
            serde_json::to_vec(value).map_err(|e| StoreError::Serialization(e.to_string()))?;
        self.set_raw(key, bytes).await
    }

    async fn scan_prefix<T: DeserializeOwned + Send>(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, T)>, StoreError> {
        let mut results = self.scan_prefix_unsorted(prefix).await?;
        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    /// Sama seperti `scan_prefix` tapi tidak melakukan sorting.
    /// Cocok untuk path yang tidak membutuhkan urutan key.
    async fn scan_prefix_unsorted<T: DeserializeOwned + Send>(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, T)>, StoreError> {
        let raw = self.scan_prefix_raw(prefix).await?;
        let mut results = Vec::new();
        for (key, bytes) in raw {
            let value = serde_json::from_slice(&bytes)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            results.push((key, value));
        }
        Ok(results)
    }
}

impl<T: Store + ?Sized> StoreExt for T {}

// ── InMemoryStore ─────────────────────────────────────────────────

#[derive(Clone)]
pub struct InMemoryStore {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl InMemoryStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Store for InMemoryStore {
    async fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn set_raw(&self, key: &str, value: Vec<u8>) -> Result<(), StoreError> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StoreError> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }

    async fn scan_prefix_raw(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
        let data = self.data.read().await;
        let mut results = Vec::new();
        for (key, bytes) in data.iter() {
            if key.starts_with(prefix) {
                results.push((key.clone(), bytes.clone()));
            }
        }
        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        let data = self.data.read().await;
        let mut keys: Vec<String> = data
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn clear(&self) -> Result<(), StoreError> {
        let mut data = self.data.write().await;
        data.clear();
        Ok(())
    }
}

// ── JsonFileStore ─────────────────────────────────────────────────

/// Persistent key-value store backed by a single JSON file.
/// All data is kept in-memory for fast lookups and synced to disk on every write.
/// Uses atomic writes (temp file + rename) for crash safety.
#[derive(Clone)]
pub struct JsonFileStore {
    path: Option<std::path::PathBuf>,
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl JsonFileStore {
    /// Open or create a JSON file store at the given path.
    ///
    /// # Errors
    /// Returns an error if the file exists but cannot be read/parsed.
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        let data = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| StoreError::Database(format!("failed to read store file: {e}")))?;
            if content.trim().is_empty() {
                HashMap::new()
            } else {
                let map: HashMap<String, serde_json::Value> = serde_json::from_str(&content)
                    .map_err(|e| StoreError::Database(format!("failed to parse store file: {e}")))?;
                // Convert JSON values back to raw bytes
                map.into_iter()
                    .map(|(k, v)| {
                        let bytes = serde_json::to_vec(&v).unwrap_or_default();
                        (k, bytes)
                    })
                    .collect()
            }
        } else {
            // Create parent directory if needed
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            HashMap::new()
        };
        Ok(Self {
            path: Some(path),
            data: Arc::new(RwLock::new(data)),
        })
    }

    /// Create a temporary in-memory store (no disk persistence).
    #[must_use]
    pub fn new_temporary() -> Self {
        Self {
            path: None,
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Flush all data to disk atomically.
    async fn flush(&self) -> Result<(), StoreError> {
        let Some(ref path) = self.path else {
            return Ok(());
        };
        let data = self.data.read().await;

        // Build a JSON map from raw bytes
        let mut map = serde_json::Map::new();
        for (key, bytes) in data.iter() {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) {
                map.insert(key.clone(), value);
            }
        }

        let json = serde_json::to_string_pretty(&map)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        // Atomic write: write to temp file then rename
        let tmp_path = path.with_extension("json.tmp");
        tokio::fs::write(&tmp_path, &json)
            .await
            .map_err(|e| StoreError::Database(format!("failed to write store file: {e}")))?;
        tokio::fs::rename(&tmp_path, path)
            .await
            .map_err(|e| StoreError::Database(format!("failed to rename store file: {e}")))?;

        Ok(())
    }
}

#[async_trait]
impl Store for JsonFileStore {
    async fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn set_raw(&self, key: &str, value: Vec<u8>) -> Result<(), StoreError> {
        {
            let mut data = self.data.write().await;
            data.insert(key.to_string(), value);
        }
        self.flush().await
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        {
            let mut data = self.data.write().await;
            data.remove(key);
        }
        self.flush().await
    }

    async fn exists(&self, key: &str) -> Result<bool, StoreError> {
        let data = self.data.read().await;
        Ok(data.contains_key(key))
    }

    async fn scan_prefix_raw(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
        let data = self.data.read().await;
        let mut results: Vec<(String, Vec<u8>)> = data
            .iter()
            .filter(|(key, _)| key.starts_with(prefix))
            .map(|(key, bytes)| (key.clone(), bytes.clone()))
            .collect();
        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        let data = self.data.read().await;
        let mut keys: Vec<String> = data
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        Ok(keys)
    }

    async fn clear(&self) -> Result<(), StoreError> {
        {
            let mut data = self.data.write().await;
            data.clear();
        }
        self.flush().await
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

