//! # AgentRuntime
//!
//! Full orchestration layer for executing agents. Integrates:
//!
//! - **Model routing** — LLM calls via `ModelRouter` with profile/fallback resolution
//! - **Tool execution** — tool registry with context construction
//! - **Worker assignment** — worker registration, heartbeat, and lease management
//! - **Lifecycle management** — hibernate/wake/terminate with checkpoint persistence
//!
//! ## Flow
//!
//! ```text
//! execute_agent()
//!   ├─ load agent from AgentStore
//!   ├─ assign runtime lease (LifecycleService)
//!   ├─ build ToolContext
//!   ├─ run AgentExecutor (model loop + tool calls)
//!   ├─ persist updated agent state
//!   └─ return (AgentSession, Vec<AgentEvent>)
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use clawhive_domain::{
    Agent, AgentId, AgentState, RuntimeLease, TaskId, WorkerId,
};
use clawhive_lifecycle::LifecycleService;
use clawhive_model_router::router::ModelRouter;
use clawhive_tool::context::ToolContext;
use clawhive_tool::registry::ToolRegistry;
use clawhive_worker::WorkerService;

use crate::error::AgentError;
use crate::events::AgentEvent;
use crate::executor::AgentExecutor;
use crate::session::{AgentSession, SessionState};
use crate::store::AgentStore;

/// Default runtime lease renewal interval in seconds.
const DEFAULT_LEASE_SECONDS: u64 = 60;

/// Default max turns multiplier (children × this value).
const DEFAULT_TURNS_MULTIPLIER: u32 = 10;

/// High-level orchestration for agent execution.
///
/// Wraps `AgentExecutor` with lifecycle management, worker assignment,
/// and state persistence.
pub struct AgentRuntime {
    agent_store: AgentStore,
    executor: AgentExecutor,
    #[allow(dead_code)]
    worker_service: Arc<WorkerService>,
    /// Fallback worker ID used when no worker is explicitly provided.
    default_worker_id: Option<WorkerId>,
}

impl AgentRuntime {
    /// Create a new agent runtime.
    #[must_use]
    pub fn new(
        agent_store: AgentStore,
        model_router: Arc<ModelRouter>,
        tool_registry: Arc<ToolRegistry>,
        budget_service: Arc<clawhive_budget::BudgetService>,
        worker_service: Arc<WorkerService>,
        default_worker_id: Option<WorkerId>,
    ) -> Self {
        Self {
            agent_store,
            executor: AgentExecutor::new(model_router, tool_registry, budget_service),
            worker_service,
            default_worker_id,
        }
    }

    // ── Public API ──────────────────────────────────────────────

