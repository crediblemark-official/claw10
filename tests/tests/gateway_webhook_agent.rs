//! E2E test: incoming gateway webhook routes to an agent and dispatches the reply.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{Json, Router, routing::post};

use claw10_control_api::state::AppState;
use claw10_domain::{
    Agent, AgentGenome, AgentId, AgentState, AutonomyConfig, Budget, IdentityId,
    LifecycleMode, LineageId, MemoryConfig, Mission, MissionId, MissionState,
    ModelPolicy, NetworkPolicy, RiskLevel, RuntimeConfig,
};
use claw10_model_router::types::{
    ChatRequest, ChatResponse, FinishReason, MessageRole, ModelMessage, ModelProfile,
    UsageInfo,
};
use claw10_store::StoreExt;

struct MockProvider;

#[async_trait::async_trait]
#[allow(dead_code)]
impl claw10_model_router::provider::ModelProvider for MockProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn supported_models(&self) -> Vec<&str> {
        vec!["gpt-4o"]
    }

    fn get_profile(&self, model_name: &str) -> Option<ModelProfile> {
        if model_name == "gpt-4o" {
            Some(ModelProfile {
                id: "gpt-4o".into(),
                provider: "mock".into(),
                model_name: "gpt-4o".into(),
                context_window: 4096,
                max_output_tokens: 1024,
                cost_per_1m_input: 0.0,
                cost_per_1m_output: 0.0,
                suitable_for: vec!["general".into()],
            })
        } else {
            None
        }
    }

    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, claw10_model_router::ModelError> {
        Ok(ChatResponse {
            message: ModelMessage {
                role: MessageRole::Assistant,
                content: "Hello from agent via gateway".into(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            finish_reason: FinishReason::Stop,
            usage: UsageInfo {
                prompt_tokens: 10,
                completion_tokens: 10,
                total_tokens: 20,
                cost_usd: 0.0,
            },
            model_used: "gpt-4o".into(),
        })
    }
}

fn make_mission() -> Mission {
    Mission {
        id: MissionId(uuid::Uuid::now_v7()),
        owner_id: IdentityId(uuid::Uuid::now_v7()),
        objective: "gateway-e2e-mission".into(),
        scope: None,
        lifecycle_mode: LifecycleMode::Ephemeral,
        campaign_end: None,
        review_interval_days: None,
        budget: Budget {
            allocated_usd: 1000.0,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: Some(1000.0),
            recurring_monthly_usd: None,
        },
        risk: RiskLevel("low".into()),
        require_evidence: false,
        minimum_verifiers: 1,
        state: MissionState::Active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn make_agent(mission: &Mission) -> Agent {
    let now = chrono::Utc::now();
    Agent {
        id: AgentId(uuid::Uuid::now_v7()),
        identity_id: IdentityId(uuid::Uuid::now_v7()),
        mission_id: mission.id.clone(),
        parent_agent_id: None,
        lineage_id: LineageId(uuid::Uuid::now_v7()),
        name: "gateway-agent".into(),
        role: "worker".into(),
        genome: AgentGenome {
            id: "test-genome-1".into(),
            version: "1.0".into(),
            role: "worker".into(),
            lifecycle_modes: vec![LifecycleMode::Ephemeral],
            model_policy: ModelPolicy {
                preferred_profile: "gpt-4o".into(),
                fallback_profiles: vec![],
                max_context_tokens: 128_000,
            },
            autonomy: AutonomyConfig {
                can_spawn: false,
                max_spawn_depth: 1,
                max_children: 3,
            },
            delegable_permissions: vec![],
            non_delegable_permissions: vec![],
            verification_required: false,
            memory: MemoryConfig {
                default_read_scopes: vec![],
                default_write_scope: None,
            },
            runtime: RuntimeConfig {
                preferred_class: "local".into(),
                network: NetworkPolicy::AllowByDefault,
            },
        },
        state: AgentState::Active,
        lifecycle_mode: LifecycleMode::Ephemeral,
        persistent_pattern: None,
        budget: Budget {
            allocated_usd: 100.0,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: Some(1000.0),
            recurring_monthly_usd: None,
        },
        delegable_permissions: vec![],
        non_delegable_permissions: vec![],
        current_runtime: None,
        checkpoints: vec![],
        subscriptions: vec![],
        schedules: vec![],
        policy_bundle: claw10_domain::PolicyBundle {
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

async fn spawn_server(state: AppState) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = claw10_control_api::build_router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

#[tokio::test]
async fn test_webhook_routes_to_agent_and_dispatches_reply() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("claw10=debug")
        .try_init();

    // 1. Create shared KV store and AppState with mock model provider.
    let kv = Arc::new(claw10_store::InMemoryStore::new()) as Arc<dyn claw10_store::Store>;

    let mut registry = claw10_model_router::provider::ModelRegistry::new();
    registry.register(Box::new(MockProvider));
    let model_router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));
    let tool_registry = Arc::new(claw10_tool::registry::ToolRegistry::new());

    let state = AppState::new_with_services(kv.clone(), model_router, tool_registry);

    // 2. Create mission and agent, save to store.
    let mission = make_mission();
    let agent = make_agent(&mission);
    let agent_id = agent.id.clone();
    kv.set(&format!("mission:{}", mission.id.0), &mission).await.unwrap();
    kv.set(&format!("agent:{}", agent.id.0), &agent).await.unwrap();

    // 3. Start a local HTTP server to capture the outgoing webhook reply.
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
    async fn capture_server(Json(body): Json<serde_json::Value>) -> &'static str {
        CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        assert_eq!(body["recipient"], "98765");
        assert_eq!(body["body"], "Hello from agent via gateway");
        "ok"
    }
    let capture_app = Router::new().route("/capture", post(capture_server));
    let capture_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let _capture_addr = capture_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(capture_listener, capture_app).await.unwrap();
    });

    // 4. Register a Webhook channel bound to the agent.
    let server_addr = spawn_server(state.clone()).await;
    let register_resp: serde_json::Value = reqwest::Client::new()
        .post(format!("http://{server_addr}/v1/gateway/channels"))
        .json(&serde_json::json!({
            "channel_type": "Webhook",
            "config": {
                "url": format!("http://{}/capture", _capture_addr),
                "agent_id": agent_id.0.to_string()
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let channel_id = register_resp["id"].as_str().unwrap();

    // 5. Send incoming generic webhook.
    let resp = reqwest::Client::new()
        .post(format!(
            "http://{server_addr}/v1/gateway/webhooks/{channel_id}"
        ))
        .json(&serde_json::json!({
            "sender": "98765",
            "text": "ping"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 6. Wait for background agent execution + outbound dispatch.
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if CALL_COUNT.load(Ordering::SeqCst) == 1 {
            break;
        }
    }

    assert_eq!(
        CALL_COUNT.load(Ordering::SeqCst),
        1,
        "outgoing webhook should be dispatched once"
    );
}
