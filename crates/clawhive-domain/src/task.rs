use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::budget::Budget;
use crate::evidence::Evidence;
use crate::mission::MissionId;
use crate::model::RiskLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub mission_id: MissionId,
    pub parent_task_id: Option<TaskId>,
    pub owner_id: AgentId,
    pub objective: String,
    pub dependencies: Vec<TaskId>,
    pub risk: RiskLevel,
    pub budget: Budget,
    pub deadline: Option<DateTime<Utc>>,
    pub input: serde_json::Value,
    pub output_contract: serde_json::Value,
    pub evidence_contract: Vec<String>,
    pub retry_policy: RetryPolicy,
    pub idempotency_key: Option<String>,
    pub lifecycle_mode: String,
    pub state: TaskState,
    pub evidence: Vec<Evidence>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskState {
    Created,
    Ready,
    Claimed,
    PolicyCheck,
    Denied,
    AwaitingApproval,
    Running,
    Waiting,
    EvidenceSubmitted,
    Verifying,
    RevisionRequired,
    Accepted,
    Closed,
    Failed,
    Retrying,
    Escalated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependency {
    pub task_id: TaskId,
    pub depends_on: TaskId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLease {
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