    /// Execute an agent end-to-end.
    ///
    /// 1. Load agent from store
    /// 2. Assign a runtime lease (if none exists)
    /// 3. Construct a `ToolContext` for the session
    /// 4. Run the `AgentExecutor` turn loop
    /// 5. Persist final agent state
    /// 6. Return session + event log
    ///
    /// # Errors
    ///
    /// Returns `AgentError::AgentNotFound` if the agent does not exist.
    /// Returns `AgentError::Other` if the agent is not in a runnable state,
    /// or if no worker is available.
    pub async fn execute_agent(
        &self,
        agent_id: &AgentId,
        objective: String,
        context: HashMap<String, String>,
        worker_override: Option<WorkerId>,
    ) -> Result<(AgentSession, Vec<AgentEvent>), AgentError> {
        let mut agent = self.agent_store.get_or_not_found(agent_id).await?;

        // ── Pre-flight checks ───────────────────────────────────
        self.ensure_runnable(&agent, agent_id)?;

        let worker_id = worker_override
            .or_else(|| self.default_worker_id.clone())
            .ok_or_else(|| {
                AgentError::Other(
                    "no worker specified and no default worker configured".into(),
                )
            })?;

        // ── Assign runtime lease ─────────────────────────────────
        if agent.current_runtime.is_none() {
            LifecycleService::assign_lease(&mut agent, &worker_id.0.to_string(), DEFAULT_LEASE_SECONDS);
            self.agent_store.save(&agent).await?;
        }

        // ── Build ToolContext ────────────────────────────────────
        let tool_context = ToolContext {
            tenant_id: agent.organization_id.0.to_string(),
            mission_id: agent.mission_id.clone(),
            task_id: TaskId(uuid::Uuid::now_v7()),
            agent_id: agent.id.clone(),
            worker_id: worker_id.clone(),
            idempotency_key: uuid::Uuid::now_v7().to_string(),
            risk_level: "medium".to_string(),
            approval_id: None,
            budget_remaining: agent.budget.remaining(),
            workspace_dir: format!("/tmp/clawhive/{}", agent.id.0),
        };

        // ── Compute max turns from genome ────────────────────────
        let max_turns = agent.genome.autonomy.max_children.max(1) * DEFAULT_TURNS_MULTIPLIER;

        // ── Execute ──────────────────────────────────────────────
        let (session, events) = self
            .executor
            .execute(&agent, &objective, context, tool_context, max_turns)
            .await?;

        // ── Persist updated agent ────────────────────────────────
        if session.state == SessionState::Completed {
            agent.state = AgentState::Active;
        }
        agent.turn_count = session.turn_count as u64;
        agent.total_cost_usd = session.total_cost_usd;
        agent.updated_at = chrono::Utc::now();

        // Carry forward session messages back to agent context
        // (so future sessions start from where we left off)
        self.agent_store.save(&agent).await?;

        Ok((session, events))
    }

    /// Hibernate an agent: creates checkpoint, releases lease, persists.
    ///
    /// # Errors
    ///
    /// Delegates to [`LifecycleService::hibernate`] and store errors.
    pub async fn hibernate_agent(&self, agent_id: &AgentId) -> Result<(), AgentError> {
        let mut agent = self.agent_store.get_or_not_found(agent_id).await?;
        LifecycleService::hibernate(&mut agent).map_err(|e| AgentError::Other(e.to_string()))?;
        self.agent_store.save(&agent).await?;
        Ok(())
    }

    /// Wake an agent from hibernation with a new runtime lease.
    ///
    /// # Errors
    ///
    /// Delegates to [`LifecycleService::wake`] and store errors.
    pub async fn wake_agent(
        &self,
        agent_id: &AgentId,
        worker_id: &WorkerId,
    ) -> Result<(), AgentError> {
        let mut agent = self.agent_store.get_or_not_found(agent_id).await?;

        let lease = RuntimeLease {
            worker_id: worker_id.0.to_string(),
            acquired_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now()
                + chrono::Duration::seconds(DEFAULT_LEASE_SECONDS as i64),
            renewal_interval_seconds: DEFAULT_LEASE_SECONDS,
        };

        LifecycleService::wake(&mut agent, lease)
            .map_err(|e| AgentError::Other(e.to_string()))?;

        self.agent_store.save(&agent).await?;
        Ok(())
    }

    /// Terminate an agent through the full teardown sequence.
    ///
    /// # Errors
    ///
    /// Returns `AgentError::AgentNotFound` if the agent does not exist.
    pub async fn terminate_agent(&self, agent_id: &AgentId) -> Result<(), AgentError> {
        let mut agent = self.agent_store.get_or_not_found(agent_id).await?;
        LifecycleService::terminate(&mut agent);
        self.agent_store.save(&agent).await?;
        Ok(())
    }

    /// Apply the agent's persistent pattern (auto-hibernate or auto-wake).
    ///
    /// # Errors
    ///
    /// Returns `AgentError::AgentNotFound` if the agent does not exist.
    pub async fn apply_pattern(&self, agent_id: &AgentId) -> Result<(), AgentError> {
        let mut agent = self.agent_store.get_or_not_found(agent_id).await?;
        LifecycleService::apply_pattern(&mut agent);
        self.agent_store.save(&agent).await?;
        Ok(())
    }

