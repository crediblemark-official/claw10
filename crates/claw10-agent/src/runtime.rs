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
    Agent, AgentId, AgentState, MemoryType, RuntimeLease, SkillState, Task, TaskId, TaskState,
    WorkerId,
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
    tool_registry: Arc<ToolRegistry>,
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
                Arc::clone(&tool_registry),
                budget_service,
                Arc::clone(&store),
            ),
            tool_registry,
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
        let chat_history_raw = context.get("chat_history").cloned().unwrap_or_default();
        let chat_history: Vec<String> = if !chat_history_raw.is_empty() {
            serde_json::from_str(&chat_history_raw).unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(system_context) = self.build_context_for_agent(&agent, &chat_history).await {
            context.insert("system_context".to_string(), system_context);
        }

        // Load distilled memory jangka panjang dari KV store
        let distilled_key = format!("memory:distilled:agent:{}", agent.id.0);
        if let Ok(Some(distilled_mem)) = self.agent_store.store().get::<String>(&distilled_key).await {
            context.insert("distilled_memory".to_string(), distilled_mem);
        }

        // Load profil kepribadian dinamis dari database KV store
        if let Ok(Some(soul)) = self.agent_store.store().get::<String>("profile:agent:soul").await {
            context.insert("agent_soul".to_string(), soul);
        }
        if let Ok(Some(name)) = self.agent_store.store().get::<String>("profile:agent:name").await {
            context.insert("agent_name".to_string(), name);
        }
        if let Ok(Some(op_name)) = self.agent_store.store().get::<String>("profile:operator:name").await {
            context.insert("operator_name".to_string(), op_name);
        }
        if let Ok(Some(op_tz)) = self.agent_store.store().get::<String>("profile:operator:timezone").await {
            context.insert("operator_timezone".to_string(), op_tz);
        }
        if let Ok(Some(op_lang)) = self.agent_store.store().get::<String>("profile:operator:language").await {
            context.insert("operator_language".to_string(), op_lang);
        }
        if let Ok(Some(op_style)) = self.agent_store.store().get::<String>("profile:operator:style").await {
            context.insert("operator_style".to_string(), op_style);
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
        let chat_history_raw = context.get("chat_history").cloned().unwrap_or_default();
        let chat_history: Vec<String> = if !chat_history_raw.is_empty() {
            serde_json::from_str(&chat_history_raw).unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(system_context) = self.build_context_for_agent(&agent, &chat_history).await {
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
        let chat_history_raw = context.get("chat_history").cloned().unwrap_or_default();
        let chat_history: Vec<String> = if !chat_history_raw.is_empty() {
            serde_json::from_str(&chat_history_raw).unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(system_context) = self.build_context_for_agent(agent, &chat_history).await {
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

    /// Execute a skill by its ID, running its steps as tool calls.
    ///
    /// Looks up the skill from the store, validates it's in Active state,
    /// then executes each step as a tool invocation using the tool registry.
    ///
    /// # Errors
    /// Returns `AgentError::Other` if the skill is not found or not active.
    pub async fn execute_skill(
        &self,
        agent: &mut Agent,
        skill_id: &str,
        input: serde_json::Value,
        worker_id: &WorkerId,
    ) -> Result<serde_json::Value, AgentError> {
        // 1. Load skill from store
        let skill_key = format!("skill:{}", skill_id);
        let skill: claw10_domain::Skill = self.agent_store.store()
            .get(&skill_key)
            .await
            .map_err(|e| AgentError::Other(e.to_string()))?
            .ok_or_else(|| AgentError::Other(format!("skill not found: {skill_id}")))?;

        // 2. Verify skill is Active
        if skill.state != SkillState::Active {
            return Err(AgentError::Other(format!(
                "skill {} is in {:?} state, must be Active",
                skill_id, skill.state
            )));
        }

        // 3. Execute each step as a tool call
        let mut output = serde_json::json!({ "steps_completed": 0, "results": [] });
        let mut results = Vec::new();

        for (i, step) in skill.steps.iter().enumerate() {
            tracing::info!("Executing skill '{}' step {}/{}: {}", skill.name, i + 1, skill.steps.len(), step);

            // Each step is treated as a tool name to invoke with the input
            match self.tool_registry.get(step) {
                Ok(tool) => {
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
                    match tool.execute(&tool_context, input.clone()).await {
                        Ok(result) => {
                            results.push(serde_json::json!({
                                "step": step,
                                "status": "ok",
                                "result": result.data
                            }));
                        }
                        Err(e) => {
                            results.push(serde_json::json!({
                                "step": step,
                                "status": "error",
                                "error": e.to_string()
                            }));
                        }
                    }
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "step": step,
                        "status": "not_found",
                        "error": format!("tool '{}' not found: {}", step, e)
                    }));
                }
            }
        }

        output["steps_completed"] = serde_json::json!(results.len());
        output["results"] = serde_json::json!(results);
        Ok(output)
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
    async fn build_context_for_agent(&self, agent: &Agent, chat_history: &[String]) -> Option<String> {
        let store = Arc::clone(self.agent_store.store());

        let mission: Option<claw10_domain::Mission> = match store
            .get::<claw10_domain::Mission>(&format!("mission:{}", agent.mission_id.0))
            .await
        {
            Ok(val) => val,
            Err(e) => {
                tracing::warn!("Failed to load mission {} for context: {e}", agent.mission_id.0);
                None
            }
        };

        let lineage: Option<claw10_domain::Lineage> = match store
            .get::<claw10_domain::Lineage>(&format!("lineage:{}", agent.lineage_id.0))
            .await
        {
            Ok(val) => val,
            Err(e) => {
                tracing::warn!("Failed to load lineage {} for context: {e}", agent.lineage_id.0);
                None
            }
        };

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
            history: chat_history,
            tools: &[],
            agents: &agents,
            lineage: lineage.as_ref(),
            workers: &[],
            evidence: &[],
        };

        match pipeline.build_context(sources).await {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                tracing::warn!("Failed to build context pipeline for agent {}: {e}", agent.id.0);
                None
            }
        }
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
#[path = "runtime_test.rs"]
mod tests;
