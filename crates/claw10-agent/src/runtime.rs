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
//!   ├─ build ToolContext + workspace_dir
//!   ├─ run AgentExecutor (model loop + tool calls)
//!   ├─ write-back memory via MemoryService
//!   ├─ persist updated agent state
//!   └─ return (AgentSession, Vec<AgentEvent>)
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use claw10_domain::{
    Agent, AgentId, AgentState, MemoryType, RuntimeLease, Task, TaskId, TaskState, WorkerId,
};

use claw10_context::{ContextPipeline, ContextSources, PipelineConfig};
use claw10_lifecycle::LifecycleService;
use claw10_memory::{MemoryService, StoreMemoryInput};
use claw10_model_router::router::ModelRouter;
use claw10_store::StoreExt;
use claw10_tool::context::ToolContext;
use claw10_tool::registry::ToolRegistry;
use claw10_worker::WorkerService;

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
/// Wraps `AgentExecutor` dengan lifecycle management, worker assignment,
/// memory write-back, dan state persistence.
pub struct AgentRuntime {
    agent_store: AgentStore,
    executor: AgentExecutor,
    /// Worker service untuk registrasi worker saat dibutuhkan.
    #[allow(dead_code)]
    worker_service: Arc<WorkerService>,
    memory_service: MemoryService,
    /// Fallback worker ID jika tidak ada worker yang di-provide secara eksplisit.
    default_worker_id: Option<WorkerId>,
}

impl AgentRuntime {
    /// Create a new agent runtime.
    #[must_use]
    pub fn new(
        agent_store: AgentStore,
        model_router: Arc<ModelRouter>,
        tool_registry: Arc<ToolRegistry>,
        budget_service: Arc<claw10_budget::BudgetService>,
        worker_service: Arc<WorkerService>,
        default_worker_id: Option<WorkerId>,
    ) -> Self {
        let store = Arc::clone(agent_store.store());
        let memory_service = MemoryService::new(Arc::clone(&store));
        Self {
            agent_store,
            executor: AgentExecutor::new(
                model_router,
                tool_registry,
                budget_service,
                Arc::clone(&store),
            ),
            worker_service,
            memory_service,
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
        task_id_override: Option<TaskId>,
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

        // ── Siapkan workspace_dir untuk agent ────────────────────
        let workspace_dir = format!("/tmp/claw10/{}", agent.id.0);
        if let Err(e) = std::fs::create_dir_all(&workspace_dir) {
            tracing::warn!("Gagal membuat workspace_dir {workspace_dir}: {e}");
        }

        // Tentukan task_id sesungguhnya
        let task_id = task_id_override.clone().unwrap_or_else(|| TaskId(uuid::Uuid::now_v7()));

        // Update task state ke Running di database jika di-pass
        if let Some(ref tid) = task_id_override {
            self.update_task_state(tid, TaskState::Running).await;
        }

        // ── Build ToolContext ────────────────────────────────────
        let tool_context = ToolContext {
            tenant_id: "default".to_string(),
            mission_id: agent.mission_id.clone(),
            task_id,
            agent_id: agent.id.clone(),
            worker_id: worker_id.clone(),
            idempotency_key: uuid::Uuid::now_v7().to_string(),
            risk_level: "medium".to_string(),
            approval_id: None,
            budget_remaining: agent.budget.remaining(),
            workspace_dir,
        };

        // ── Compute max turns from genome ────────────────────────
        let max_turns = agent.genome.autonomy.max_children.max(1) * DEFAULT_TURNS_MULTIPLIER;

        // ── Build and inject system context ──────────────────────
        let mut context = context;
        if let Some(system_context) = self.build_context_for_agent(&agent).await {
            context.insert("system_context".to_string(), system_context);
        }

        // ── Execute ──────────────────────────────────────────────
        let (session, events) = match self
            .executor
            .execute(&mut agent, &objective, context, tool_context, max_turns)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                if let Some(ref tid) = task_id_override {
                    self.update_task_state(tid, TaskState::Failed).await;
                }
                return Err(e);
            }
        };

        // ── Persist updated agent ────────────────────────────────
        if session.state == SessionState::Completed {
            agent.state = AgentState::Active;
            // Simpan semua Thought events sebagai memori
            self.write_session_memory(&agent, &objective, &events).await;

            // Update status task ke Completed/Accepted di database
            if let Some(ref tid) = task_id_override {
                self.update_task_state(tid, TaskState::Accepted).await;
            }
        } else if let Some(ref tid) = task_id_override {
            self.update_task_state(tid, TaskState::Failed).await;
        }

