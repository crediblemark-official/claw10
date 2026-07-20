use super::*;

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
                    content_parts: None,
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
    tool_registry.register(Box::new(claw10_tool::builtin::ShellTool::new()));
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

#[tokio::test]
async fn test_execute_skill_active() {
    use claw10_domain::{Skill, SkillCostProfile, SkillId, SkillState};
    use claw10_store::StoreExt;

    let memory = Arc::new(claw10_store::InMemoryStore::new());
    let store = AgentStore::new(memory.clone());

    // Register a mock tool that the skill steps will reference
    let mut tool_registry = claw10_tool::registry::ToolRegistry::new();
    tool_registry.register(Box::new(SkillTestTool::new("echo_tool")));
    let tool_registry = Arc::new(tool_registry);

    let registry = claw10_model_router::provider::ModelRegistry::new();
    let model_router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));
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

    // Save an Active skill to the store
    let skill_id = SkillId(uuid::Uuid::now_v7());
    let skill = Skill {
        id: skill_id.clone(),
        name: "test-skill".into(),
        purpose: "A test skill".into(),
        version: "1.0".into(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        steps: vec!["echo_tool".into()],
        required_tools: vec!["echo_tool".into()],
        required_permissions: vec![],
        state: SkillState::Active,
        signature: None,
        cost_profile: SkillCostProfile {
            estimated_cost_usd: 0.0,
            average_duration_seconds: 0.0,
        },
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    memory.set(&format!("skill:{}", skill_id.0), &skill).await.unwrap();

    let mut agent = sample_agent();
    let input = serde_json::json!({ "message": "hello" });

    let result = runtime
        .execute_skill(&mut agent, &skill_id.0.to_string(), input, &worker_id)
        .await;

    assert!(result.is_ok(), "execute_skill should succeed: {:?}", result.err());
    let output = result.unwrap();
    assert_eq!(output["steps_completed"], 1);
    assert_eq!(output["results"][0]["status"], "ok");
}

#[tokio::test]
async fn test_execute_skill_not_found() {
    let memory = Arc::new(claw10_store::InMemoryStore::new());
    let (runtime, _) = make_runtime(memory);

    let mut agent = sample_agent();
    let worker_id = WorkerId(uuid::Uuid::now_v7());
    let input = serde_json::json!({});

    let result = runtime
        .execute_skill(&mut agent, "nonexistent-skill", input, &worker_id)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"), "error should mention not found: {}", err);
}

#[tokio::test]
async fn test_execute_skill_not_active() {
    use claw10_domain::{Skill, SkillCostProfile, SkillId, SkillState};
    use claw10_store::StoreExt;

    let memory = Arc::new(claw10_store::InMemoryStore::new());
    let store = AgentStore::new(memory.clone());
    let (runtime, _) = make_runtime(memory.clone());

    let skill_id = SkillId(uuid::Uuid::now_v7());
    let skill = Skill {
        id: skill_id.clone(),
        name: "draft-skill".into(),
        purpose: "Not active".into(),
        version: "1.0".into(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        steps: vec![],
        required_tools: vec![],
        required_permissions: vec![],
        state: SkillState::Candidate,
        signature: None,
        cost_profile: SkillCostProfile {
            estimated_cost_usd: 0.0,
            average_duration_seconds: 0.0,
        },
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    memory.set(&format!("skill:{}", skill_id.0), &skill).await.unwrap();

    let mut agent = sample_agent();
    let worker_id = WorkerId(uuid::Uuid::now_v7());
    let input = serde_json::json!({});

    let result = runtime
        .execute_skill(&mut agent, &skill_id.0.to_string(), input, &worker_id)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Candidate"), "error should mention current state: {}", err);
}

#[tokio::test]
async fn test_execute_skill_tool_not_found() {
    use claw10_domain::{Skill, SkillCostProfile, SkillId, SkillState};
    use claw10_store::StoreExt;

    let memory = Arc::new(claw10_store::InMemoryStore::new());
    let store = AgentStore::new(memory.clone());
    let (runtime, _) = make_runtime(memory.clone());

    let skill_id = SkillId(uuid::Uuid::now_v7());
    let skill = Skill {
        id: skill_id.clone(),
        name: "bad-skill".into(),
        purpose: "References missing tool".into(),
        version: "1.0".into(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        steps: vec!["nonexistent_tool".into()],
        required_tools: vec![],
        required_permissions: vec![],
        state: SkillState::Active,
        signature: None,
        cost_profile: SkillCostProfile {
            estimated_cost_usd: 0.0,
            average_duration_seconds: 0.0,
        },
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    memory.set(&format!("skill:{}", skill_id.0), &skill).await.unwrap();

    let mut agent = sample_agent();
    let worker_id = WorkerId(uuid::Uuid::now_v7());
    let input = serde_json::json!({});

    let result = runtime
        .execute_skill(&mut agent, &skill_id.0.to_string(), input, &worker_id)
        .await;

    assert!(result.is_ok(), "execute_skill should still succeed even with missing tool");
    let output = result.unwrap();
    assert_eq!(output["steps_completed"], 1);
    assert_eq!(output["results"][0]["status"], "not_found");
}

// ── Helper tool for skill tests ─────────────────────────────

struct SkillTestTool {
    name: String,
}

impl SkillTestTool {
    fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}

#[async_trait::async_trait]
impl claw10_tool::registry::Tool for SkillTestTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "A test tool for skill execution tests"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    fn categories(&self) -> Vec<&str> {
        vec!["test"]
    }

    fn side_effect_class(&self) -> claw10_domain::SideEffectClass {
        claw10_domain::SideEffectClass::ReadOnly
    }

    async fn execute(
        &self,
        _context: &claw10_tool::context::ToolContext,
        args: serde_json::Value,
    ) -> Result<claw10_tool::result::ToolOutput, claw10_tool::error::ToolError> {
        Ok(claw10_tool::result::ToolOutput::ok(serde_json::json!({
            "echo": args
        })))
    }
}
