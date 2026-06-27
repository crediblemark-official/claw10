use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: AuditEventId,
    pub tenant_id: String,
    pub mission_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub parent_agent_id: Option<String>,
    pub lineage_id: Option<String>,
    pub worker_id: Option<String>,
    pub trace_id: Option<String>,
    pub event_type: String,
    pub lifecycle_mode: Option<String>,
    pub risk_level: Option<String>,
    pub status: String,
    pub cost_usd: f64,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}
