use super::*;
use claw10_domain::{AgentId, Budget, MissionId, RiskLevel};
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
