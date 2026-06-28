//! Persistensi agent ke/dari `Arc<dyn Store>`.
//!
//! Setiap agent disimpan dengan key `agent:<uuid>`.
//! Prefix scan digunakan untuk list dan query agent per mission.

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{Agent, AgentId, AgentState, LifecycleMode, MissionId};
use clawhive_store::{Store, StoreError, StoreExt};

#[derive(Debug, thiserror::Error)]
pub enum AgentStoreError {
    #[error("agent tidak ditemukan: {0}")]
    NotFound(String),

    #[error("store error: {0}")]
    Store(#[from] StoreError),
}

const KEY_PREFIX: &str = "agent:";

/// Filter untuk query agent.
#[derive(Debug, Default)]
pub struct AgentQuery {
    pub mission_id: Option<MissionId>,
    pub state: Option<AgentState>,
    pub lifecycle_mode: Option<LifecycleMode>,
    pub parent_agent_id: Option<AgentId>,
}

/// Service persistensi agent.
pub struct AgentStore {
    store: Arc<dyn Store>,
}

impl AgentStore {
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    fn key(id: &AgentId) -> String {
        format!("{KEY_PREFIX}{}", id.0)
    }

    /// Simpan atau update agent.
    pub async fn save(&self, agent: &Agent) -> Result<(), AgentStoreError> {
        self.store.set(&Self::key(&agent.id), agent).await?;
        Ok(())
    }

    /// Ambil agent by ID.
    pub async fn get(&self, id: &AgentId) -> Result<Option<Agent>, AgentStoreError> {
        Ok(self.store.get::<Agent>(&Self::key(id)).await?)
    }

    /// Ambil agent by ID, error jika tidak ada.
    pub async fn get_or_not_found(&self, id: &AgentId) -> Result<Agent, AgentStoreError> {
        self.get(id)
            .await?
            .ok_or_else(|| AgentStoreError::NotFound(id.0.to_string()))
    }

    /// Hapus agent dari store.
    pub async fn delete(&self, id: &AgentId) -> Result<(), AgentStoreError> {
        self.store.delete(&Self::key(id)).await?;
        Ok(())
    }

    /// List semua agent, dengan optional filter.
    pub async fn list(&self, query: AgentQuery) -> Result<Vec<Agent>, AgentStoreError> {
        let all: Vec<(String, Agent)> = self.store.scan_prefix(KEY_PREFIX).await?;

        let agents = all
            .into_iter()
            .map(|(_, a)| a)
            .filter(|a| {
                if let Some(ref mission_id) = query.mission_id {
                    if a.mission_id != *mission_id {
                        return false;
                    }
                }
                if let Some(ref state) = query.state {
                    if a.state != *state {
                        return false;
                    }
                }
                if let Some(ref mode) = query.lifecycle_mode {
                    if a.lifecycle_mode != *mode {
                        return false;
                    }
                }
                if let Some(ref parent_id) = query.parent_agent_id {
                    match &a.parent_agent_id {
                        Some(pid) => {
                            if pid != parent_id {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                true
            })
            .collect();

        Ok(agents)
    }

    /// Simpan batch agents sekaligus (digunakan setelah spawn).
    pub async fn save_many(&self, agents: &[Agent]) -> Result<(), AgentStoreError> {
        for agent in agents {
            self.save(agent).await?;
        }
        Ok(())
    }

    /// Update state agent tanpa load penuh (read-modify-write).
    pub async fn update_state(
        &self,
        id: &AgentId,
        new_state: AgentState,
    ) -> Result<(), AgentStoreError> {
        let mut agent = self.get_or_not_found(id).await?;
        agent.state = new_state;
        agent.updated_at = Utc::now();
        self.save(&agent).await
    }

    /// Buat AgentId baru (UUID v7).
    #[must_use]
    pub fn new_id() -> AgentId {
        AgentId(Uuid::now_v7())
    }

    /// Ambil referensi ke database store dasar.
    pub fn store(&self) -> &Arc<dyn clawhive_store::Store> {
        &self.store
    }
}

#[cfg(test)]
#[path = "store_test.rs"]
mod tests;


