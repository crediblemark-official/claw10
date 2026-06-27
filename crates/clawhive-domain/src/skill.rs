use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::organization::Permission;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillId(pub Uuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: SkillId,
    pub name: String,
    pub purpose: String,
    pub version: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub steps: Vec<String>,
    pub required_tools: Vec<String>,
    pub required_permissions: Vec<Permission>,
    pub state: SkillState,
    pub signature: Option<String>,
    pub cost_profile: SkillCostProfile,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SkillState {
    Candidate,
    Scanning,
    Rejected,
    Testing,
    Failed,
    Review,
    Approved,
    Staged,
    Active,
    Deprecated,
    Quarantined,
    Retired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCostProfile {
    pub estimated_cost_usd: f64,
    pub average_duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVersion {
    pub skill_id: SkillId,
    pub version: String,
    pub previous_version: Option<String>,
    pub change_log: String,
    pub is_deprecated: bool,
    pub created_at: DateTime<Utc>,
}
