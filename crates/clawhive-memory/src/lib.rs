#![allow(clippy::pedantic)]

pub mod admission;

use std::sync::Arc;

use chrono::Utc;

use clawhive_domain::{
    AgentId, EvidenceId, Memory, MemoryId, MemorySource, MemoryStatus, MemoryType, TaskId,
};
use clawhive_store::{Store, StoreError, StoreExt};

pub use admission::{AdmissionConfig, AdmissionPipeline, AdmissionResult};

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("memory not found: {0}")]
    NotFound(String),
    #[error("invalid memory status transition: {from} → {to}")]
    InvalidTransition {
        from: MemoryStatus,
        to: MemoryStatus,
    },
    #[error("{0}")]
    Other(String),
}

impl From<StoreError> for MemoryError {
    fn from(e: StoreError) -> Self {
        Self::Other(e.to_string())
    }
}

/// Input for storing a new memory.
pub struct StoreMemoryInput {
    pub tenant_id: String,
    pub scope: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub source_agent: AgentId,
    pub source_task: TaskId,
    pub evidence_id: Option<EvidenceId>,
    pub confidence: f64,
    pub classification: String,
}

const KEY_PREFIX: &str = "memory:";

pub struct MemoryService {
    store: Arc<dyn Store>,
    admission: AdmissionPipeline,
}

