use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Display, EnumString)]
pub enum LifecycleMode {
    Ephemeral,
    Persistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Display, EnumString)]
pub enum AgentState {
    Draft,
    Validating,
    Rejected,
    Ready,
    Active,
    Hibernating,
    Paused,
    Degraded,
    Quarantined,
    Completing,
    PreservingTrace,
    Terminating,
    Terminated,
    Migrating,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Display, EnumString)]
pub enum PersistentPattern {
    AlwaysOn,
    Scheduled,
    Campaign,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLease {
    pub worker_id: String,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub renewal_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub agent_id: String,
    pub state_snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub reason: CheckpointReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckpointReason {
    StateTransition,
    ToolSideEffect,
    PreHibernation,
    PreMigration,
    PreUpgrade,
    Periodic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub event_type: String,
    pub filter: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub cron: String,
    pub timezone: String,
    pub action: ScheduleAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleAction {
    Wake,
    Review,
    Checkpoint,
    PolicyRenewal,
    CredentialRotation,
}
