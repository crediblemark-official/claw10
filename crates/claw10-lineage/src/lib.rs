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
    use uuid::Uuid;

    #[test]
    fn test_add_entry() {
        let mission_id = MissionId(Uuid::now_v7());
        let root_agent_id = AgentId(Uuid::now_v7());
        let mut lineage = LineageService::create_lineage(mission_id, root_agent_id.clone());

        assert_eq!(lineage.entries.len(), 0);

        LineageService::add_entry(
            &mut lineage,
            root_agent_id.clone(),
            None,
            "root".into(),
        );

        assert_eq!(lineage.entries.len(), 1);
        let entry = &lineage.entries[0];
        assert_eq!(entry.agent_id, root_agent_id);
        assert_eq!(entry.parent_agent_id, None);
        assert_eq!(entry.role, "root");
        assert_eq!(entry.state, "active");
        assert!(entry.terminated_at.is_none());

        let child_agent_id = AgentId(Uuid::now_v7());
        LineageService::add_entry(
            &mut lineage,
            child_agent_id.clone(),
            Some(root_agent_id.clone()),
            "child".into(),
        );

        assert_eq!(lineage.entries.len(), 2);
        let child_entry = &lineage.entries[1];
        assert_eq!(child_entry.agent_id, child_agent_id);
        assert_eq!(child_entry.parent_agent_id, Some(root_agent_id));
        assert_eq!(child_entry.role, "child");
        assert_eq!(child_entry.state, "active");
        assert!(child_entry.terminated_at.is_none());
    }
}
