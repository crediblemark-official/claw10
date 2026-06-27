use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::task::TaskId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: EvidenceId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub evidence_type: EvidenceType,
    pub content: String,
    pub content_hash: String,
    pub accepted: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceType {
    File,
    Screenshot,
    TestOutput,
    CommandResult,
    ApiResult,
    DatabaseDiff,
    DeploymentHealth,
    SourceCitation,
    HumanConfirmation,
}
