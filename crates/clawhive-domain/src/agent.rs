use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::budget::Budget;
use crate::identity::IdentityId;
use crate::lifecycle::{AgentState, Checkpoint, LifecycleMode, PersistentPattern, RuntimeLease, Schedule, Subscription};
use crate::lineage::LineageId;
use crate::mission::MissionId;
use crate::organization::{OrganizationId, Permission};
use crate::policy::PolicyBundle;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AgentId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub identity_id: IdentityId,
    pub organization_id: OrganizationId,
    pub mission_id: MissionId,
    pub parent_agent_id: Option<AgentId>,
    pub lineage_id: LineageId,
    pub name: String,
    pub role: String,
    pub genome: AgentGenome,
    pub state: AgentState,
    pub lifecycle_mode: LifecycleMode,
    pub persistent_pattern: Option<PersistentPattern>,
    pub budget: Budget,
    pub delegable_permissions: Vec<Permission>,
    pub non_delegable_permissions: Vec<Permission>,
    pub current_runtime: Option<RuntimeLease>,
    pub checkpoints: Vec<Checkpoint>,
    pub subscriptions: Vec<Subscription>,
    pub schedules: Vec<Schedule>,
    pub policy_bundle: PolicyBundle,
    pub turn_count: u64,
    pub total_cost_usd: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub terminated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentGenome {
    pub id: String,
    pub version: String,
    pub role: String,
    pub lifecycle_modes: Vec<LifecycleMode>,
    pub model_policy: ModelPolicy,
    pub autonomy: AutonomyConfig,
    pub delegable_permissions: Vec<Permission>,
    pub non_delegable_permissions: Vec<Permission>,
    pub memory: MemoryConfig,
    pub runtime: RuntimeConfig,
    pub verification_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPolicy {
    pub preferred_profile: String,
    pub fallback_profiles: Vec<String>,
    pub max_context_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyConfig {
    pub can_spawn: bool,
    pub max_spawn_depth: u32,
    pub max_children: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub default_read_scopes: Vec<String>,
    pub default_write_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub preferred_class: String,
    pub network: NetworkPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkPolicy {
    AllowByDefault,
    DenyByDefault,
}