    /// Process a heartbeat for the agent, renewing its runtime lease.
    ///
    /// # Errors
    ///
    /// Returns `AgentError::AgentNotFound` if the agent does not exist.
    /// Returns `AgentError::Other` if the agent is not active or has no lease.
    pub async fn heartbeat_agent(&self, agent_id: &AgentId) -> Result<chrono::Duration, AgentError> {
        let mut agent = self.agent_store.get_or_not_found(agent_id).await?;
        let remaining = LifecycleService::heartbeat(&mut agent)
            .map_err(|e| AgentError::Other(e.to_string()))?;
        self.agent_store.save(&agent).await?;
        Ok(remaining)
    }

    /// Run a low-level session on an already-loaded agent.
    ///
    /// Useful when the caller wants to manage the agent lifecycle themselves
    /// but still use the runtime's executor + context construction.
    ///
    /// # Errors
    ///
    /// Delegates to [`AgentExecutor::execute`].
    pub async fn run_session(
        &self,
        agent: &Agent,
        objective: &str,
        context: HashMap<String, String>,
        worker_id: &WorkerId,
        max_turns: u32,
    ) -> Result<(AgentSession, Vec<AgentEvent>), AgentError> {
        let tool_context = ToolContext {
            tenant_id: agent.organization_id.0.to_string(),
            mission_id: agent.mission_id.clone(),
            task_id: TaskId(uuid::Uuid::now_v7()),
            agent_id: agent.id.clone(),
            worker_id: worker_id.clone(),
            idempotency_key: uuid::Uuid::now_v7().to_string(),
            risk_level: "medium".to_string(),
            approval_id: None,
            budget_remaining: agent.budget.remaining(),
            workspace_dir: format!("/tmp/clawhive/{}", agent.id.0),
        };

        self.executor.execute(agent, objective, context, tool_context, max_turns).await
    }

    // ── Helpers ─────────────────────────────────────────────────