        agent.turn_count = session.turn_count as u64;
        agent.total_cost_usd = session.total_cost_usd;
        agent.updated_at = chrono::Utc::now();
        self.agent_store.save(&agent).await?;

        // ── Cleanup workspace setelah agent selesai ──────────────
        let workspace_dir = format!("/tmp/claw10/{}", agent.id.0);
        if let Err(e) = std::fs::remove_dir_all(&workspace_dir) {
            // Jangan gagalkan eksekusi karena cleanup error
            tracing::debug!("Cleanup workspace {workspace_dir} gagal (mungkin sudah kosong): {e}");
        }

        Ok((session, events))
    }


    /// Versi streaming dari `execute_agent` — AgentEvent langsung dikirim ke `event_tx`
    /// sehingga TUI dapat menampilkan progres real-time (thinking, tool call, done).
    ///
    /// # Errors
    ///
    /// Sama dengan [`execute_agent`]: `AgentNotFound`, state/worker errors.
    pub async fn execute_agent_streaming(
        &self,
        agent_id: &AgentId,
        objective: String,
        context: HashMap<String, String>,
        worker_override: Option<WorkerId>,
        event_tx: crate::executor::EventSender,
        task_id_override: Option<TaskId>,
    ) -> Result<AgentSession, AgentError> {
        let mut agent = self.agent_store.get_or_not_found(agent_id).await?;

        // ── Pre-flight checks ───────────────────────────────────
        self.ensure_runnable(&agent, agent_id)?;

        let worker_id = worker_override
            .or_else(|| self.default_worker_id.clone())
            .ok_or_else(|| {
                AgentError::Other("no worker specified and no default worker configured".into())
            })?;

        // ── Assign runtime lease ─────────────────────────────────
        if agent.current_runtime.is_none() {
            LifecycleService::assign_lease(&mut agent, &worker_id.0.to_string(), DEFAULT_LEASE_SECONDS);
            self.agent_store.save(&agent).await?;
        }

        // ── Siapkan workspace_dir untuk agent ────────────────────
        let workspace_dir = format!("/tmp/claw10/{}", agent.id.0);
        if let Err(e) = std::fs::create_dir_all(&workspace_dir) {
            tracing::warn!("Gagal membuat workspace_dir {workspace_dir}: {e}");
        }

        // Tentukan task_id sesungguhnya
        let task_id = task_id_override.clone().unwrap_or_else(|| TaskId(uuid::Uuid::now_v7()));

        // Update task state ke Running di database jika di-pass
        if let Some(ref tid) = task_id_override {
            self.update_task_state(tid, TaskState::Running).await;
        }

        // ── Build ToolContext ────────────────────────────────────
        let tool_context = ToolContext {
            tenant_id: "default".to_string(),
            mission_id: agent.mission_id.clone(),
            task_id,
            agent_id: agent.id.clone(),
            worker_id: worker_id.clone(),
            idempotency_key: uuid::Uuid::now_v7().to_string(),
            risk_level: "medium".to_string(),
            approval_id: None,
            budget_remaining: agent.budget.remaining(),
            workspace_dir: workspace_dir.clone(),
        };

        // ── Compute max turns from genome ────────────────────────
        let max_turns = agent.genome.autonomy.max_children.max(1) * DEFAULT_TURNS_MULTIPLIER;

        // ── Build and inject system context ──────────────────────
        let mut context = context;
        if let Some(system_context) = self.build_context_for_agent(&agent).await {
            context.insert("system_context".to_string(), system_context);
        }

        // ── Execute streaming ────────────────────────────────────
        let session = match self
            .executor
            .execute_streaming(&mut agent, &objective, context, tool_context, max_turns, event_tx)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                if let Some(ref tid) = task_id_override {
                    self.update_task_state(tid, TaskState::Failed).await;
                }
                return Err(e);
            }
        };

        // ── Persist updated agent ────────────────────────────────
        if session.state == SessionState::Completed {
            agent.state = AgentState::Active;

            // Update status task ke Completed/Accepted di database
            if let Some(ref tid) = task_id_override {
                self.update_task_state(tid, TaskState::Accepted).await;
            }
        } else if let Some(ref tid) = task_id_override {
            self.update_task_state(tid, TaskState::Failed).await;
        }
        agent.turn_count = session.turn_count as u64;
        agent.total_cost_usd = session.total_cost_usd;
        agent.updated_at = chrono::Utc::now();
        self.agent_store.save(&agent).await?;

        // ── Cleanup workspace setelah agent selesai ──────────────
        if let Err(e) = std::fs::remove_dir_all(&workspace_dir) {
            // Jangan gagalkan eksekusi karena cleanup error
            tracing::debug!("Cleanup workspace {workspace_dir} gagal (mungkin sudah kosong): {e}");
        }

        Ok(session)
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
        agent: &mut Agent,
        objective: &str,
        context: HashMap<String, String>,
        worker_id: &WorkerId,
        max_turns: u32,
    ) -> Result<(AgentSession, Vec<AgentEvent>), AgentError> {
        let mut context = context;
        if let Some(system_context) = self.build_context_for_agent(agent).await {
            context.insert("system_context".to_string(), system_context);
        }

        let tool_context = ToolContext {
            tenant_id: "default".to_string(),
            mission_id: agent.mission_id.clone(),
            task_id: TaskId(uuid::Uuid::now_v7()),
            agent_id: agent.id.clone(),
            worker_id: worker_id.clone(),
            idempotency_key: uuid::Uuid::now_v7().to_string(),
            risk_level: "medium".to_string(),
            approval_id: None,
            budget_remaining: agent.budget.remaining(),
            workspace_dir: format!("/tmp/claw10/{}", agent.id.0),
        };

        self.executor.execute(agent, objective, context, tool_context, max_turns).await
    }

    // ── Helpers ─────────────────────────────────────────────────

    /// Simpan semua AgentEvent::Thought ke MemoryService sebagai Working memory
    /// terpisah per thought, agar dapat digunakan sebagai konteks di sesi berikutnya.
    async fn write_session_memory(
        &self,
        agent: &Agent,
        objective: &str,
        events: &[AgentEvent],
    ) {
        // Tentukan scope dari genome agent
        let scope = agent
            .genome
            .memory
            .default_write_scope
            .clone()
            .unwrap_or_else(|| "global".to_string());

        let mut stored = 0usize;

        for event in events {
            let (content, memory_type) = match event {
                AgentEvent::Thought { content, .. } if !content.is_empty() => {
                    (content.clone(), MemoryType::Working)
                }
                AgentEvent::ObjectiveComplete { summary, .. } if !summary.is_empty() => {
                    (format!("Objective: {}\n\nResult: {}", objective, summary), MemoryType::Episodic)
                }
                _ => continue,
            };

            let input = StoreMemoryInput {
                tenant_id: "default".to_string(),
                scope: scope.clone(),
                memory_type,
                content,
                source_agent: agent.id.clone(),
                source_task: TaskId(uuid::Uuid::now_v7()),
                evidence_id: None,
                confidence: 0.85,
                classification: "unclassified".to_string(),
            };

            let mem = self.memory_service.store(input).await;
            tracing::debug!(
                "Memory write-back: agent {} → memory {} ({:?})",
                agent.id.0,
                mem.id.0,
                mem.status
            );
            stored += 1;
        }

        if stored > 0 {
            tracing::info!("Memory write-back: agent {} menyimpan {} memories", agent.id.0, stored);
        }
    }

    /// Build a system context string for an agent using the context pipeline.
    /// Menggunakan MemoryService::query() untuk mengambil memori active.
    async fn build_context_for_agent(&self, agent: &Agent) -> Option<String> {
        let store = Arc::clone(self.agent_store.store());

        let mission: Option<claw10_domain::Mission> = store
            .get::<claw10_domain::Mission>(&format!("mission:{}", agent.mission_id.0))
            .await
            .ok()
            .flatten();

        let lineage: Option<claw10_domain::Lineage> = store
            .get::<claw10_domain::Lineage>(&format!("lineage:{}", agent.lineage_id.0))
            .await
            .ok()
            .flatten();

        let agents: Vec<claw10_domain::Agent> = store
            .scan_prefix_unsorted::<claw10_domain::Agent>("agent:")
            .await
            .map(|v| v.into_iter().map(|(_, a)| a).collect())
            .unwrap_or_default();

        let skills: Vec<claw10_domain::Skill> = store
            .scan_prefix_unsorted::<claw10_domain::Skill>("skill:")
            .await
            .map(|v| {
                v.into_iter()
                    .map(|(_, s)| s)
                    .filter(|s| matches!(s.state, claw10_domain::SkillState::Active))
                    .collect()
            })
            .unwrap_or_default();

        // Gunakan MemoryService dengan filter Active untuk konteks yang relevan
        let memories = self
            .memory_service
            .query(claw10_memory::MemoryQuery {
                status: Some(claw10_domain::MemoryStatus::Active),
                ..Default::default()
            })
            .await
            .unwrap_or_default();

        let pipeline = ContextPipeline::new(PipelineConfig::default());
        let sources = ContextSources {
            task: None,
            mission: mission.as_ref(),
            memories: &memories,
            policies: &[agent.policy_bundle.clone()],
            skills: &skills,
            history: &[],
            tools: &[],
            agents: &agents,
            lineage: lineage.as_ref(),
            workers: &[],
            evidence: &[],
        };

        pipeline.build_context(sources).await.ok()
    }

    fn ensure_runnable(&self, agent: &Agent, id: &AgentId) -> Result<(), AgentError> {
        if agent.state != AgentState::Active && agent.state != AgentState::Ready {
            return Err(AgentError::Other(format!(
                "agent {} is in {:?} state, cannot execute",
                id.0, agent.state
            )));
        }
        Ok(())
    }


    /// Memperbarui status task di database.
    async fn update_task_state(&self, task_id: &TaskId, state: TaskState) {
        let store = self.agent_store.store();
        let key = format!("task:{}", task_id.0);
        if let Ok(Some(mut task)) = store.get::<Task>(&key).await {
            task.state = state;
            task.updated_at = chrono::Utc::now();
            if let Err(e) = store.set(&key, &task).await {
                tracing::warn!("Gagal memperbarui status task {}: {e}", task_id.0);
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use claw10_domain::{
        Agent, AgentGenome, AgentId, AgentState, AutonomyConfig, Budget,
        IdentityId, LifecycleMode, LineageId, MemoryConfig, MissionId,
        ModelPolicy, NetworkPolicy, PolicyBundle, RuntimeConfig, WorkerId,
    };
    use claw10_lifecycle::LifecycleService;
    use claw10_store::Store;

    use crate::runtime::AgentRuntime;
    use crate::store::AgentStore;

    // ── Helpers ─────────────────────────────────────────────────

    fn sample_agent() -> Agent {
        let now = chrono::Utc::now();
        Agent {
            id: AgentId(uuid::Uuid::now_v7()),
            identity_id: IdentityId(uuid::Uuid::now_v7()),
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
                id: claw10_domain::PolicyBundleId(uuid::Uuid::now_v7()),
                name: "default".into(),
                version: "1.0.0".into(),
                rules: vec![claw10_domain::PolicyRule {
                    id: claw10_domain::PolicyRuleId(uuid::Uuid::now_v7()),
                    subject: claw10_domain::PolicySubject::Role("*".into()),
                    effect: claw10_domain::PolicyEffect::Allow,
                    action: "*".into(),
                    resource: "*".into(),
                    priority: 0,
                    condition: None,
                }],
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

        let registry = claw10_model_router::provider::ModelRegistry::new();
        let model_router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));
        let tool_registry = Arc::new(claw10_tool::registry::ToolRegistry::new());
        let budget_service = Arc::new(claw10_budget::BudgetService);
        let worker_service = Arc::new(claw10_worker::WorkerService::new(store.clone()));
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
        let memory = Arc::new(claw10_store::InMemoryStore::new());
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
        let memory = Arc::new(claw10_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let mut agent = sample_agent();
        agent.state = AgentState::Terminated;
        store.save(&agent).await.unwrap();

        let result = runtime
            .execute_agent(&agent.id, "do something".into(), HashMap::new(), None, None)
            .await;


        assert!(result.is_err(), "expected error for terminated agent");
    }

    #[tokio::test]
    async fn test_execute_agent_fails_without_worker() {
        let memory = Arc::new(claw10_store::InMemoryStore::new());
        let agent_store = AgentStore::new(memory.clone());
        let registry = claw10_model_router::provider::ModelRegistry::new();
        let model_router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));
        let tool_registry = Arc::new(claw10_tool::registry::ToolRegistry::new());
        let budget_service = Arc::new(claw10_budget::BudgetService);
        let worker_service = Arc::new(claw10_worker::WorkerService::new(memory));



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
            .execute_agent(&agent.id, "test".into(), HashMap::new(), None, None)
            .await;


        assert!(result.is_err(), "expected NotFound error");
    }

    #[tokio::test]
    async fn test_hibernate_and_wake_agent() {
        let memory = Arc::new(claw10_store::InMemoryStore::new());
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
        let memory = Arc::new(claw10_store::InMemoryStore::new());
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
        let memory = Arc::new(claw10_store::InMemoryStore::new());
        let (runtime, store) = make_runtime(memory);

        let mut agent = sample_agent();
        agent.state = AgentState::Terminated;
        store.save(&agent).await.unwrap();

        let result = runtime.hibernate_agent(&agent.id).await;
        assert!(result.is_err(), "should reject hibernate from Terminated");
    }

    #[tokio::test]
    async fn test_heartbeat_renews_lease() {
        let memory = Arc::new(claw10_store::InMemoryStore::new());
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
        let memory = Arc::new(claw10_store::InMemoryStore::new());
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
        let memory = Arc::new(claw10_store::InMemoryStore::new());
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
        let memory = Arc::new(claw10_store::InMemoryStore::new());
        let (runtime, _) = make_runtime(memory);

        let missing_id = AgentId(uuid::Uuid::now_v7());
        let result = runtime.terminate_agent(&missing_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_session_constructs_context() {
        let memory = Arc::new(claw10_store::InMemoryStore::new());
        let (runtime, _) = make_runtime(memory);

        let mut agent = sample_agent();
        let worker_id = WorkerId(uuid::Uuid::now_v7());

        // run_session on an agent with no model provider configured will fail gracefully
        let result = runtime
            .run_session(
                &mut agent,
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

    // ── Integration test with MockModelProvider ─────────────────

    struct MockModelProvider {
        name: String,
        models: Vec<String>,
    }

    impl MockModelProvider {
        fn new(name: &str, models: Vec<&str>) -> Self {
            Self {
                name: name.to_string(),
                models: models.into_iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    #[async_trait::async_trait]
    impl claw10_model_router::provider::ModelProvider for MockModelProvider {
        fn name(&self) -> &str {
            &self.name
        }

        fn supported_models(&self) -> Vec<&str> {
            self.models.iter().map(|s| s.as_str()).collect()
        }

        fn get_profile(&self, model_name: &str) -> Option<claw10_model_router::types::ModelProfile> {
            if self.models.iter().any(|m| m == model_name) {
                Some(claw10_model_router::types::ModelProfile {
                    id: model_name.to_string(),
                    provider: self.name.clone(),
                    model_name: model_name.to_string(),
                    context_window: 4096,
                    max_output_tokens: 1024,
                    cost_per_1m_input: 10.00,
                    cost_per_1m_output: 30.00,
                    suitable_for: vec!["general".to_string()],
                })
            } else {
                None
            }
        }

        async fn chat(
            &self,
            _request: claw10_model_router::types::ChatRequest,
        ) -> Result<claw10_model_router::types::ChatResponse, claw10_model_router::ModelError> {
            // Return a simple final answer immediately (no tool calls → loop terminates)
            Ok(claw10_model_router::types::ChatResponse {
                message: claw10_model_router::types::ModelMessage {
                    role: claw10_model_router::types::MessageRole::Assistant,
                    content: "Task completed successfully.".into(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                finish_reason: claw10_model_router::types::FinishReason::Stop,
                usage: claw10_model_router::types::UsageInfo {
                    prompt_tokens: 100,
                    completion_tokens: 20,
                    total_tokens: 120,
                    cost_usd: 0.002,
                },
                model_used: self.models[0].clone(),
            })
        }
    }

    #[tokio::test]
    async fn test_runtime_integration_with_mock_provider() {
        let memory = Arc::new(claw10_store::InMemoryStore::new());
        let store = AgentStore::new(memory.clone());

        // Register mock model provider
        let mut registry = claw10_model_router::provider::ModelRegistry::new();
        registry.register(Box::new(MockModelProvider::new(
            "mock",
            vec!["gpt-4o", "gpt-4o-mini"],
        )));
        let model_router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));

        // Register tools (shell, read_file for context)
        let mut tool_registry = claw10_tool::registry::ToolRegistry::new();
        tool_registry.register(Box::new(claw10_tool::builtin::ShellTool));
        tool_registry.register(Box::new(claw10_tool::builtin::ReadFileTool));
        let tool_registry = Arc::new(tool_registry);

        let budget_service = Arc::new(claw10_budget::BudgetService);
        let worker_service = Arc::new(claw10_worker::WorkerService::new(memory.clone()));
        let default_worker_id = Some(WorkerId(uuid::Uuid::now_v7()));

        let runtime = AgentRuntime::new(
            AgentStore::new(memory.clone()),
            model_router,
            tool_registry,
            budget_service,
            worker_service,
            default_worker_id.clone(),
        );

        // Save an Active agent
        let mut agent = sample_agent();
        agent.genome.model_policy.preferred_profile = "gpt-4o".into();
        agent.genome.model_policy.fallback_profiles = vec!["gpt-4o-mini".into()];
        store.save(&agent).await.unwrap();

        let (session, events) = runtime
            .execute_agent(
                &agent.id,
                "complete the task".into(),
                HashMap::new(),
                default_worker_id,
                None,
            )
            .await
            .expect("runtime.execute_agent should succeed with mock provider");


        // Verify session state
        assert!(
            session.state == crate::session::SessionState::Completed
                || session.state == crate::session::SessionState::Active,
            "session should be completed or active, got {:?}",
            session.state
        );
        assert!(session.turn_count > 0, "should have at least 1 turn");
        assert!(session.total_tokens > 0, "should have consumed tokens");

        // Verify events
        assert!(!events.is_empty(), "should have events");
        let has_session_started = events
            .iter()
            .any(|e| matches!(e, crate::events::AgentEvent::SessionStarted { .. }));
        assert!(has_session_started, "should have SessionStarted event");

        let has_model_call = events
            .iter()
            .any(|e| matches!(e, crate::events::AgentEvent::ModelCall { .. }));
        assert!(has_model_call, "should have ModelCall event");

        let has_objective_complete = events
            .iter()
            .any(|e| matches!(e, crate::events::AgentEvent::ObjectiveComplete { .. }));
        assert!(has_objective_complete, "should have ObjectiveComplete event");

        // Verify agent state was persisted
        let saved = store.get(&agent.id).await.unwrap().unwrap();
        assert!(
            saved.turn_count >= 1,
            "persisted agent should have turn_count >= 1"
        );
        assert!(
            saved.total_cost_usd > 0.0,
            "persisted agent should have total_cost_usd > 0"
        );
    }

    #[tokio::test]
    async fn test_runtime_integration_with_context() {
        let memory = Arc::new(claw10_store::InMemoryStore::new());
        let store = AgentStore::new(memory.clone());

        // Register mock provider
        let mut registry = claw10_model_router::provider::ModelRegistry::new();
        registry.register(Box::new(MockModelProvider::new(
            "mock",
            vec!["gpt-4o"],
        )));
        let model_router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));

        let tool_registry = Arc::new(claw10_tool::registry::ToolRegistry::new());
        let budget_service = Arc::new(claw10_budget::BudgetService);
        let worker_service = Arc::new(claw10_worker::WorkerService::new(memory.clone()));
        let worker_id = WorkerId(uuid::Uuid::now_v7());

        let runtime = AgentRuntime::new(
            AgentStore::new(memory.clone()),
            model_router,
            tool_registry,
            budget_service,
            worker_service,
            Some(worker_id.clone()),
        );

        // Save agent with context-relevant settings
        let mut agent = sample_agent();
        agent.genome.model_policy.preferred_profile = "gpt-4o".into();
        store.save(&agent).await.unwrap();

        // Execute with extra context
        let mut context = HashMap::new();
        context.insert("mission_statement".into(), "Test mission".into());
        context.insert("user_id".into(), "user-123".into());

        let (session, events) = runtime
            .execute_agent(&agent.id, "test with context".into(), context, None, None)
            .await
            .expect("execute_agent with context should succeed");


        assert!(session.turn_count > 0, "should have completed at least 1 turn");

        let has_thought = events
            .iter()
            .any(|e| matches!(e, crate::events::AgentEvent::Thought { .. }));
        assert!(has_thought, "should have Thought event with response content");
    }
}
