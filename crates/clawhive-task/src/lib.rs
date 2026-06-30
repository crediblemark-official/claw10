#![allow(clippy::pedantic)]

use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{AgentId, Budget, MissionId, RiskLevel, Task, TaskId, TaskState};

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("task not found: {0}")]
    NotFound(String),
    #[error("invalid state transition: from {from:?} to {to:?}")]
    InvalidTransition { from: TaskState, to: TaskState },
}

pub struct TaskService;

impl TaskService {
    #[must_use]
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

    pub fn transition(task: &mut Task, to: TaskState) -> Result<(), TaskError> {
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
    use TaskState::{
        Accepted, AwaitingApproval, Claimed, Closed, Created, Denied, Escalated, EvidenceSubmitted,
        Failed, PolicyCheck, Ready, Retrying, RevisionRequired, Running, Verifying, Waiting,
    };
    matches!(
        (from, to),
        (Created | RevisionRequired, Ready)
            | (Ready, Claimed)
            | (Claimed, PolicyCheck)
            | (PolicyCheck | AwaitingApproval, Denied)
            | (PolicyCheck, AwaitingApproval | Running)
            | (AwaitingApproval | Waiting | Retrying, Running)
            | (Running, Waiting | EvidenceSubmitted | Failed)
            | (EvidenceSubmitted, Verifying)
            | (Verifying, RevisionRequired | Accepted)
            | (Accepted | Escalated, Closed)
            | (Failed, Retrying | Escalated)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawhive_domain::{AgentId, Budget, MissionId, RiskLevel};
    use uuid::Uuid;

    fn make_task() -> Task {
        TaskService::create_task(
            MissionId(Uuid::now_v7()),
            AgentId(Uuid::now_v7()),
            "test objective".into(),
            serde_json::Value::Null,
            serde_json::Value::Null,
            Budget {
                allocated_usd: 10.0,
                spent_usd: 0.0,
                soft_limit_usd: None,
                hard_limit_usd: None,
                recurring_monthly_usd: None,
            },
            RiskLevel("low".into()),
        )
    }

    #[test]
    fn task_created_to_ready_is_valid() {
        let mut task = make_task();
        assert_eq!(task.state, TaskState::Created);
        TaskService::transition(&mut task, TaskState::Ready).unwrap();
        assert_eq!(task.state, TaskState::Ready);
    }

    #[test]
    fn task_created_to_running_is_invalid() {
        let mut task = make_task();
        let err = TaskService::transition(&mut task, TaskState::Running).unwrap_err();
        assert!(matches!(err, TaskError::InvalidTransition { .. }));
    }

    #[test]
    fn task_full_lifecycle_to_closed() {
        let mut task = make_task();
        TaskService::transition(&mut task, TaskState::Ready).unwrap();
        TaskService::transition(&mut task, TaskState::Claimed).unwrap();
        TaskService::transition(&mut task, TaskState::PolicyCheck).unwrap();
        TaskService::transition(&mut task, TaskState::Running).unwrap();
        TaskService::transition(&mut task, TaskState::EvidenceSubmitted).unwrap();
        TaskService::transition(&mut task, TaskState::Verifying).unwrap();
        TaskService::transition(&mut task, TaskState::Accepted).unwrap();
        TaskService::transition(&mut task, TaskState::Closed).unwrap();
        assert_eq!(task.state, TaskState::Closed);
    }
}