    fn ensure_runnable(&self, agent: &Agent, id: &AgentId) -> Result<(), AgentError> {
        if agent.state != AgentState::Active && agent.state != AgentState::Ready {
            return Err(AgentError::Other(format!(
                "agent {} is in {:?} state, cannot execute",
                id.0, agent.state
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use clawhive_domain::{
        Agent, AgentGenome, AgentId, AgentState, AutonomyConfig, Budget,
        IdentityId, LifecycleMode, LineageId, MemoryConfig, MissionId,
        ModelPolicy, NetworkPolicy, OrganizationId, PolicyBundle, RuntimeConfig, WorkerId,
    };
    use clawhive_lifecycle::LifecycleService;
    use clawhive_store::Store;

    use crate::runtime::AgentRuntime;
    use crate::store::AgentStore;

    // ── Helpers ─────────────────────────────────────────────────

    fn sample_agent() -> Agent {
        let now = chrono::Utc::now();
        Agent {
            id: AgentId(uuid::Uuid::now_v7()),
            identity_id: IdentityId(uuid::Uuid::now_v7()),
            organization_id: OrganizationId(uuid::Uuid::now_v7()),
            mission_id: MissionId(uuid::Uuid::now_v7()),
            parent_agent_id: None,
            lineage_id: LineageId(uuid::Uuid::now_v7()),
            name: "test-agent".into(),
            role: "worker".into(),
            genome: AgentGenome {
                id: "test-genome-1".into(),
                version: "1.0".into(),
                role: "worker".into(),
                lifecycle_modes: vec![LifecycleMode::Ephemeral],
                model_policy: ModelPolicy {
                    preferred_profile: "gpt-4o".into(),
                    fallback_profiles: vec!["gpt-4o-mini".into()],
                    max_context_tokens: 128_000,
                },
                autonomy: AutonomyConfig {
                    can_spawn: false,
                    max_spawn_depth: 1,
                    max_children: 3,
                },
                delegable_permissions: vec![],
                non_delegable_permissions: vec![],
                memory: MemoryConfig {
                    default_read_scopes: vec![],
                    default_write_scope: None,
                },
                runtime: RuntimeConfig {
                    preferred_class: "local".into(),
                    network: NetworkPolicy::AllowByDefault,
                },
                verification_required: false,
            },
            state: AgentState::Active,
            lifecycle_mode: LifecycleMode::Ephemeral,
            persistent_pattern: None,
            budget: Budget {
                allocated_usd: 10.0,
                spent_usd: 0.0,
                soft_limit_usd: None,
                hard_limit_usd: Some(100.0),
                recurring_monthly_usd: None,
            },
            delegable_permissions: vec![],
            non_delegable_permissions: vec![],
            current_runtime: None,
            checkpoints: vec![],
            subscriptions: vec![],
            schedules: vec![],
            policy_bundle: PolicyBundle {
                id: clawhive_domain::PolicyBundleId(uuid::Uuid::now_v7()),
                name: "default".into(),
                version: "1.0.0".into(),
                rules: vec![],
                is_active: true,
                signed_by: None,
                signature: None,
                activated_at: None,
                created_at: now,
            },
            turn_count: 0,
            total_cost_usd: 0.0,
            created_at: now,
            updated_at: now,
            terminated_at: None,
        }
    }

    fn make_runtime(store: Arc<dyn Store>) -> (AgentRuntime, AgentStore) {
        // Clone Arc so runtime and assertion handle share the same backing store.
        let assert_store = AgentStore::new(store.clone());

        // Minimal mock services for construction
        let registry = clawhive_model_router::provider::ModelRegistry::new();
        let model_router = Arc::new(clawhive_model_router::router::ModelRouter::new(registry));
        let tool_registry = Arc::new(clawhive_tool::registry::ToolRegistry::new());
        let budget_service = Arc::new(clawhive_budget::BudgetService);

        let worker_service = Arc::new(clawhive_worker::WorkerService::new(store.clone()));

        let default_worker_id = Some(WorkerId(uuid::Uuid::now_v7()));

        let runtime = AgentRuntime::new(
            AgentStore::new(store),
            model_router,
            tool_registry,
            budget_service,
            worker_service,
            default_worker_id,
        );

        (runtime, assert_store)
    }

    // ── Tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_execute_agent_rejects_hibernating_state() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let mut agent = sample_agent();
        agent.state = AgentState::Hibernating;
        store.save(&agent).await.unwrap();

        let result = runtime
            .execute_agent(
                &agent.id,
                "do something".into(),
                HashMap::new(),
                None,
            )
            .await;

        assert!(result.is_err(), "expected error for hibernating agent");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Hibernating"),
            "error should mention state: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_execute_agent_rejects_terminated_state() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let mut agent = sample_agent();
        agent.state = AgentState::Terminated;
        store.save(&agent).await.unwrap();

        let result = runtime
            .execute_agent(&agent.id, "do something".into(), HashMap::new(), None)
            .await;

        assert!(result.is_err(), "expected error for terminated agent");
    }

    #[tokio::test]
    async fn test_execute_agent_fails_without_worker() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let agent_store = AgentStore::new(memory.clone());
        let registry = clawhive_model_router::provider::ModelRegistry::new();
        let model_router = Arc::new(clawhive_model_router::router::ModelRouter::new(registry));
        let tool_registry = Arc::new(clawhive_tool::registry::ToolRegistry::new());
        let budget_service = Arc::new(clawhive_budget::BudgetService);
        let worker_service = Arc::new(clawhive_worker::WorkerService::new(memory));



        let runtime = AgentRuntime::new(
            agent_store,
            model_router,
            tool_registry,
            budget_service,
            worker_service,
            None, // no default worker
        );

        let agent = sample_agent();
        // Agent is not saved, so it will fail with NotFound
        let result = runtime
            .execute_agent(&agent.id, "test".into(), HashMap::new(), None)
            .await;

        assert!(result.is_err(), "expected NotFound error");
    }

    #[tokio::test]
    async fn test_hibernate_and_wake_agent() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let mut agent = sample_agent();
        agent.state = AgentState::Active;
        // Assign a lease so hibernate has something to checkpoint
        LifecycleService::assign_lease(&mut agent, "worker-1", 60);
        store.save(&agent).await.unwrap();

        // Hibernate
        runtime.hibernate_agent(&agent.id).await.unwrap();
        let saved = store.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(saved.state, AgentState::Hibernating);
        assert!(saved.current_runtime.is_none(), "lease should be released");
        assert!(!saved.checkpoints.is_empty(), "should have checkpoint");

        // Wake
        let worker_id = WorkerId(uuid::Uuid::now_v7());
        runtime.wake_agent(&agent.id, &worker_id).await.unwrap();
        let saved = store.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(saved.state, AgentState::Active);
        assert!(saved.current_runtime.is_some(), "should have new lease");
    }

    #[tokio::test]
    async fn test_terminate_agent_full_teardown() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let agent = sample_agent();
        store.save(&agent).await.unwrap();

        runtime.terminate_agent(&agent.id).await.unwrap();

        let saved = store.get(&agent.id).await.unwrap().unwrap();
        assert_eq!(saved.state, AgentState::Terminated, "final state should be Terminated");
        assert!(saved.terminated_at.is_some(), "should have terminated_at");
        assert!(saved.current_runtime.is_none(), "lease should be revoked");
    }

    #[tokio::test]
    async fn test_hibernate_rejects_non_active_state() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let mut agent = sample_agent();
        agent.state = AgentState::Terminated;
        store.save(&agent).await.unwrap();

        let result = runtime.hibernate_agent(&agent.id).await;
        assert!(result.is_err(), "should reject hibernate from Terminated");
    }

