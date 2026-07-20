use super::*;
use uuid::Uuid;

fn make_mission() -> Mission {
    MissionService::create_mission(
        IdentityId(Uuid::now_v7()),
        "test mission".into(),
        Budget {
            allocated_usd: 100.0,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: None,
            recurring_monthly_usd: None,
        },
        RiskLevel("low".into()),
    )
}

#[test]
fn mission_pause_and_complete() {
    let mut mission = make_mission();
    assert_eq!(mission.state, MissionState::Active);

    MissionService::pause_mission(&mut mission).unwrap();
    assert_eq!(mission.state, MissionState::Paused);

    MissionService::complete_mission(&mut mission).unwrap();
    assert_eq!(mission.state, MissionState::Completed);
}

#[test]
fn mission_cannot_pause_completed() {
    let mut mission = make_mission();
    MissionService::complete_mission(&mut mission).unwrap();
    let err = MissionService::pause_mission(&mut mission).unwrap_err();
    assert!(matches!(err, MissionError::InvalidState(_)));
}

#[test]
fn mission_cancel_from_any_state() {
    let mut mission = make_mission();
    MissionService::pause_mission(&mut mission).unwrap();
    MissionService::cancel_mission(&mut mission).unwrap();
    assert_eq!(mission.state, MissionState::Cancelled);
}
