#![allow(clippy::pedantic)]
use std::sync::Arc;

use chrono::Utc;

use claw10_domain::{AgentId, Artifact, ArtifactId, TaskId};
use claw10_store::{Store, StoreError, StoreExt};

const META_PREFIX: &str = "artifact:meta:";
const CONTENT_PREFIX: &str = "artifact:content:";

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("artifact not found: {0}")]
    NotFound(String),
    #[error("content hash mismatch for artifact {0}")]
    ContentHashMismatch(String),
    #[error("{0}")]
    Store(#[from] StoreError),
}

pub struct ArtifactService {
    store: Arc<dyn Store>,
}

impl ArtifactService {
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    fn meta_key(id: &ArtifactId) -> String {
        format!("{META_PREFIX}{}", id.0)
    }

    fn content_key(id: &ArtifactId) -> String {
        format!("{CONTENT_PREFIX}{}", id.0)
    }

    fn compute_hash(content: &[u8]) -> String {
        use sha2::Digest;
        let hash = sha2::Sha256::digest(content);
        hash.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    }

    pub async fn store_artifact(
        &self,
        task_id: TaskId,
        agent_id: AgentId,
        name: String,
        mime_type: String,
        content: Vec<u8>,
    ) -> Result<Artifact, ArtifactError> {
        let now = Utc::now();
        let content_hash = Self::compute_hash(&content);
        let id = ArtifactId(uuid::Uuid::now_v7());
        let storage_path = format!("store://local/{}", id.0);

        let artifact = Artifact {
            id: id.clone(),
            task_id,
            agent_id,
            name,
            mime_type,
            size_bytes: content.len() as u64,
            storage_path,
            content_hash: content_hash.clone(),
            created_at: now,
        };

        self.store
            .set(&Self::meta_key(&id), &artifact)
            .await?;
        self.store
            .set_raw(&Self::content_key(&id), content)
            .await?;

        Ok(artifact)
    }

    pub async fn get_artifact(&self, id: &ArtifactId) -> Result<Option<Artifact>, ArtifactError> {
        Ok(self.store.get::<Artifact>(&Self::meta_key(id)).await?)
    }

    pub async fn get_content(&self, id: &ArtifactId) -> Result<Option<Vec<u8>>, ArtifactError> {
        Ok(self.store.get_raw(&Self::content_key(id)).await?)
    }

    pub async fn verify_content(&self, id: &ArtifactId) -> Result<bool, ArtifactError> {
        let meta = self
            .get_artifact(id)
            .await?
            .ok_or_else(|| ArtifactError::NotFound(id.0.to_string()))?;
        let content = self
            .get_content(id)
            .await?
            .ok_or_else(|| ArtifactError::NotFound(id.0.to_string()))?;
        let computed = Self::compute_hash(&content);
        Ok(computed == meta.content_hash)
    }

    pub async fn delete_artifact(&self, id: &ArtifactId) -> Result<(), ArtifactError> {
        self.store.delete(&Self::meta_key(id)).await?;
        self.store.delete(&Self::content_key(id)).await?;
        Ok(())
    }

    pub async fn list_by_task(&self, task_id: &TaskId) -> Result<Vec<Artifact>, ArtifactError> {
        let all: Vec<(String, Artifact)> = self.store.scan_prefix(META_PREFIX).await?;
        Ok(all
            .into_iter()
            .map(|(_, a)| a)
            .filter(|a| a.task_id.0 == task_id.0)
            .collect())
    }

    pub async fn list_by_agent(&self, agent_id: &AgentId) -> Result<Vec<Artifact>, ArtifactError> {
        let all: Vec<(String, Artifact)> = self.store.scan_prefix(META_PREFIX).await?;
        Ok(all
            .into_iter()
            .map(|(_, a)| a)
            .filter(|a| a.agent_id.0 == agent_id.0)
            .collect())
    }

    pub async fn count(&self) -> Result<usize, ArtifactError> {
        let keys = self.store.list_keys(META_PREFIX).await?;
        Ok(keys.len())
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

