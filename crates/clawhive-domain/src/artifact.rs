use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::task::TaskId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub storage_path: String,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}
