use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{
    AgentId, Budget, MissionId, RiskLevel, Task, TaskId, TaskState,
};

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("task not found: {0}")]
    NotFound(String),
    #[error("invalid state transition: from {from:?} to {to:?}")]
    InvalidTransition { from: TaskState, to: TaskState },
}

pub struct TaskService;

impl TaskService {
    pub fn create_task(
        mission_id: MissionId,
        owner_id: AgentId,
        objective: String,
        input: serde_json::Value,
        output_contract: serde_json::Value,
        budget: Budget,
        risk: RiskLevel,
    ) -> Task {
        let now = Utc::now();
        Task {
            id: TaskId(Uuid::now_v7()),
            mission_id,
            parent_task_id: None,
            owner_id,
            objective,
            dependencies: vec![],
            risk,
            budget,
            deadline: None,
            input,
            output_contract,
            evidence_contract: vec![],
            retry_policy: clawhive_domain::RetryPolicy {
                max_retries: 3,
                backoff_seconds: 30,
            },
            idempotency_key: None,
            lifecycle_mode: "ephemeral".into(),
            state: TaskState::Created,
            evidence: vec![],
            created_at: now,
            updated_at: now,
        }
    }

    pub fn transition(
        task: &mut Task,
        to: TaskState,
    ) -> Result<(), TaskError> {
        let from = task.state.clone();
        if !is_valid_transition(&from, &to) {
            return Err(TaskError::InvalidTransition { from, to });
        }
        task.state = to;
        task.updated_at = Utc::now();
        Ok(())
    }
}

fn is_valid_transition(from: &TaskState, to: &TaskState) -> bool {
    use TaskState::*;
    matches!(
        (from, to),
        (Created, Ready)
            | (Ready, Claimed)
            | (Claimed, PolicyCheck)
            | (PolicyCheck, Denied)
            | (PolicyCheck, AwaitingApproval)
            | (PolicyCheck, Running)
            | (AwaitingApproval, Running)
            | (AwaitingApproval, Denied)
            | (Running, Waiting)
            | (Waiting, Running)
            | (Running, EvidenceSubmitted)
            | (EvidenceSubmitted, Verifying)
            | (Verifying, RevisionRequired)
            | (RevisionRequired, Ready)
            | (Verifying, Accepted)
            | (Accepted, Closed)
            | (Running, Failed)
            | (Failed, Retrying)
            | (Retrying, Running)
            | (Failed, Escalated)
            | (Escalated, Closed)
    )
}
