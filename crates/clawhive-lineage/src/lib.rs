use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{
    AgentId, Lineage, LineageEntry, LineageId, MissionId,
};

#[derive(Debug, thiserror::Error)]
pub enum LineageError {
    #[error("lineage not found: {0}")]
    NotFound(String),
}

pub struct LineageService;

impl LineageService {
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
