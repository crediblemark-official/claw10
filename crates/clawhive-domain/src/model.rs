use serde::{Deserialize, Serialize};

use crate::agent::AgentId;
use crate::identity::IdentityId;
use crate::mission::MissionId;
use crate::task::TaskId;
use crate::tenant::TenantId;
use crate::worker::WorkerId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: String,
    pub provider: String,
    pub model_name: String,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub cost_per_1k_input: f64,
    pub cost_per_1k_output: f64,
    pub suitable_for: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProvider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key_ref: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLevel(pub String);

impl RiskLevel {
    pub const LOW: &'static str = "low";
    pub const MEDIUM: &'static str = "medium";
    pub const HIGH: &'static str = "high";
    pub const CRITICAL: &'static str = "critical";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub mission_id: MissionId,
    pub agent_id: AgentId,
    pub severity: String,
    pub description: String,
    pub state: IncidentState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IncidentState {
    Open,
    Investigating,
    Mitigating,
    Resolved,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reputation {
    pub agent_id: AgentId,
    pub score: f64,
    pub total_tasks_completed: u64,
    pub total_tasks_accepted: u64,
    pub total_revisions: u64,
    pub average_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub channel_type: ChannelType,
    pub config: serde_json::Value,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelType {
    Terminal,
    Rest,
    Webhook,
    Email,
    Telegram,
    WhatsApp,
    Slack,
    Discord,
    Mobile,
    InternalBus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub identity_id: IdentityId,
    pub channel_id: String,
    pub state: SessionState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Idle,
    Expired,
    Terminated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmLimitsConfig {
    pub max_spawn_depth: u32,
    pub max_children_per_agent: u32,
    pub max_agents_per_mission: u32,
    pub max_concurrent_agents: u32,
    pub max_persistent_children_per_agent: u32,
    pub max_turns_per_ephemeral_agent: u32,
    pub max_idle_seconds_ephemeral: u64,
}

impl Default for SwarmLimitsConfig {
    fn default() -> Self {
        Self {
            max_spawn_depth: 3,
            max_children_per_agent: 5,
            max_agents_per_mission: 30,
            max_concurrent_agents: 12,
            max_persistent_children_per_agent: 3,
            max_turns_per_ephemeral_agent: 40,
            max_idle_seconds_ephemeral: 600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ids {
    pub tenant_id: TenantId,
    pub organization_id: super::organization::OrganizationId,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
    pub worker_id: Option<WorkerId>,
}
