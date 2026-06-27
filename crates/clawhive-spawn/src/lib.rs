use chrono::Utc;
use uuid::Uuid;

use clawhive_domain::{
    Agent, AgentId, ChildSpawnPolicy, ChildSpec, LifecycleMode, MissionId, Permission,
    SpawnRequest, SpawnRequestId, SpawnState, SwarmTeamSpec, TerminationPolicy,
};

#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("spawn depth exceeded: max {max}, current {current}")]
    DepthExceeded { max: u32, current: u32 },
    #[error("child limit exceeded")]
    ChildLimitExceeded,
    #[error("budget insufficient")]
    BudgetInsufficient,
    #[error("permission not delegable: {0}")]
    PermissionNotDelegable(String),
    #[error("duplicate objective: {0}")]
    DuplicateObjective(String),
    #[error("spawn not found: {0}")]
    NotFound(String),
}

pub struct SpawnBroker;

impl SpawnBroker {
    pub fn create_request(
        mission_id: MissionId,
        requested_by: AgentId,
        reason: String,
        children: Vec<ChildSpec>,
    ) -> SpawnRequest {
        let now = Utc::now();
        SpawnRequest {
            id: SpawnRequestId(Uuid::now_v7()),
            mission_id,
            task_id: None,
            requested_by,
            reason,
            team: SwarmTeamSpec {
                name: "default-team".into(),
                lifecycle_mode: LifecycleMode::Ephemeral,
                ttl_seconds: Some(7200),
                idle_timeout_seconds: Some(600),
            },
            children,
            child_spawn_policy: ChildSpawnPolicy {
                allowed: false,
                max_depth: None,
                max_children: None,
            },
            termination: TerminationPolicy {
                on_task_complete: true,
                on_parent_terminated: true,
                on_budget_exhausted: true,
            },
            state: SpawnState::Pending,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn validate_spawn_constraints(
        parent: &Agent,
        child_objectives: &[String],
        current_depth: u32,
        swarm_limits: &clawhive_domain::SwarmLimitsConfig,
    ) -> Result<(), SpawnError> {
        if current_depth >= swarm_limits.max_spawn_depth {
            return Err(SpawnError::DepthExceeded {
                max: swarm_limits.max_spawn_depth,
                current: current_depth,
            });
        }

        if !parent.genome.autonomy.can_spawn {
            return Err(SpawnError::PermissionNotDelegable(
                "agent does not have spawn permission".into(),
            ));
        }

        if child_objectives.len() > parent.genome.autonomy.max_children as usize {
            return Err(SpawnError::ChildLimitExceeded);
        }

        Ok(())
    }

    pub fn calculate_child_permissions(
        parent_delegable: &[Permission],
        requested: &[Permission],
    ) -> Vec<Permission> {
        let parent_set: std::collections::HashSet<&Permission> =
            parent_delegable.iter().collect();
        requested
            .iter()
            .filter(|p| parent_set.contains(*p))
            .cloned()
            .collect()
    }
}
