use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentId;
use crate::identity::IdentityId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    pub id: ApprovalId,
    pub target_type: ApprovalTargetType,
    pub target_id: String,
    pub requested_by: AgentId,
    pub approved_by: Option<IdentityId>,
    pub level: ApprovalLevel,
    pub reason: String,
    pub state: ApprovalState,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    pub expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalTargetType {
    SpawnRequest,
    ToolInvocation,
    PolicyActivation,
    PermissionIncrease,
    PersistentAgentCreation,
    ExternalCommunication,
    ProductionMutation,
    FinancialTransaction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalLevel {
    L0 = 0,
    L1 = 1,
    L2 = 2,
    L3 = 3,
    L4 = 4,
    L5 = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToolApprovalState {
    Pending,
    Approved,
    Denied,
    AlwaysApproved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolApprovalRequest {
    pub id: String,
    pub agent_id: AgentId,
    pub tool_name: String,
    pub command: String,
    pub state: ToolApprovalState,
    pub created_at: DateTime<Utc>,
}
