use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::RwLock;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl From<sled::Error> for StoreError {
    fn from(e: sled::Error) -> Self {
        Self::Database(e.to_string())
    }
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
        let raw = self.scan_prefix_raw(prefix).await?;
        let mut results = Vec::new();
        for (key, bytes) in raw {
            let value = serde_json::from_slice(&bytes)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            results.push((key, value));
        }
        results.sort_by(|a, b| a.0.cmp(&b.0));
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

// ── SledStore ─────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SledStore {
    db: sled::Db,
}

impl SledStore {
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        let db = sled::open(path).map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(Self { db })
    }

    pub fn new_temporary() -> Result<Self, StoreError> {
        let db = sled::Config::new()
            .temporary(true)
            .open()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(Self { db })
    }
}

#[async_trait]
impl Store for SledStore {
    async fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        match self.db.get(key.as_bytes())? {
            Some(ivec) => Ok(Some(ivec.to_vec())),
            None => Ok(None),
        }
    }

    async fn set_raw(&self, key: &str, value: Vec<u8>) -> Result<(), StoreError> {
        self.db.insert(key.as_bytes(), value)?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.db.remove(key.as_bytes())?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, StoreError> {
        Ok(self.db.contains_key(key.as_bytes())?)
    }

    async fn scan_prefix_raw(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StoreError> {
        let mut results = Vec::new();
        for result in self.db.scan_prefix(prefix.as_bytes()) {
            let (key_bytes, value_bytes) = result?;
            let key = String::from_utf8_lossy(&key_bytes).to_string();
            let value = value_bytes.to_vec();
            results.push((key, value));
        }
        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        let mut keys = Vec::new();
        for result in self.db.scan_prefix(prefix.as_bytes()) {
            let (key_bytes, _) = result?;
            let key = String::from_utf8_lossy(&key_bytes).to_string();
            keys.push(key);
        }
        keys.sort();
        Ok(keys)
    }

    async fn clear(&self) -> Result<(), StoreError> {
        self.db.clear()?;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

