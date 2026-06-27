use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::evidence::EvidenceId;
use crate::task::TaskId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: MemoryId,
    pub tenant_id: String,
    pub scope: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub source: MemorySource,
    pub confidence: f64,
    pub classification: String,
    pub status: MemoryStatus,
    pub verified_by: Vec<AgentId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryType {
    Working,
    Episodic,
    Semantic,
    Procedural,
    User,
    Organization,
    Mission,
    AgentContinuity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySource {
    pub agent_id: AgentId,
    pub task_id: TaskId,
    pub evidence_id: Option<EvidenceId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryStatus {
    Candidate,
    Scanning,
    Verified,
    Active,
    Rejected,
    Expired,
}
