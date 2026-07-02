//! HTTP E2E Test: starts an Axum server on a random port and exercises
//! the full agent spawn → approve → lifecycle flow via HTTP endpoints.

use std::net::SocketAddr;

use claw10_control_api::state::AppState;
use claw10_store::StoreExt;

const AGENT_PREFIX: &str = "agent:";
const MISSION_PREFIX: &str = "mission:";
const SPAWNREQ_PREFIX: &str = "spawnreq:";

async fn spawn_server(state: AppState) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let app = claw10_control_api::build_router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

fn make_mission() -> claw10_domain::Mission {
    use claw10_domain::*;
    Mission {
        id: MissionId(uuid::Uuid::now_v7()),
        owner_id: IdentityId(uuid::Uuid::now_v7()),
        objective: "http-e2e-mission".into(),
        scope: None,
        lifecycle_mode: LifecycleMode::Ephemeral,
        campaign_end: None,
        review_interval_days: None,
        budget: Budget {
            allocated_usd: 1000.0,
            spent_usd: 0.0,
            soft_limit_usd: Some(800.0),
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

fn make_root_agent(mission: &claw10_domain::Mission) -> claw10_domain::Agent {
    use claw10_domain::*;
    let now = chrono::Utc::now();
    Agent {
        id: AgentId(uuid::Uuid::now_v7()),
        identity_id: IdentityId(uuid::Uuid::now_v7()),
        mission_id: mission.id.clone(),
        parent_agent_id: None,
        lineage_id: LineageId(uuid::Uuid::now_v7()),
        name: "http-root".into(),
        role: "root".into(),
        genome: AgentGenome {
            id: "root-genome".into(),
            version: "1.0".into(),
            role: "root".into(),
            lifecycle_modes: vec![LifecycleMode::Ephemeral],
            model_policy: ModelPolicy {
                preferred_profile: "gpt-4".into(),
                fallback_profiles: vec!["gpt-3.5".into()],
                max_context_tokens: 4096,
            },
            autonomy: AutonomyConfig {
                can_spawn: true,
                max_spawn_depth: 5,
                max_children: 10,
            },
            delegable_permissions: vec![
                Permission("read".into()),
                Permission("write".into()),
            ],
            non_delegable_permissions: vec![],
            memory: MemoryConfig {
                default_read_scopes: vec!["public".into()],
                default_write_scope: Some("agent-scope".into()),
            },
            runtime: RuntimeConfig {
                preferred_class: "standard".into(),
                network: NetworkPolicy::AllowByDefault,
            },
            verification_required: false,
        },
        state: AgentState::Active,
        lifecycle_mode: LifecycleMode::Ephemeral,
        persistent_pattern: None,
        budget: Budget {
            allocated_usd: 500.0,
            spent_usd: 0.0,
            soft_limit_usd: Some(400.0),
            hard_limit_usd: Some(500.0),
            recurring_monthly_usd: None,
        },
        delegable_permissions: vec![
            Permission("read".into()),
            Permission("write".into()),
        ],
        non_delegable_permissions: vec![],
        current_runtime: Some(RuntimeLease {
            worker_id: "worker-1".into(),
            acquired_at: now,
            expires_at: now + chrono::Duration::seconds(60),
            renewal_interval_seconds: 60,
        }),
        checkpoints: vec![],
        subscriptions: vec![],
        schedules: vec![],
        policy_bundle: PolicyBundle {
            id: PolicyBundleId(uuid::Uuid::now_v7()),
            name: "default".into(),
            version: "1.0".into(),
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

#[tokio::test]
async fn test_http_health_check() {
    let state = AppState::new();
    let addr = spawn_server(state).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    let resp = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_http_spawn_approve_lifecycle() {
    let state = AppState::new();
    let store = state.kv_store.clone();
    let addr = spawn_server(state).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // 1. Seed mission + agent directly via store (mission/agent endpoints are stubs)
    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    let root = make_root_agent(&mission);
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    // 2. GET /v1/agents — should list our seeded agent
    let resp = client.get(format!("{base}/v1/agents")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let agents: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["name"], "http-root");
    assert_eq!(agents[0]["state"], "Active");

    // 3. GET /v1/agents/{id} — get specific agent
    let resp = client
        .get(format!("{base}/v1/agents/{}", root.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let agent: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(agent["id"], root.id.0.to_string());
    assert_eq!(agent["name"], "http-root");

    // 4. POST /v1/spawn-requests — create a spawn request for 2 children
    let create_req = serde_json::json!({
        "mission_id": mission.id.0.to_string(),
        "requested_by": root.id.0.to_string(),
        "reason": "http e2e test",
        "children": [
            {
                "role": "scout",
                "objective": "scout the perimeter",
                "budget_usd": 50.0,
                "model_profile": "gpt-4",
                "max_turns": 50
            },
            {
                "role": "worker",
                "objective": "execute tasks",
                "budget_usd": 100.0,
                "model_profile": "gpt-4",
                "max_turns": 100
            }
        ]
    });

    let resp = client
        .post(format!("{base}/v1/spawn-requests"))
        .json(&create_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().await.unwrap();
    let spawn_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["state"], "Pending");

    // 5. GET /v1/spawn-requests — list spawn requests
    let resp = client
        .get(format!("{base}/v1/spawn-requests"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let requests: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["state"], "Pending");

    // 6. POST /v1/spawn-requests/{id}/approve — approve spawn
    let resp = client
        .post(format!("{base}/v1/spawn-requests/{spawn_id}/approve"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let approve: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(approve["state"], "approved");
    let children = approve["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["role"], "scout");
    assert_eq!(children[1]["role"], "worker");

    // 7. GET /v1/agents — should now have 3 agents (1 parent + 2 children)
    let resp = client.get(format!("{base}/v1/agents")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let agents_after: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(agents_after.len(), 3);

    // At least one should be Ready (the children)
    let ready_count = agents_after.iter().filter(|a| a["state"] == "Ready").count();
    assert_eq!(ready_count, 2);

    // 8. POST /v1/agents/{id}/pause — pause the root
    let resp = client
        .post(format!("{base}/v1/agents/{}/pause", root.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let paused: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(paused["state"], "Paused");

    // 9. POST /v1/agents/{id}/terminate — terminate the root
    let resp = client
        .post(format!("{base}/v1/agents/{}/terminate", root.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let terminated: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(terminated["state"], "Terminated");

    // 10. GET /v1/workers — should be empty initially
    let resp = client
        .get(format!("{base}/v1/workers"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let workers: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(workers.is_empty());

    // 11. POST /v1/workers — register a worker
    let worker_req = serde_json::json!({
        "name": "e2e-worker",
        "worker_type": "Local",
        "capabilities": ["shell"],
        "version": "1.0"
    });
    let resp = client
        .post(format!("{base}/v1/workers"))
        .json(&worker_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let worker: serde_json::Value = resp.json().await.unwrap();
    let worker_id = worker["id"].as_str().unwrap().to_string();
    assert_eq!(worker["name"], "e2e-worker");
    assert_eq!(worker["state"], "Online");

    // 12. POST /v1/workers/{id}/heartbeat
    let hb = serde_json::json!({
        "cpu_percent": 25.0,
        "memory_percent": 50.0,
        "active_runtimes": 2,
        "queue_depth": 0,
        "tool_availability": ["shell"],
        "sandbox_healthy": true
    });
    let resp = client
        .post(format!("{base}/v1/workers/{worker_id}/heartbeat"))
        .json(&hb)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 13. POST /v1/workers/{id}/drain
    let resp = client
        .post(format!("{base}/v1/workers/{worker_id}/drain"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let drained: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(drained["state"], "Draining");
    assert_eq!(drained["is_draining"], true);

    // 14. GET /v1/policies/compile — test policy stub
    let compile_req = serde_json::json!({ "source": "allow role:admin read *" });
    let resp = client
        .post(format!("{base}/v1/policies/compile"))
        .json(&compile_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 15. GET /v1/policies/evaluate — test policy evaluation via agent's bundle
    let eval_req = serde_json::json!({
        "bundle_id": root.policy_bundle.id.0.to_string(),
        "subject": { "Role": "admin" },
        "action": "read",
        "resource": "secrets"
    });
    let resp = client
        .post(format!("{base}/v1/policies/evaluate"))
        .json(&eval_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let eval_result: serde_json::Value = resp.json().await.unwrap();
    assert!(!eval_result["allowed"].as_bool().unwrap());
    assert!(eval_result["reason"].as_str().unwrap().contains("default deny"));
}

#[tokio::test]
async fn test_http_gateway_memory_scheduler() {
    let state = AppState::new();
    let addr = spawn_server(state).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // 1. Register a gateway channel
    let channel_req = serde_json::json!({
        "channel_type": "Webhook",
        "config": { "url": "https://example.com/hook" }
    });
    let resp = client
        .post(format!("{base}/v1/gateway/channels"))
        .json(&channel_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let channel: serde_json::Value = resp.json().await.unwrap();
    let _channel_id = channel["id"].as_str().unwrap().to_string();
    assert_eq!(channel["channel_type"], "Webhook");
    assert_eq!(channel["is_active"], true);

    // 2. List channels
    let resp = client
        .get(format!("{base}/v1/gateway/channels"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let channels: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(channels.len(), 1);

    // 3. Store a memory
    let agent_id = uuid::Uuid::now_v7().to_string();
    let task_id = uuid::Uuid::now_v7().to_string();
    let store_req = serde_json::json!({
        "tenant_id": "e2e-tenant",
        "scope": "public",
        "memory_type": "Episodic",
        "content": "e2e http test memory",
        "source_agent": agent_id,
        "source_task": task_id,
        "confidence": 0.85,
        "classification": "test"
    });
    let resp = client
        .post(format!("{base}/v1/memories"))
        .json(&store_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let memory: serde_json::Value = resp.json().await.unwrap();
    let memory_id = memory["id"].as_str().unwrap().to_string();
    assert_eq!(memory["status"], "Active");

    // 4. Get memory by ID
    let resp = client
        .get(format!("{base}/v1/memories/{memory_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 5. Query memories
    let resp = client
        .get(format!("{base}/v1/memories?scope=public"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let memories: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(memories.len(), 1);

    // 6. Health check
    let resp = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_http_lineage_tracking() {
    let state = AppState::new();
    let store = state.kv_store.clone();
    let addr = spawn_server(state).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // Seed mission + root agent (no parent, has lineage_id)
    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    let root = make_root_agent(&mission);
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    // Create and approve spawn to generate lineage entries
    let create_req = serde_json::json!({
        "mission_id": mission.id.0.to_string(),
        "requested_by": root.id.0.to_string(),
        "reason": "lineage test",
        "children": [
            {
                "role": "scout",
                "objective": "test",
                "budget_usd": 30.0,
                "model_profile": "gpt-4",
                "max_turns": 20
            }
        ]
    });
    let resp = client
        .post(format!("{base}/v1/spawn-requests"))
        .json(&create_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let spawn_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .post(format!("{base}/v1/spawn-requests/{spawn_id}/approve"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET /v1/lineages/{lineage_id}
    let resp = client
        .get(format!("{base}/v1/lineages/{}", root.lineage_id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let lineage: serde_json::Value = resp.json().await.unwrap();
    assert!(lineage["id"].is_string());
    assert!(lineage["entries"].is_array());

    // GET /v1/agents/{id}/legacy
    let resp = client
        .get(format!("{base}/v1/agents/{}/legacy", root.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET /v1/agents — should now have 2 agents
    let resp = client.get(format!("{base}/v1/agents")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let agents: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(agents.len(), 2);
}

#[tokio::test]
async fn test_http_schedule_crud() {
    let state = AppState::new();
    let store = state.kv_store.clone();
    let addr = spawn_server(state).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // Seed an agent
    let mission = make_mission();
    store
        .set(&format!("{MISSION_PREFIX}{}", mission.id.0), &mission)
        .await
        .unwrap();

    let agent = make_root_agent(&mission);
    let agent_key = format!("{AGENT_PREFIX}{}", agent.id.0);
    store.set(&agent_key, &agent).await.unwrap();

    // POST /v1/agents/{agent_id}/schedules — add a schedule
    let add_req = serde_json::json!({
        "cron": "0 0 * * * *",
        "timezone": "UTC",
        "action": "Wake"
    });
    let resp = client
        .post(format!("{base}/v1/agents/{}/schedules", agent.id.0))
        .json(&add_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(created["action"], "Wake");
    assert_eq!(created["cron"], "0 0 * * * *");
    assert_eq!(created["timezone"], "UTC");
    assert_eq!(created["index"], 0);

    // Add a second schedule
    let add_req2 = serde_json::json!({
        "cron": "*/30 * * * * *",
        "timezone": "UTC",
        "action": "Review"
    });
    let resp = client
        .post(format!("{base}/v1/agents/{}/schedules", agent.id.0))
        .json(&add_req2)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created2: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(created2["index"], 1);
    assert_eq!(created2["action"], "Review");

    // GET /v1/agents/{agent_id}/schedules — list
    let resp = client
        .get(format!("{base}/v1/agents/{}/schedules", agent.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let schedules: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(schedules.len(), 2);
    assert_eq!(schedules[0]["index"], 0);
    assert_eq!(schedules[0]["action"], "Wake");
    assert_eq!(schedules[1]["index"], 1);
    assert_eq!(schedules[1]["action"], "Review");

    // DELETE /v1/agents/{agent_id}/schedules/{index} — remove first schedule
    let resp = client
        .delete(format!("{base}/v1/agents/{}/schedules/0", agent.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let removed: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(removed["status"], "removed");

    // Verify only 1 schedule remains
    let resp = client
        .get(format!("{base}/v1/agents/{}/schedules", agent.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let schedules_after: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(schedules_after.len(), 1);
    assert_eq!(schedules_after[0]["action"], "Review");

    // GET /v1/schedules/due
    let resp = client
        .get(format!("{base}/v1/schedules/due"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let due: Vec<serde_json::Value> = resp.json().await.unwrap();
    // The Review schedule with cron */30 may or may not be due right now
    // but the endpoint should return 200 regardless
    assert!(due.iter().all(|s| s["agent_id"].as_str().is_some()));
}

#[tokio::test]
async fn test_http_lifecycle_and_gateway() {
    let state = AppState::new();
    let store = state.kv_store.clone();
    let addr = spawn_server(state).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // ── Seed: mission + agent ──────────────────────────────────────
    let mission = make_mission();
    store
        .set(&format!("{MISSION_PREFIX}{}", mission.id.0), &mission)
        .await
        .unwrap();

    // Create agent with Active state and runtime lease (needed for hibernate)
    let mut agent = make_root_agent(&mission);
    agent.state = claw10_domain::AgentState::Active;
    agent.current_runtime = Some(claw10_domain::RuntimeLease {
        worker_id: "worker-1".into(),
        acquired_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::seconds(60),
        renewal_interval_seconds: 60,
    });
    let agent_key = format!("{AGENT_PREFIX}{}", agent.id.0);
    store.set(&agent_key, &agent).await.unwrap();

    // ── 1. POST /v1/agents/{id}/hibernate ──────────────────────────
    let resp = client
        .post(format!("{base}/v1/agents/{}/hibernate", agent.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let hibernate_resp: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(hibernate_resp["reason"], "PreHibernation");
    assert!(!hibernate_resp["id"].as_str().unwrap().is_empty());

    // Verify agent state is now Hibernating
    let resp = client
        .get(format!("{base}/v1/agents/{}", agent.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let agent_get: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(agent_get["state"], "Hibernating");

    // ── 2. POST /v1/agents/{id}/wake ───────────────────────────────
    let wake_req = serde_json::json!({
        "worker_id": "worker-2",
        "renewal_interval_seconds": 120
    });
    let resp = client
        .post(format!("{base}/v1/agents/{}/wake", agent.id.0))
        .json(&wake_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let wake_resp: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(wake_resp["status"], "woken");

    // Verify agent is now Active again
    let resp = client
        .get(format!("{base}/v1/agents/{}", agent.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let agent_after_wake: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(agent_after_wake["state"], "Active");

    // ── 3. POST /v1/agents/{id}/heartbeat ──────────────────────────
    let resp = client
        .post(format!("{base}/v1/agents/{}/heartbeat", agent.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let hb: serde_json::Value = resp.json().await.unwrap();
    assert!(hb["remaining_seconds"].as_i64().unwrap() > 0);

    // ── 4. POST /v1/agents/{id}/checkpoints ────────────────────────
    let cp_req = serde_json::json!({ "reason": "Periodic" });
    let resp = client
        .post(format!("{base}/v1/agents/{}/checkpoints", agent.id.0))
        .json(&cp_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let cp: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(cp["reason"], "Periodic");
    assert!(!cp["id"].as_str().unwrap().is_empty());

    // GET /v1/agents/{id}/checkpoints — should have at least 2 (hibernate + periodic)
    let resp = client
        .get(format!("{base}/v1/agents/{}/checkpoints", agent.id.0))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let checkpoints: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(checkpoints.len() >= 2);

    // ── 5. Gateway: Register channel → activate/deactivate ─────────
    let chan_req = serde_json::json!({
        "channel_type": "Webhook",
        "config": { "url": "https://example.com/hook2" }
    });
    let resp = client
        .post(format!("{base}/v1/gateway/channels"))
        .json(&chan_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let channel: serde_json::Value = resp.json().await.unwrap();
    let channel_id = channel["id"].as_str().unwrap().to_string();
    assert_eq!(channel["is_active"], true);

    // Deactivate
    let resp = client
        .post(format!("{base}/v1/gateway/channels/{channel_id}/deactivate"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let deact: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(deact["status"], "deactivated");

    // Reactivate
    let resp = client
        .post(format!("{base}/v1/gateway/channels/{channel_id}/activate"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let act: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(act["status"], "activated");

    // Get channel
    let resp = client
        .get(format!("{base}/v1/gateway/channels/{channel_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let chan_get: serde_json::Value = resp.json().await.unwrap();
    assert!(chan_get["is_active"].as_bool().unwrap());

    // ── 6. Gateway: Sessions — create, get, terminate ──────────────
    let session_req = serde_json::json!({
        "identity_id": uuid::Uuid::now_v7().to_string(),
        "channel_id": channel_id,
        "ttl_seconds": 3600
    });
    let resp = client
        .post(format!("{base}/v1/gateway/sessions"))
        .json(&session_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let session: serde_json::Value = resp.json().await.unwrap();
    let session_id = session["id"].as_str().unwrap().to_string();
    assert_eq!(session["state"], "Active");

    // Get session
    let resp = client
        .get(format!("{base}/v1/gateway/sessions/{session_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let session_get: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(session_get["state"], "Active");
    assert_eq!(session_get["id"], session_id);

    // Terminate session
    let resp = client
        .post(format!("{base}/v1/gateway/sessions/{session_id}/terminate"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let term: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(term["status"], "terminated");

    // ── 7. Memory: store → update → verify → transition ────────────
    let agent_id_uuid = agent.id.0.to_string();
    let task_id_uuid = uuid::Uuid::now_v7().to_string();

    let mem_req = serde_json::json!({
        "tenant_id": "e2e-test",
        "scope": "private",
        "memory_type": "Episodic",
        "content": "original content",
        "source_agent": agent_id_uuid,
        "source_task": task_id_uuid,
        "confidence": 0.9,
        "classification": "lifecycle-test"
    });
    let resp = client
        .post(format!("{base}/v1/memories"))
        .json(&mem_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let mem: serde_json::Value = resp.json().await.unwrap();
    let memory_id = mem["id"].as_str().unwrap().to_string();
    assert_eq!(mem["status"], "Active");

    // PUT /v1/memories/{id} — update content
    let update_req = serde_json::json!({ "content": "updated content" });
    let resp = client
        .put(format!("{base}/v1/memories/{memory_id}"))
        .json(&update_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let upd: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(upd["status"], "updated");

    // POST /v1/memories/{id}/verify
    let verify_req = serde_json::json!({ "verifier": agent.id.0.to_string() });
    let resp = client
        .post(format!("{base}/v1/memories/{memory_id}/verify"))
        .json(&verify_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let ver: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(ver["status"], "verified");

    // POST /v1/memories/{id}/transition — Active → Expired
    let trans_req = serde_json::json!({ "status": "Expired" });
    let resp = client
        .post(format!("{base}/v1/memories/{memory_id}/transition"))
        .json(&trans_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let trans: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(trans["status"], "transitioned");

    // GET /v1/memories/counts
    let resp = client
        .get(format!("{base}/v1/memories/counts"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let counts: serde_json::Value = resp.json().await.unwrap();
    assert!(counts.as_object().unwrap().contains_key("Expired"));

    // DELETE /v1/memories/{id} — cleanup
    let resp = client
        .delete(format!("{base}/v1/memories/{memory_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let del: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(del["status"], "deleted");

    // ── 8. Worker: get, mark_offline, quarantine, stale, counts ────
    let worker_req = serde_json::json!({
        "name": "lifecycle-worker",
        "worker_type": "Local",
        "capabilities": ["shell"],
        "version": "1.0"
    });
    let resp = client
        .post(format!("{base}/v1/workers"))
        .json(&worker_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let worker: serde_json::Value = resp.json().await.unwrap();
    let worker_id = worker["id"].as_str().unwrap().to_string();

    // GET /v1/workers/{id}
    let resp = client
        .get(format!("{base}/v1/workers/{worker_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let w_get: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(w_get["name"], "lifecycle-worker");

    // POST /v1/workers/{id}/offline
    let resp = client
        .post(format!("{base}/v1/workers/{worker_id}/offline"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let off: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(off["status"], "marked offline");

    // POST /v1/workers/{id}/quarantine
    let resp = client
        .post(format!("{base}/v1/workers/{worker_id}/quarantine"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let q: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(q["status"], "quarantined");

    // GET /v1/workers/stale
    let resp = client
        .get(format!("{base}/v1/workers/stale"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let stale: Vec<serde_json::Value> = resp.json().await.unwrap();
    // Quarantined workers shouldn't appear as stale
    assert!(stale.is_empty());

    // GET /v1/workers/counts
    let resp = client
        .get(format!("{base}/v1/workers/counts"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let counts_w: serde_json::Value = resp.json().await.unwrap();
    assert!(counts_w.as_object().unwrap().contains_key("Quarantined"));

    // Also register a fresh worker for health check
    let worker_hb_req = serde_json::json!({
        "name": "hb-check",
        "worker_type": "Local",
        "capabilities": ["shell"],
        "version": "1.0"
    });
    let resp = client
        .post(format!("{base}/v1/workers"))
        .json(&worker_hb_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let w_hb = resp.json::<serde_json::Value>().await.unwrap();
    let w_hb_id = w_hb["id"].as_str().unwrap().to_string();

    // Heartbeat
    let hb_req = serde_json::json!({
        "cpu_percent": 10.0,
        "memory_percent": 30.0,
        "active_runtimes": 1,
        "queue_depth": 0,
        "tool_availability": ["shell"],
        "sandbox_healthy": true
    });
    let resp = client
        .post(format!("{base}/v1/workers/{w_hb_id}/heartbeat"))
        .json(&hb_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // GET /v1/workers/stale — now not stale
    let resp = client
        .get(format!("{base}/v1/workers/stale"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Final health check
    let resp = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_http_spawn_deny() {
    let state = AppState::new();
    let store = state.kv_store.clone();
    let addr = spawn_server(state).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // Seed mission + root agent
    let mission = make_mission();
    store.set(&format!("{MISSION_PREFIX}{}", mission.id.0), &mission).await.unwrap();

    let root = make_root_agent(&mission);
    store.set(&format!("{AGENT_PREFIX}{}", root.id.0), &root).await.unwrap();

    // POST /v1/spawn-requests
    let create_req = serde_json::json!({
        "mission_id": mission.id.0.to_string(),
        "requested_by": root.id.0.to_string(),
        "reason": "http deny test",
        "children": [
            {
                "role": "scout",
                "objective": "test deny",
                "budget_usd": 10.0,
                "model_profile": "gpt-4",
                "max_turns": 10
            }
        ]
    });

    let resp = client
        .post(format!("{base}/v1/spawn-requests"))
        .json(&create_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().await.unwrap();
    let spawn_id = created["id"].as_str().unwrap().to_string();

    // Verifikasi data ada di KV store menggunakan SPAWNREQ_PREFIX untuk menghilangkan warning unused constant
    let stored_req: claw10_domain::SpawnRequest = store
        .get(&format!("{SPAWNREQ_PREFIX}{spawn_id}"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_req.state, claw10_domain::SpawnState::Pending);

    // POST /v1/spawn-requests/{id}/deny
    let resp = client
        .post(format!("{base}/v1/spawn-requests/{spawn_id}/deny"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let deny_resp: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(deny_resp["state"], "denied");

    // GET /v1/spawn-requests/{id} -> verify state
    let resp = client
        .get(format!("{base}/v1/spawn-requests/{spawn_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let get_resp: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(get_resp["state"], "Denied");
}

#[tokio::test]
async fn test_http_spawn_validation_failures() {
    let state = AppState::new();
    let store = state.kv_store.clone();
    let addr = spawn_server(state).await;
    let client = reqwest::Client::new();
    let base = format!("http://{addr}");

    // Seed mission + root agent
    let mission = make_mission();
    store.set(&format!("{MISSION_PREFIX}{}", mission.id.0), &mission).await.unwrap();

    let mut root = make_root_agent(&mission);
    // Beri budget kecil sekali agar gagal validasi budget
    root.budget.spent_usd = 495.0; // sisa 5.0
    store.set(&format!("{AGENT_PREFIX}{}", root.id.0), &root).await.unwrap();

    // 1. Create spawn request dengan budget 10.0 (melebihi 5.0)
    let create_req = serde_json::json!({
        "mission_id": mission.id.0.to_string(),
        "requested_by": root.id.0.to_string(),
        "reason": "http budget failure test",
        "children": [
            {
                "role": "scout",
                "objective": "test budget fail",
                "budget_usd": 10.0,
                "model_profile": "gpt-4",
                "max_turns": 10
            }
        ]
    });

    let resp = client
        .post(format!("{base}/v1/spawn-requests"))
        .json(&create_req)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().await.unwrap();
    let spawn_id = created["id"].as_str().unwrap().to_string();

    // 2. Approve spawn -> should fail with 500 (SpawnFailed karena budget tidak cukup)
    let resp = client
        .post(format!("{base}/v1/spawn-requests/{spawn_id}/approve"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 500);

    // 3. Deny spawn request yang sudah gagal tadi -> should succeed (karena masih Pending)
    let resp = client
        .post(format!("{base}/v1/spawn-requests/{spawn_id}/deny"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 4. Deny lagi spawn request yang sudah Denied -> should fail with 400 (Validation)
    let resp = client
        .post(format!("{base}/v1/spawn-requests/{spawn_id}/deny"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // 5. Approve spawn request yang sudah Denied -> should fail with 400 (Validation)
    let resp = client
        .post(format!("{base}/v1/spawn-requests/{spawn_id}/approve"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

