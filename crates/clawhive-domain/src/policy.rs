use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBundleId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBundle {
    pub id: PolicyBundleId,
    pub name: String,
    pub version: String,
    pub rules: Vec<PolicyRule>,
    pub is_active: bool,
    pub signed_by: Option<String>,
    pub signature: Option<String>,
    pub activated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: PolicyRuleId,
    pub subject: PolicySubject,
    pub effect: PolicyEffect,
    pub action: String,
    pub resource: String,
    pub condition: Option<serde_json::Value>,
    pub priority: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRuleId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicySubject {
    Tenant(String),
    Organization(String),
    Department(String),
    Role(String),
    Agent(String),
    Mission(String),
    Task(String),
    Tool(String),
    Worker(String),
    DataClass(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyEffect {
    ExplicitDeny,
    Allow,
    ExplicitDenyPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEvaluateResult {
    pub allowed: bool,
    pub matched_rule: Option<PolicyRule>,
    pub evaluation_time_ms: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmLimits {
    Default,
    Custom {
        max_spawn_depth: u32,
        max_children_per_agent: u32,
        max_agents_per_mission: u32,
        max_concurrent_agents: u32,
        max_persistent_children_per_agent: u32,
        max_turns_per_ephemeral_agent: u32,
        max_idle_seconds_ephemeral: u64,
    },
}

impl Default for SwarmLimits {
    fn default() -> Self {
        Self::Custom {
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
