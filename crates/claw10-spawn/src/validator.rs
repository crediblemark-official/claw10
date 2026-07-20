use claw10_auth::rbac::RbacService;
use claw10_domain::{Agent, Mission, Permission, SpawnRequest, SwarmLimitsConfig};

use crate::error::SpawnError;

/// `SpawnValidator` checks all constraints before allowing a spawn request.
/// Implements the spawn validation pipeline from PRD section 17.
pub struct SpawnValidator;

impl SpawnValidator {
    /// Validate a spawn request against all constraints.
    /// Returns Ok(()) if all checks pass.
    #[allow(clippy::too_many_arguments)]
    pub fn validate(
        parent: &Agent,
        mission: &Mission,
        request: &SpawnRequest,
        all_agents: &[Agent],
        current_depth: u32,
        limits: &SwarmLimitsConfig,
    ) -> Result<(), SpawnError> {
        // Chain of checks - fail closed on any violation
        Self::check_parent_active(parent)?;
        Self::check_parent_spawn_permission(parent)?;
        Self::check_depth(current_depth, limits)?;
        Self::check_swarm_size(all_agents, request, limits)?;
        Self::check_duplicate_objective(request, all_agents)?;
        Self::check_child_budget(parent, request)?;
        Self::check_permissions_delegable(parent, request)?;
        Self::check_child_spawn_policy(request, current_depth)?;
        Self::check_mission_active(mission)?;

        Ok(())
    }

    /// Parent must be in Active state
    fn check_parent_active(parent: &Agent) -> Result<(), SpawnError> {
        use claw10_domain::AgentState;
        if parent.state != AgentState::Active {
            return Err(SpawnError::ParentNotActive);
        }
        Ok(())
    }

    /// Parent must have spawn permission in its genome
    fn check_parent_spawn_permission(parent: &Agent) -> Result<(), SpawnError> {
        if !parent.genome.autonomy.can_spawn {
            return Err(SpawnError::ParentCannotSpawn);
        }
        Ok(())
    }

    /// Current depth must not exceed max spawn depth
    fn check_depth(current_depth: u32, limits: &SwarmLimitsConfig) -> Result<(), SpawnError> {
        if current_depth >= limits.max_spawn_depth {
            return Err(SpawnError::DepthExceeded {
                max: limits.max_spawn_depth,
                current: current_depth,
            });
        }
        Ok(())
    }

    /// Total agents in mission + new children must not exceed limit
    fn check_swarm_size(
        all_agents: &[Agent],
        request: &SpawnRequest,
        limits: &SwarmLimitsConfig,
    ) -> Result<(), SpawnError> {
        let total = all_agents.len() + request.children.len();
        if total > limits.max_agents_per_mission as usize {
            return Err(SpawnError::SwarmSizeExceeded);
        }
        Ok(())
    }

    /// Check for duplicate objectives among existing agents.
    /// Only flags a duplicate when a child's objective exactly matches
    /// an existing agent's name (case-insensitive).
    fn check_duplicate_objective(
        request: &SpawnRequest,
        all_agents: &[Agent],
    ) -> Result<(), SpawnError> {
        for child in &request.children {
            for agent in all_agents {
                if agent.name.to_lowercase() == child.objective.to_lowercase() {
                    return Err(SpawnError::DuplicateObjective(child.objective.clone()));
                }
            }
        }
        Ok(())
    }

    /// Parent must have enough remaining budget to cover all children
    fn check_child_budget(parent: &Agent, request: &SpawnRequest) -> Result<(), SpawnError> {
        let total_child_budget: f64 = request.children.iter().map(|c| c.budget_usd).sum();
        let remaining = parent.budget.remaining();
        if remaining < total_child_budget {
            return Err(SpawnError::BudgetInsufficient {
                remaining,
                required: total_child_budget,
            });
        }
        Ok(())
    }

    /// Each child permission must be in parent's delegable permissions
    fn check_permissions_delegable(
        parent: &Agent,
        request: &SpawnRequest,
    ) -> Result<(), SpawnError> {
        for child in &request.children {
            if let Some(custom_perms) = &child.custom_permissions {
                let child_perms =
                    RbacService::child_permissions(&parent.delegable_permissions, custom_perms);
                if child_perms.len() != custom_perms.len() {
                    if let Some(missing) = custom_perms
                        .iter()
                        .find(|cp| !parent.delegable_permissions.contains(cp))
                    {
                        let missing_perm = missing.0.clone();
                        return Err(SpawnError::PermissionNotDelegable(missing_perm));
                    }
                }
            }
        }
        Ok(())
    }

    /// Validate child spawn policy constraints.
    /// - Jika `allowed=false`, children tidak boleh spawn lebih lanjut.
    /// - `max_children` membatasi jumlah children dalam request ini.
    /// - `max_depth` membatasi depth relatif dari children terhadap root.
    fn check_child_spawn_policy(
        request: &SpawnRequest,
        current_depth: u32,
    ) -> Result<(), SpawnError> {
        let policy = &request.child_spawn_policy;

        if !policy.allowed {
            // Cek apakah ada child yang request spawn permission
            for child in &request.children {
                if let Some(perms) = &child.custom_permissions {
                    let can_spawn_perm = claw10_domain::Permission("spawn".into());
                    if perms.contains(&can_spawn_perm) {
                        return Err(SpawnError::ChildSpawnDenied(format!(
                            "child role '{}' requests spawn permission but child_spawn_policy disallows it",
                            child.role
                        )));
                    }
                }
            }
        }

        if let Some(max_children) = policy.max_children {
            let requested = request.children.len() as u32;
            if requested > max_children {
                return Err(SpawnError::MaxChildrenExceeded {
                    max: max_children,
                    requested,
                });
            }
        }

        if let Some(max_depth) = policy.max_depth {
            // current_depth adalah depth parent; children akan berada di current_depth + 1
            let child_depth = current_depth + 1;
            if child_depth > max_depth {
                return Err(SpawnError::ChildSpawnDepthExceeded {
                    max: max_depth,
                    current: child_depth,
                });
            }
        }

        Ok(())
    }

    /// Mission must be active
    fn check_mission_active(mission: &Mission) -> Result<(), SpawnError> {
        use claw10_domain::MissionState;
        if mission.state != MissionState::Active {
            return Err(SpawnError::Validation("mission is not active".into()));
        }
        Ok(())
    }
}

/// Result of a successful spawn validation with calculated constraints
pub struct ValidatedSpawn {
    pub child_permissions: Vec<Vec<Permission>>,
    pub total_cost: f64,
    pub effective_depth: u32,
}

impl SpawnValidator {
    /// Calculate the effective child permissions after delegation filtering
    #[must_use]
    pub fn calculate_child_permissions(
        parent: &Agent,
        request: &SpawnRequest,
    ) -> Vec<Vec<Permission>> {
        request
            .children
            .iter()
            .map(|child| match &child.custom_permissions {
                Some(requested) => {
                    RbacService::child_permissions(&parent.delegable_permissions, requested)
                }
                None => parent.delegable_permissions.clone(),
            })
            .collect()
    }

    /// Calculate total cost of all children
    #[must_use]
    pub fn calculate_total_cost(request: &SpawnRequest) -> f64 {
        request.children.iter().map(|c| c.budget_usd).sum()
    }
}
