#![allow(clippy::pedantic)]

use chrono::Utc;
use uuid::Uuid;

use claw10_domain::{AgentId, Lineage, LineageEntry, LineageId, MissionId};

#[derive(Debug, thiserror::Error)]
pub enum LineageError {
    #[error("lineage not found: {0}")]
    NotFound(String),
}

pub struct LineageService;

impl LineageService {
    #[must_use]
    pub fn create_lineage(mission_id: MissionId, root_agent_id: AgentId) -> Lineage {
        let now = Utc::now();
        Lineage {
            id: LineageId(Uuid::now_v7()),
            mission_id,
            root_agent_id,
            entries: vec![],
            created_at: now,
        }
    }

    pub fn add_entry(
        lineage: &mut Lineage,
        agent_id: AgentId,
        parent_agent_id: Option<AgentId>,
        role: String,
    ) {
        let entry = LineageEntry {
            agent_id,
            parent_agent_id,
            role,
            state: "active".into(),
            created_at: Utc::now(),
            terminated_at: None,
        };
        lineage.entries.push(entry);
    }

    pub fn terminate_entry(lineage: &mut Lineage, agent_id: &AgentId) {
        for entry in &mut lineage.entries {
            if entry.agent_id == *agent_id {
                entry.state = "terminated".into();
                entry.terminated_at = Some(Utc::now());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_lineage() {
        let mission_id = MissionId(Uuid::now_v7());
        let root_agent_id = AgentId(Uuid::now_v7());

        let lineage = LineageService::create_lineage(mission_id.clone(), root_agent_id.clone());

        assert_eq!(lineage.mission_id, mission_id);
        assert_eq!(lineage.root_agent_id, root_agent_id);
        assert!(lineage.entries.is_empty());
        // Since `created_at` is generated inside `create_lineage`, we check if it is reasonably close to `Utc::now()`
        assert!(Utc::now().signed_duration_since(lineage.created_at).num_seconds() < 5);
    }
}