impl MemoryService {
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self::with_config(store, AdmissionConfig::default())
    }

    #[must_use]
    pub fn with_config(store: Arc<dyn Store>, config: AdmissionConfig) -> Self {
        Self {
            store,
            admission: AdmissionPipeline::new(config),
        }
    }

    /// Store a new memory.
    ///
    /// Runs the admission pipeline before persisting:
    /// - memories that pass all checks are stored as `Active`
    /// - memories that fail are stored as `Rejected`
    pub async fn store(&self, input: StoreMemoryInput) -> Memory {
        let now = Utc::now();
        let mut memory = Memory {
            id: MemoryId(uuid::Uuid::now_v7()),
            tenant_id: input.tenant_id,
            scope: input.scope,
            memory_type: input.memory_type,
            content: input.content,
            source: MemorySource {
                agent_id: input.source_agent,
                task_id: input.source_task,
                evidence_id: input.evidence_id,
            },
            confidence: input.confidence,
            classification: input.classification,
            status: MemoryStatus::Candidate,
            verified_by: vec![],
            created_at: now,
            updated_at: now,
        };

        // Run admission pipeline against existing active memories for deduplication.
        let existing = self
            .query(MemoryQuery {
                status: Some(MemoryStatus::Active),
                ..Default::default()
            })
            .await
            .unwrap_or_default();

        memory.status = match self.admission.evaluate(&memory, &existing) {
            AdmissionResult::Activated => MemoryStatus::Active,
            AdmissionResult::Rejected { reason } => {
                tracing::debug!("memory {} rejected: {}", memory.id.0, reason);
                MemoryStatus::Rejected
            }
        };
        memory.updated_at = Utc::now();

        let key = format!("{KEY_PREFIX}{}", memory.id.0);
        self.store
            .set(&key, &memory)
            .await
            .expect("MemoryService::store: store set failed");
        memory
    }

    /// Retrieve a memory by ID.
    pub async fn get(&self, memory_id: &MemoryId) -> Result<Option<Memory>, MemoryError> {
        let key = format!("{KEY_PREFIX}{}", memory_id.0);
        Ok(self.store.get::<Memory>(&key).await?)
    }

    /// Update memory content.
    ///
    /// # Errors
    /// Returns `MemoryError::NotFound` if the memory does not exist.
    pub async fn update_content(
        &self,
        memory_id: &MemoryId,
        new_content: String,
    ) -> Result<(), MemoryError> {
        let key = format!("{KEY_PREFIX}{}", memory_id.0);
        let mut memory = self
            .store
            .get::<Memory>(&key)
            .await?
            .ok_or_else(|| MemoryError::NotFound(memory_id.0.to_string()))?;

        memory.content = new_content;
        memory.updated_at = Utc::now();

        self.store.set(&key, &memory).await?;
        Ok(())
    }

    /// Transition memory status.
    ///
    /// Valid transitions:
    /// - Candidate → {Scanning, Rejected}
    /// - Scanning → {Verified, Rejected}
    /// - Verified → Active
    /// - Active → {Expired, Rejected}
    ///
    /// # Errors
    /// Returns `MemoryError::InvalidTransition` if the transition is not allowed.
    pub async fn transition_status(
        &self,
        memory_id: &MemoryId,
        new_status: MemoryStatus,
    ) -> Result<(), MemoryError> {
        let key = format!("{KEY_PREFIX}{}", memory_id.0);
        let mut memory = self
            .store
            .get::<Memory>(&key)
            .await?
            .ok_or_else(|| MemoryError::NotFound(memory_id.0.to_string()))?;

        if !Self::is_valid_transition(&memory.status, &new_status) {
            return Err(MemoryError::InvalidTransition {
                from: memory.status.clone(),
                to: new_status,
            });
        }

        memory.status = new_status;
        memory.updated_at = Utc::now();

        self.store.set(&key, &memory).await?;
        Ok(())
    }

    /// Check if a status transition is valid.
    #[must_use]
    pub fn is_valid_transition(from: &MemoryStatus, to: &MemoryStatus) -> bool {
        matches!(
            (from, to),
            (MemoryStatus::Candidate, MemoryStatus::Scanning)
                | (MemoryStatus::Candidate, MemoryStatus::Rejected)
                | (MemoryStatus::Scanning, MemoryStatus::Verified)
                | (MemoryStatus::Scanning, MemoryStatus::Rejected)
                | (MemoryStatus::Verified, MemoryStatus::Active)
                | (MemoryStatus::Active, MemoryStatus::Expired)
                | (MemoryStatus::Active, MemoryStatus::Rejected)
        )
    }

    /// Verify a memory by an agent.
    ///
    /// # Errors
    /// Returns `MemoryError::NotFound` if the memory does not exist.
    pub async fn verify(
        &self,
        memory_id: &MemoryId,
        verifier: AgentId,
    ) -> Result<(), MemoryError> {
        let key = format!("{KEY_PREFIX}{}", memory_id.0);
        let mut memory = self
            .store
            .get::<Memory>(&key)
            .await?
            .ok_or_else(|| MemoryError::NotFound(memory_id.0.to_string()))?;

        if !memory.verified_by.contains(&verifier) {
            memory.verified_by.push(verifier);
        }
        memory.updated_at = Utc::now();

        self.store.set(&key, &memory).await?;
        Ok(())
    }

    /// Query memories by filter.
    pub async fn query(&self, filter: MemoryQuery) -> Result<Vec<Memory>, MemoryError> {
        let all: Vec<(String, Memory)> = self.store.scan_prefix(KEY_PREFIX).await?;
        Ok(all
            .into_iter()
            .map(|(_, m)| m)
            .filter(|m| {
                if let Some(ref tenant) = filter.tenant_id && m.tenant_id != *tenant {
                    return false;
                }
                if let Some(ref scope) = filter.scope && m.scope != *scope {
                    return false;
                }
                if let Some(ref memory_type) = filter.memory_type && m.memory_type != *memory_type {
                    return false;
                }
                if let Some(ref status) = filter.status && m.status != *status {
                    return false;
                }
                if let Some(ref agent) = filter.source_agent && m.source.agent_id != *agent {
                    return false;
                }
                if let Some(min_confidence) = filter.min_confidence && m.confidence < min_confidence
                {
                    return false;
                }
                true
            })
            .collect())
    }

    /// Delete a memory.
    ///
    /// # Errors
    /// Returns `MemoryError::NotFound` if the memory does not exist.
    pub async fn delete(&self, memory_id: &MemoryId) -> Result<(), MemoryError> {
        let key = format!("{KEY_PREFIX}{}", memory_id.0);
        if !self.store.exists(&key).await? {
            return Err(MemoryError::NotFound(memory_id.0.to_string()));
        }
        self.store.delete(&key).await?;
        Ok(())
    }

    /// Count memories by status.
    pub async fn count_by_status(&self) -> Result<std::collections::HashMap<String, usize>, MemoryError> {
        let all: Vec<(String, Memory)> = self.store.scan_prefix(KEY_PREFIX).await?;
        let mut counts = std::collections::HashMap::new();
        for (_, memory) in all {
            *counts.entry(format!("{:?}", memory.status)).or_insert(0) += 1;
        }
        Ok(counts)
    }
}

/// Filter for querying memories.
#[derive(Debug, Default)]
pub struct MemoryQuery {
    pub tenant_id: Option<String>,
    pub scope: Option<String>,
    pub memory_type: Option<MemoryType>,
    pub status: Option<MemoryStatus>,
    pub source_agent: Option<AgentId>,
    pub min_confidence: Option<f64>,
}
