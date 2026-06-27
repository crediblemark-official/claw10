use clawhive_domain::{AgentId, MissionId, TaskId, WorkerId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContext {
    pub tenant_id: String,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub worker_id: WorkerId,
    pub idempotency_key: String,
    pub risk_level: String,
    pub approval_id: Option<String>,
    pub budget_remaining: f64,
    pub workspace_dir: String,
}
