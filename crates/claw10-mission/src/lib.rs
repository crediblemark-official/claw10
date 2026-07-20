#![allow(clippy::pedantic)]

use chrono::Utc;
use uuid::Uuid;

use claw10_domain::{
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
#[path = "lib_test.rs"]
mod tests;
