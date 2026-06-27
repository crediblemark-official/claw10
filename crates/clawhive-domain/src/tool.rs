use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::mission::MissionId;
use crate::task::TaskId;
use crate::worker::WorkerId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: ToolId,
    pub name: String,
    pub category: ToolCategory,
    pub schema: serde_json::Value,
    pub side_effect_class: SideEffectClass,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCategory {
    Filesystem,
    Shell,
    Browser,
    Http,
    Database,
    SourceControl,
    Communication,
    Document,
    Spreadsheet,
    Media,
    Cloud,
    Infrastructure,
    Hardware,
    Mcp,
    CustomApi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SideEffectClass {
    ReadOnly,
    ReversibleWrite,
    ControlledWrite,
    ExternalCommunication,
    ProductionMutation,
    Destructive,
    Physical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub id: ToolInvocationId,
    pub tool_id: ToolId,
    pub agent_id: AgentId,
    pub worker_id: WorkerId,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub idempotency_key: String,
    pub risk_level: String,
    pub approval_id: Option<String>,
    pub state: InvocationState,
    pub cost_usd: f64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocationId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InvocationState {
    Pending,
    Approved,
    Running,
    Completed,
    Failed,
    Cancelled,
}
