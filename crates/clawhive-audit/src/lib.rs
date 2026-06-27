use std::sync::Arc;

use chrono::Utc;

use clawhive_domain::{AuditEvent, AuditEventId};
use clawhive_store::{Store, StoreError, StoreExt};

const KEY_PREFIX: &str = "audit:";

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("{0}")]
    Store(#[from] StoreError),
}

pub struct AuditService {
    store: Arc<dyn Store>,
}

impl AuditService {
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    pub async fn emit_event(&self, event: AuditEvent) -> Result<AuditEvent, AuditError> {
        let mut event = event;
        let key = format!("{KEY_PREFIX}{}", event.id.0);
        event.timestamp = Utc::now();
        self.store.set(&key, &event).await?;
        Ok(event)
    }

    pub async fn get_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>, AuditError> {
        let key = format!("{KEY_PREFIX}{}", id.0);
        Ok(self.store.get::<AuditEvent>(&key).await?)
    }

    pub async fn list_all(&self) -> Result<Vec<AuditEvent>, AuditError> {
        let results: Vec<(String, AuditEvent)> = self.store.scan_prefix(KEY_PREFIX).await?;
        Ok(results.into_iter().map(|(_, e)| e).collect())
    }

    pub async fn list_by_agent(&self, agent_id: &str) -> Result<Vec<AuditEvent>, AuditError> {
        let all = self.list_all().await?;
        Ok(all.into_iter().filter(|e| e.agent_id.as_deref() == Some(agent_id)).collect())
    }

    pub async fn list_by_mission(&self, mission_id: &str) -> Result<Vec<AuditEvent>, AuditError> {
        let all = self.list_all().await?;
        Ok(all.into_iter().filter(|e| e.mission_id.as_deref() == Some(mission_id)).collect())
    }

    pub async fn list_by_task(&self, task_id: &str) -> Result<Vec<AuditEvent>, AuditError> {
        let all = self.list_all().await?;
        Ok(all.into_iter().filter(|e| e.task_id.as_deref() == Some(task_id)).collect())
    }

    pub async fn list_by_event_type(&self, event_type: &str) -> Result<Vec<AuditEvent>, AuditError> {
        let all = self.list_all().await?;
        Ok(all.into_iter().filter(|e| e.event_type == event_type).collect())
    }

    pub async fn count(&self) -> Result<usize, AuditError> {
        let keys = self.store.list_keys(KEY_PREFIX).await?;
        Ok(keys.len())
    }
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

