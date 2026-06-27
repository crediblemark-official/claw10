use std::sync::Arc;

use chrono::Utc;

use clawhive_domain::{
    Permission, Skill, SkillCostProfile, SkillId, SkillState, SkillVersion,
};
use clawhive_store::{Store, StoreError, StoreExt};

const KEY_PREFIX: &str = "skill:";
const VERSION_PREFIX: &str = "skill:ver:";

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("skill not found: {0}")]
    NotFound(String),
    #[error("invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition { from: SkillState, to: SkillState },
    #[error("unsigned skills cannot be made active")]
    Unsigned,
    #[error("{0}")]
    Store(#[from] StoreError),
}

pub struct SkillService {
    store: Arc<dyn Store>,
}

impl SkillService {
    #[must_use]
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Create a new skill in Candidate state.
    pub async fn create_skill(
        &self,
        name: String,
        purpose: String,
        version: String,
        input_schema: serde_json::Value,
        output_schema: serde_json::Value,
        steps: Vec<String>,
        required_tools: Vec<String>,
        required_permissions: Vec<Permission>,
        cost_profile: SkillCostProfile,
    ) -> Result<Skill, SkillError> {
        let now = Utc::now();
        let skill = Skill {
            id: SkillId(uuid::Uuid::now_v7()),
            name,
            purpose,
            version,
            input_schema,
            output_schema,
            steps,
            required_tools,
            required_permissions,
            state: SkillState::Candidate,
            signature: None,
            cost_profile,
            created_at: now,
            updated_at: now,
        };
        let key = format!("{KEY_PREFIX}{}", skill.id.0);
        self.store.set(&key, &skill).await?;
        Ok(skill)
    }

    /// Transition a skill through its lifecycle states.
    ///
    /// Valid transitions:
    /// Candidate -> Scanning
    /// Scanning -> Testing | Rejected
    /// Testing -> Review | Failed
    /// Review -> Approved | Rejected
    /// Approved -> Staged
    /// Staged -> Active (requires signature)
    /// Active -> Deprecated | Quarantined
    /// Deprecated -> Retired | Active
    /// Quarantined -> Scanning | Retired
    pub async fn transition_state(
        &self,
        id: &SkillId,
        to: SkillState,
    ) -> Result<Skill, SkillError> {
        let key = format!("{KEY_PREFIX}{}", id.0);
        let mut skill = self
            .store
            .get::<Skill>(&key)
            .await?
            .ok_or_else(|| SkillError::NotFound(id.0.to_string()))?;

        if !is_valid_transition(&skill.state, &to) {
            return Err(SkillError::InvalidTransition {
                from: skill.state,
                to: to.clone(),
            });
        }

        // Active requires signature (FR-064)
        if to == SkillState::Active && skill.signature.is_none() {
            return Err(SkillError::Unsigned);
        }

        skill.state = to;
        skill.updated_at = Utc::now();
        self.store.set(&key, &skill).await?;
        Ok(skill)
    }

    /// Sign a skill so it can be activated.
    pub async fn sign_skill(
        &self,
        id: &SkillId,
        signature: String,
    ) -> Result<Skill, SkillError> {
        let key = format!("{KEY_PREFIX}{}", id.0);
        let mut skill = self
            .store
            .get::<Skill>(&key)
            .await?
            .ok_or_else(|| SkillError::NotFound(id.0.to_string()))?;

        skill.signature = Some(signature);
        skill.updated_at = Utc::now();
        self.store.set(&key, &skill).await?;
        Ok(skill)
    }

    /// Record a new version entry for this skill.
    pub async fn create_version(
        &self,
        skill_id: &SkillId,
        version: String,
        previous_version: Option<String>,
        change_log: String,
    ) -> Result<SkillVersion, SkillError> {
        let skill = self
            .get_skill(skill_id)
            .await?
            .ok_or_else(|| SkillError::NotFound(skill_id.0.to_string()))?;

        let sv = SkillVersion {
            skill_id: skill.id,
            version,
            previous_version,
            change_log,
            is_deprecated: false,
            created_at: Utc::now(),
        };
        let key = format!("{VERSION_PREFIX}{}:{}", sv.skill_id.0, sv.version);
        self.store.set(&key, &sv).await?;
        Ok(sv)
    }

    pub async fn get_skill(&self, id: &SkillId) -> Result<Option<Skill>, SkillError> {
        let key = format!("{KEY_PREFIX}{}", id.0);
        Ok(self.store.get::<Skill>(&key).await?)
    }

    pub async fn list_skills(
        &self,
        state_filter: Option<SkillState>,
    ) -> Result<Vec<Skill>, SkillError> {
        let all: Vec<(String, Skill)> = self.store.scan_prefix(KEY_PREFIX).await?;
        Ok(all
            .into_iter()
            .map(|(_, s)| s)
            .filter(|s| match &state_filter {
                Some(state) => &s.state == state,
                None => true,
            })
            .collect())
    }

    pub async fn list_by_required_tool(&self, tool_name: &str) -> Result<Vec<Skill>, SkillError> {
        let all: Vec<(String, Skill)> = self.store.scan_prefix(KEY_PREFIX).await?;
        Ok(all
            .into_iter()
            .map(|(_, s)| s)
            .filter(|s| s.required_tools.iter().any(|t| t == tool_name))
            .collect())
    }

    pub async fn count_by_state(
        &self,
    ) -> Result<std::collections::HashMap<String, usize>, SkillError> {
        let all: Vec<(String, Skill)> = self.store.scan_prefix(KEY_PREFIX).await?;
        let mut counts = std::collections::HashMap::new();
        for (_, skill) in all {
            *counts.entry(format!("{:?}", skill.state)).or_insert(0) += 1;
        }
        Ok(counts)
    }
}

fn is_valid_transition(from: &SkillState, to: &SkillState) -> bool {
    use SkillState::*;
    matches!(
        (from, to),
        (Candidate, Scanning)
            | (Scanning, Testing)
            | (Scanning, Rejected)
            | (Testing, Review)
            | (Testing, Failed)
            | (Review, Approved)
            | (Review, Rejected)
            | (Approved, Staged)
            | (Staged, Active)
            | (Active, Deprecated)
            | (Active, Quarantined)
            | (Deprecated, Retired)
            | (Deprecated, Active)
            | (Quarantined, Scanning)
            | (Quarantined, Retired)
    )
}

#[cfg(test)]
#[path = "lib_test.rs"]
mod tests;

