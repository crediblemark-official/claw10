#![allow(clippy::pedantic)]

use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{
    Budget, IdentityId, LifecycleMode, Mission, MissionId, MissionState, RiskLevel,
};

#[derive(Debug, thiserror::Error)]
pub enum MissionError {
    #[error("mission not found: {0}")]
    NotFound(String),
    #[error("invalid state transition: {0}")]
    InvalidState(String),
}

pub struct MissionService;

impl MissionService {
    #[must_use]
    pub fn create_mission(
        owner_id: IdentityId,
        objective: String,
        budget: Budget,
        risk: RiskLevel,
    ) -> Mission {
        let now = Utc::now();
        Mission {
            id: MissionId(Uuid::now_v7()),
            owner_id,
            objective,
            scope: None,
            lifecycle_mode: LifecycleMode::Ephemeral,
            campaign_end: None,
            review_interval_days: None,
            budget,
            risk,
            require_evidence: true,
            minimum_verifiers: 1,
            state: MissionState::Active,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn pause_mission(mission: &mut Mission) -> Result<(), MissionError> {
        if mission.state != MissionState::Active {
            return Err(MissionError::InvalidState(
                "only active missions can be paused".into(),
            ));
        }
        mission.state = MissionState::Paused;
        mission.updated_at = Utc::now();
        Ok(())
    }

    pub fn complete_mission(mission: &mut Mission) -> Result<(), MissionError> {
        if mission.state != MissionState::Active && mission.state != MissionState::Paused {
            return Err(MissionError::InvalidState(
                "only active or paused missions can be completed".into(),
            ));
        }
        mission.state = MissionState::Completed;
        mission.updated_at = Utc::now();
        Ok(())
    }

    pub fn cancel_mission(mission: &mut Mission) -> Result<(), MissionError> {
        mission.state = MissionState::Cancelled;
        mission.updated_at = Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
}