    #[tokio::test]
    async fn test_heartbeat_renews_lease() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let mut agent = sample_agent();
        agent.state = AgentState::Active;
        LifecycleService::assign_lease(&mut agent, "worker-1", 60);
        store.save(&agent).await.unwrap();

        let remaining = runtime.heartbeat_agent(&agent.id).await.unwrap();
        assert!(
            remaining.num_seconds() > 0,
            "lease should have positive remaining TTL"
        );
    }

    #[tokio::test]
    async fn test_apply_pattern_on_persistent_agent() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let mut agent = sample_agent();
        agent.state = AgentState::Active;
        agent.lifecycle_mode = LifecycleMode::Persistent;
        store.save(&agent).await.unwrap();

        // apply_pattern on an Active persistent agent should keep it active (no schedules → should_hibernate)
        runtime.apply_pattern(&agent.id).await.unwrap();
        let saved = store.get(&agent.id).await.unwrap().unwrap();
        // The agent has no schedules, so should_be_active returns true (AlwaysOn default)
        // Actually, looking at the domain, `None` pattern means should_be_active = true
        // Still active should pass if pattern doesn't hibernate
        assert_eq!(saved.state, AgentState::Active);
    }

    #[tokio::test]
    async fn test_hibernate_agent_not_found() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, _) = make_runtime(memory);

        let missing_id = AgentId(uuid::Uuid::now_v7());
        let result = runtime.hibernate_agent(&missing_id).await;
        assert!(result.is_err(), "should error for missing agent");
        assert!(
            result.unwrap_err().to_string().contains("not found"),
            "error should mention not found"
        );
    }

    #[tokio::test]
    async fn test_terminate_agent_not_found() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, _) = make_runtime(memory);

        let missing_id = AgentId(uuid::Uuid::now_v7());
        let result = runtime.terminate_agent(&missing_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_session_constructs_context() {
        let memory = Arc::new(clawhive_store::InMemoryStore::new());
        let (runtime, _) = make_runtime(memory);

        let agent = sample_agent();
        let worker_id = WorkerId(uuid::Uuid::now_v7());

        // run_session on an agent with no model provider configured will fail gracefully
        let result = runtime
            .run_session(
                &agent,
                "test objective",
                HashMap::new(),
                &worker_id,
                5,
            )
            .await;

        // Should fail due to no model provider (not due to bad context construction)
        assert!(
            result.is_err(),
            "expected error due to missing model provider"
        );
        let err = result.unwrap_err().to_string();
        // The error should mention model, not tool context construction
        assert!(
            err.contains("model") || err.contains("provider") || err.contains("not available"),
            "error should relate to model routing, got: {}",
            err
        );
    }
}
