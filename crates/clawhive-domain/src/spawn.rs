use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::lifecycle::LifecycleMode;
use crate::mission::MissionId;
use crate::organization::Permission;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRequestId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRequest {
    pub id: SpawnRequestId,
    pub mission_id: MissionId,
    pub task_id: Option<String>,
    pub requested_by: AgentId,
    pub reason: String,
    pub team: SwarmTeamSpec,
    pub children: Vec<ChildSpec>,
    pub child_spawn_policy: ChildSpawnPolicy,
    pub termination: TerminationPolicy,
    pub state: SpawnState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTeamSpec {
    pub name: String,
    pub lifecycle_mode: LifecycleMode,
    pub ttl_seconds: Option<u64>,
    pub idle_timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildSpec {
    pub role: String,
    pub objective: String,
    pub budget_usd: f64,
    pub model_profile: String,
    pub max_turns: u32,
    pub custom_permissions: Option<Vec<Permission>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildSpawnPolicy {
    pub allowed: bool,
    pub max_depth: Option<u32>,
    pub max_children: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationPolicy {
    pub on_task_complete: bool,
    pub on_parent_terminated: bool,
    pub on_budget_exhausted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SpawnState {
    Pending,
    Validating,
    Approved,
    Denied,
    Provisioning,
    Completed,
    Failed,
}
