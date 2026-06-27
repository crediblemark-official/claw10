use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::evidence::EvidenceId;
use crate::mission::MissionId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lineage {
    pub id: LineageId,
    pub mission_id: MissionId,
    pub root_agent_id: AgentId,
    pub entries: Vec<LineageEntry>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageEntry {
    pub agent_id: AgentId,
    pub parent_agent_id: Option<AgentId>,
    pub role: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub terminated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLegacy {
    pub agent_id: AgentId,
    pub parent_agent_id: Option<AgentId>,
    pub lineage_id: LineageId,
    pub mission_id: MissionId,
    pub lifecycle_mode: String,
    pub created_at: DateTime<Utc>,
    pub terminated_at: DateTime<Utc>,
    pub termination_reason: String,
    pub model_calls: u64,
    pub tool_calls: u64,
    pub children_created: u32,
    pub cost_usd: f64,
    pub status: String,
    pub artifact_ids: Vec<String>,
    pub evidence_ids: Vec<EvidenceId>,
    pub memory_proposed: Vec<String>,
    pub memory_accepted: Vec<String>,
    pub policy_denials: u32,
    pub anomalies: Vec<String>,
    pub trace_hash: String,
    pub signed_by: String,
}
