use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::budget::Budget;
use crate::identity::IdentityId;
use crate::lifecycle::LifecycleMode;
use crate::organization::OrganizationId;
use crate::model::RiskLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mission {
    pub id: MissionId,
    pub organization_id: OrganizationId,
    pub owner_id: IdentityId,
    pub objective: String,
    pub scope: Option<String>,
    pub lifecycle_mode: LifecycleMode,
    pub campaign_end: Option<DateTime<Utc>>,
    pub review_interval_days: Option<u32>,
    pub budget: Budget,
    pub risk: RiskLevel,
    pub require_evidence: bool,
    pub minimum_verifiers: u32,
    pub state: MissionState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MissionState {
    Active,
    Paused,
    Completed,
    Cancelled,
}
