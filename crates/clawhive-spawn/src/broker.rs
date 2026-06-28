use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use clawhive_agent::{AgentStore, AgentStoreError};
use clawhive_auth::identity::IdentityService;
use clawhive_budget::BudgetService;
use clawhive_domain::{
    Agent, AgentGenome, AgentId, AgentState, AutonomyConfig, Budget, ChildSpawnPolicy, ChildSpec,
    LifecycleMode, MemoryConfig, Mission, MissionId, ModelPolicy, Permission, RuntimeConfig,
    SpawnRequest, SpawnRequestId, SpawnState, SwarmLimitsConfig, SwarmTeamSpec, TerminationPolicy,
};
use clawhive_event::{ClawHiveEvent, EventBus};

use crate::error::SpawnError;
use crate::validator::SpawnValidator;

/// `SpawnBroker` memproses spawn request end-to-end.
/// Mengkoordinasikan validasi, identity creation, agent provisioning,
/// lineage tracking, budget reservation, persistence, dan event publish.
pub struct SpawnBroker {
    limits: SwarmLimitsConfig,
    agent_store: Arc<AgentStore>,
    event_bus: Arc<dyn EventBus>,
}

impl SpawnBroker {
    #[must_use]
    pub fn new(
        limits: SwarmLimitsConfig,
        agent_store: Arc<AgentStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            limits,
            agent_store,
            event_bus,
        }
    }

    /// Full spawn pipeline:
    /// 1. Validate parent, policy, budget, depth, duplicates, permissions
    /// 2. Reserve budget dari parent
    /// 3. Create child identities dan agents
    /// 4. Persist agents ke store
    /// 5. Publish AgentSpawned events
    /// 6. Return created children
    pub async fn process_spawn_request(
        &self,
        parent: &mut Agent,
        mission: &Mission,
        request: &SpawnRequest,
        all_agents: &[Agent],
        current_depth: u32,
    ) -> Result<Vec<Agent>, SpawnError> {
        // Step 1: Validasi semua constraint
        SpawnValidator::validate(
            parent,
            mission,
            request,
            all_agents,
            current_depth,
            &self.limits,
        )?;

        let child_permissions = SpawnValidator::calculate_child_permissions(parent, request);
        let total_cost = SpawnValidator::calculate_total_cost(request);

        // Step 2: Reserve budget
        BudgetService::reserve(&mut parent.budget, total_cost).map_err(|_e| {
            SpawnError::BudgetInsufficient {
                remaining: parent.budget.remaining(),
                required: total_cost,
            }
        })?;

        // Step 3: Create child agents
        let mut children = Vec::new();
        for (i, child_spec) in request.children.iter().enumerate() {
            let perms = child_permissions.get(i).cloned().unwrap_or_default();
            let child = self.create_child_agent(parent, child_spec, perms, current_depth + 1);
            children.push(child);
        }

        // Step 4: Persist parent (budget sudah berubah) + children ke store
        self.agent_store
            .save(parent)
            .await
            .map_err(|e: AgentStoreError| SpawnError::Other(e.to_string()))?;

        self.agent_store
            .save_many(&children)
            .await
            .map_err(|e: AgentStoreError| SpawnError::Other(e.to_string()))?;

        // Step 5: Publish event untuk setiap child yang berhasil di-spawn
        let mut events = Vec::new();
        for child in &children {
            events.push(ClawHiveEvent::AgentSpawned {
                agent_id: child.id.0,
                parent_agent_id: Some(parent.id.0),
                mission_id: child.mission_id.0,
                role: child.role.clone(),
                lifecycle_mode: format!("{:?}", child.lifecycle_mode),
                timestamp: Utc::now(),
            });
        }

        // Publish fire-and-forget — tidak block jika event bus error
        if let Err(e) = self.event_bus.publish_many(events).await {
            tracing::warn!("gagal publish AgentSpawned events: {e}");
        }

        Ok(children)
    }

    fn create_child_agent(
        &self,
        parent: &Agent,
        spec: &ChildSpec,
        permissions: Vec<Permission>,
        _depth: u32,
    ) -> Agent {
        let now = Utc::now();

        let identity = IdentityService::create_agent_identity(
            &parent.organization_id,
            &parent.id,
            vec![],
            permissions.clone(),
        );

        let agent_id = AgentId(Uuid::now_v7());

        let genome = AgentGenome {
            id: format!("{}-{}", parent.genome.id, spec.role),
            version: parent.genome.version.clone(),
            role: spec.role.clone(),
            lifecycle_modes: vec![LifecycleMode::Ephemeral],
            model_policy: ModelPolicy {
                preferred_profile: spec.model_profile.clone(),
                fallback_profiles: vec![parent.genome.model_policy.preferred_profile.clone()],
                max_context_tokens: parent.genome.model_policy.max_context_tokens,
            },
            autonomy: AutonomyConfig {
                can_spawn: false,
                max_spawn_depth: 0,
                max_children: 0,
            },
            delegable_permissions: permissions.clone(),
            non_delegable_permissions: vec![],
            memory: MemoryConfig {
                default_read_scopes: parent.genome.memory.default_read_scopes.clone(),
                default_write_scope: parent.genome.memory.default_write_scope.clone(),
            },
            runtime: RuntimeConfig {
                preferred_class: parent.genome.runtime.preferred_class.clone(),
                network: parent.genome.runtime.network.clone(),
            },
            verification_required: parent.genome.verification_required,
        };

        Agent {
            id: agent_id.clone(),
            identity_id: identity.id,
            organization_id: parent.organization_id.clone(),
            mission_id: parent.mission_id.clone(),
            parent_agent_id: Some(parent.id.clone()),
            lineage_id: parent.lineage_id.clone(),
            name: format!("{}-{}", parent.name, spec.role),
            role: spec.role.clone(),
            genome,
            state: AgentState::Ready,
            lifecycle_mode: LifecycleMode::Ephemeral,
            persistent_pattern: None,
            budget: Budget {
                allocated_usd: spec.budget_usd,
                spent_usd: 0.0,
                soft_limit_usd: Some(spec.budget_usd * 0.8),
                hard_limit_usd: Some(spec.budget_usd),
                recurring_monthly_usd: None,
            },
            delegable_permissions: permissions,
            non_delegable_permissions: vec![],
            current_runtime: None,
            checkpoints: vec![],
            subscriptions: vec![],
            schedules: vec![],
            policy_bundle: parent.policy_bundle.clone(),
            turn_count: 0,
            total_cost_usd: 0.0,
            created_at: now,
            updated_at: now,
            terminated_at: None,
        }
    }

    #[must_use]
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
                name: "auto-team".into(),
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
            state: SpawnState::Approved,
            created_at: now,
            updated_at: now,
        }
    }
}

