use std::sync::Arc;
use claw10_domain::{
    Agent, AgentGenome, AgentId, AgentState, AutonomyConfig, Budget, LifecycleMode,
    MemoryConfig, ModelPolicy, NetworkPolicy, PolicyBundle, PolicyBundleId, RuntimeConfig,
    WorkerType,
};
use claw10_store::InMemoryStore;
use claw10_model_router::router::ModelRouter;
use claw10_tool::registry::ToolRegistry;

#[tokio::test]
async fn test_real_llm_api_call() {
    // Cari API key dari env vars
    let has_key = std::env::var("OPENROUTER_API_KEY").is_ok()
        || std::env::var("OPENAI_API_KEY").is_ok();

    if !has_key {
        println!("Warning: Lewati test real API karena API keys tidak dipasang di env vars.");
        return;
    }

    let kv_store: Arc<dyn claw10_store::Store> = Arc::new(InMemoryStore::new());
    let mut registry = claw10_model_router::provider::ModelRegistry::new();

    // Registrasi provider dari env vars
    for config in claw10_model_router::providers::provider_configs() {
        // Native providers (e.g. Bedrock) are registered via their factory.
        if let Some(factory) = config.factory {
            let name = config.name.to_string();
            if !registry.list_providers().contains(&name) {
                registry.register(factory());
            }
            continue;
        }

        if let Ok(key) = std::env::var(config.api_key_env)
            && !key.trim().is_empty() {
                registry.register(Box::new(
                    claw10_model_router::openai_compat::OpenAiCompatibleProvider::with_config(
                        config.name,
                        config.base_url,
                        key,
                        config.models.clone(),
                    )
                ));
            }
    }

    let model_router = Arc::new(ModelRouter::new(registry));
    let active_profiles = model_router.registry().list_profiles();
    if active_profiles.is_empty() {
        println!("Warning: Tidak ada provider terdaftar, lewati.");
        return;
    }

    let model_id = active_profiles[0].id.clone();

    // Buat dummy agent
    let now = chrono::Utc::now();
    let agent = Agent {
        id: AgentId(uuid::Uuid::now_v7()),
        identity_id: claw10_domain::IdentityId(uuid::Uuid::now_v7()),
        mission_id: claw10_domain::MissionId(uuid::Uuid::now_v7()),
        parent_agent_id: None,
        lineage_id: claw10_domain::LineageId(uuid::Uuid::now_v7()),
        name: "test-agent".into(),
        role: "Tester".into(),
        genome: AgentGenome {
            id: "test-genome".into(),
            version: "1.0.0".into(),
            role: "Tester".into(),
            lifecycle_modes: vec![LifecycleMode::Ephemeral],
            model_policy: ModelPolicy {
                preferred_profile: model_id.clone(),
                fallback_profiles: vec![],
                max_context_tokens: 4096,
            },
            autonomy: AutonomyConfig {
                can_spawn: false,
                max_spawn_depth: 0,
                max_children: 0,
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
        state: AgentState::Ready,
        lifecycle_mode: LifecycleMode::Ephemeral,
        persistent_pattern: None,
        budget: Budget {
            allocated_usd: 10.0,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: Some(10.0),
            recurring_monthly_usd: None,
        },
        delegable_permissions: vec![],
        non_delegable_permissions: vec![],
        current_runtime: None,
        checkpoints: vec![],
        subscriptions: vec![],
        schedules: vec![],
        policy_bundle: PolicyBundle {
            id: PolicyBundleId(uuid::Uuid::now_v7()),
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
    };

    let agent_store = claw10_agent::store::AgentStore::new(Arc::clone(&kv_store));
    agent_store.save(&agent).await.unwrap();

    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Box::new(claw10_tool::builtin::ReadFileTool));
    tool_registry.register(Box::new(claw10_tool::builtin::WriteFileTool));
    tool_registry.register(Box::new(claw10_tool::builtin::HttpTool));
    let tool_registry = Arc::new(tool_registry);

    let worker_service = Arc::new(claw10_worker::WorkerService::new(Arc::clone(&kv_store)));
    let budget_service = Arc::new(claw10_budget::BudgetService);

    let worker = worker_service.register(
        "test-worker".to_string(),
        WorkerType::Local,
        vec![],
        "1.0.0".to_string(),
    ).await;

    let runtime = claw10_agent::runtime::AgentRuntime::new(
        agent_store,
        model_router,
        tool_registry,
        budget_service,
        worker_service,
        Some(worker.id),
    );

    let objective = "Katakan 'Halo Dunia' dan gunakan HttpTool untuk memanggil https://httpbin.org/status/200";
    let mut context = std::collections::HashMap::new();
    context.insert("mission_statement".to_string(), "TEST REAL API".to_string());

    let (session, events) = runtime.execute_agent(&agent.id, objective.to_string(), context, None, None).await.unwrap();

    assert_eq!(session.state, claw10_agent::session::SessionState::Completed);
    assert!(session.turn_count >= 1);
    assert!(!events.is_empty());
}
