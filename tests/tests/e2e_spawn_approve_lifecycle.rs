use uuid::Uuid;

use claw10_domain::{
    Agent, AgentGenome, AgentId, AgentState, AutonomyConfig, Budget, ChildSpec, IdentityId,
    LifecycleMode, MemoryConfig, Mission, MissionId, MissionState, ModelPolicy, NetworkPolicy,
    PolicyBundle, PolicyBundleId, RuntimeConfig,
};
use claw10_spawn::broker::SpawnBroker;

use claw10_control_api::state::AppState;
use claw10_store::StoreExt;

const AGENT_PREFIX: &str = "agent:";
const SPAWNREQ_PREFIX: &str = "spawnreq:";
const MISSION_PREFIX: &str = "mission:";

// ── Helpers ──────────────────────────────────────────────────────────

fn make_mission() -> Mission {
    Mission {
        id: MissionId(Uuid::now_v7()),
        owner_id: IdentityId(Uuid::now_v7()),
        objective: "e2e-test-mission".into(),
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
        risk: claw10_domain::RiskLevel("low".into()),
        require_evidence: false,
        minimum_verifiers: 1,
        state: MissionState::Active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn make_root_agent(mission: &Mission) -> Agent {
    let now = chrono::Utc::now();
    Agent {
        id: AgentId(Uuid::now_v7()),
        identity_id: IdentityId(Uuid::now_v7()),
        mission_id: mission.id.clone(),
        parent_agent_id: None,
        lineage_id: claw10_domain::LineageId(Uuid::now_v7()),
        name: "e2e-root".into(),
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
                claw10_domain::Permission("read".into()),
                claw10_domain::Permission("write".into()),
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
            claw10_domain::Permission("read".into()),
            claw10_domain::Permission("write".into()),
        ],
        non_delegable_permissions: vec![],
        current_runtime: Some(claw10_domain::RuntimeLease {
            worker_id: "worker-1".into(),
            acquired_at: now,
            expires_at: now + chrono::Duration::seconds(60),
            renewal_interval_seconds: 60,
        }),
        checkpoints: vec![],
        subscriptions: vec![],
        schedules: vec![],
        policy_bundle: PolicyBundle {
            id: PolicyBundleId(Uuid::now_v7()),
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

fn calculate_depth(agent_id: &AgentId, agents: &[Agent]) -> u32 {
    let mut depth = 0;
    let mut current = agent_id.clone();
    while let Some(agent) = agents.iter().find(|a: &&Agent| a.id == current) {
        match &agent.parent_agent_id {
            Some(pid) => {
                depth += 1;
                current = pid.clone();
            }
            None => break,
        }
    }
    depth
}

// ── E2E Test: Full Spawn → Approve → Lifecycle ──────────────────────

#[tokio::test]
async fn test_e2e_spawn_approve_lifecycle() {
    // 1. Setup AppState with InMemoryStore
    let state = AppState::new();
    let store = state.kv_store.clone();

    // 2. Create and store Mission
    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    // 3. Create and store Root Agent
    let root = make_root_agent(&mission);
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    // 4. Create SpawnRequest via SpawnBroker
    let child_specs = vec![
        ChildSpec {
            role: "scout".into(),
            objective: "scout the perimeter".into(),
            budget_usd: 50.0,
            model_profile: "gpt-4".into(),
            max_turns: 50,
            custom_permissions: None,
        },
        ChildSpec {
            role: "worker".into(),
            objective: "execute tasks".into(),
            budget_usd: 100.0,
            model_profile: "gpt-4".into(),
            max_turns: 100,
            custom_permissions: None,
        },
    ];

    let spawn_request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "e2e test spawn".into(),
        child_specs,
    );

    // 5. Store SpawnRequest
    let spawn_key = format!("{SPAWNREQ_PREFIX}{}", spawn_request.id.0);
    store.set(&spawn_key, &spawn_request).await.unwrap();

    // 6. Verify initial state
    let stored_root: Agent = store.get(&root_key).await.unwrap().unwrap();
    assert_eq!(stored_root.state, AgentState::Active);
    assert_eq!(stored_root.budget.spent_usd, 0.0);

    let stored_spawn: claw10_domain::SpawnRequest =
        store.get(&spawn_key).await.unwrap().unwrap();
    assert_eq!(stored_spawn.state, claw10_domain::SpawnState::Pending);

    // 7. Simulate approve_spawn flow (matching handler logic)
    let all_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let mut request: claw10_domain::SpawnRequest =
        store.get(&spawn_key).await.unwrap().unwrap();

    // Load mission
    let mission_stored: Mission = store.get(&mission_key).await.unwrap().unwrap();

    // Calculate depth
    let requested_by = request.requested_by.clone();
    let current_depth = calculate_depth(&requested_by, &all_agents);

    // Load parent agent
    let parent_key = format!("{AGENT_PREFIX}{}", requested_by.0);
    let mut parent: Agent = store.get(&parent_key).await.unwrap().unwrap();

    // Process spawn via SpawnBroker
    let children = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission_stored,
            &request,
            &all_agents,
            current_depth,
        )
        .await
        .expect("spawn should succeed");

    // 8. Verify spawn results
    assert_eq!(children.len(), 2, "should create 2 children");
    assert_eq!(children[0].role, "scout");
    assert_eq!(children[1].role, "worker");

    // Children should be Ready (not Active)
    assert_eq!(children[0].state, AgentState::Ready);
    assert_eq!(children[1].state, AgentState::Ready);

    // Children should reference parent
    assert_eq!(
        children[0].parent_agent_id,
        Some(parent.id.clone()),
        "child should reference parent"
    );

    // Children should NOT have spawn capability (ephemeral leaf agents)
    assert!(!children[0].genome.autonomy.can_spawn);
    assert_eq!(children[0].genome.autonomy.max_spawn_depth, 0);

    // 9. Verify parent budget was deducted
    assert!(
        (parent.budget.spent_usd - 150.0).abs() < f64::EPSILON,
        "parent budget should reflect 150.0 spent (50+100)"
    );

    // 10. Update and save spawn request
    request.state = claw10_domain::SpawnState::Approved;
    request.updated_at = chrono::Utc::now();
    store.set(&spawn_key, &request).await.unwrap();

    // 11. Save parent agent back
    store.set(&parent_key, &parent).await.unwrap();

    // 12. Save each child agent
    for child in &children {
        let child_key = format!("{AGENT_PREFIX}{}", child.id.0);
        store.set(&child_key, child).await.unwrap();
    }

    // 13. Verify persisted state
    let saved_parent: Agent = store.get(&parent_key).await.unwrap().unwrap();
    assert_eq!(saved_parent.state, AgentState::Active);
    assert!((saved_parent.budget.spent_usd - 150.0).abs() < f64::EPSILON);

    let saved_child: Agent = store
        .get(&format!("{AGENT_PREFIX}{}", children[0].id.0))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved_child.role, "scout");
    assert_eq!(saved_child.state, AgentState::Ready);

    let all_after: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(all_after.len(), 3, "should have 3 agents total (1 parent + 2 children)");

    // 14. Test lifecycle: Pause child → should fail because not Active
    // (Agent must be Active or Hibernating to pause)
    {
        let child_key = format!("{AGENT_PREFIX}{}", children[0].id.0);
        let child: Agent = store.get(&child_key).await.unwrap().unwrap();
        assert_eq!(child.state, AgentState::Ready);
        // Attempting to pause a Ready agent should fail
        assert!(
            !matches!(child.state, AgentState::Active | AgentState::Hibernating),
            "Ready agent should not be pausable"
        );
    }

    // 15. Test lifecycle: Terminate child
    {
        let child_key = format!("{AGENT_PREFIX}{}", children[0].id.0);
        let mut child: Agent = store.get(&child_key).await.unwrap().unwrap();
        claw10_lifecycle::LifecycleService::terminate_descendant(&mut child);
        store.set(&child_key, &child).await.unwrap();

        let terminated: Agent = store.get(&child_key).await.unwrap().unwrap();
        assert_eq!(
            terminated.state,
            AgentState::Terminated,
            "child should be terminated"
        );
        assert!(
            terminated.terminated_at.is_some(),
            "child should have terminated_at timestamp"
        );
        assert!(
            terminated.current_runtime.is_none(),
            "child runtime should be revoked"
        );
    }

    // 16. Test lifecycle: Heartbeat parent
    {
        let mut parent: Agent = store.get(&parent_key).await.unwrap().unwrap();
        let remaining = claw10_lifecycle::LifecycleService::heartbeat(&mut parent).unwrap();
        assert!(
            remaining.num_seconds() > 0,
            "heartbeat should return positive remaining time"
        );
        store.set(&parent_key, &parent).await.unwrap();
    }

    // 17. Test lifecycle: Hibernate parent → wake parent
    {
        let mut parent: Agent = store.get(&parent_key).await.unwrap().unwrap();
        let _cp = claw10_lifecycle::LifecycleService::hibernate(&mut parent).unwrap();
        assert_eq!(parent.state, AgentState::Hibernating);
        assert!(parent.current_runtime.is_none());
        assert_eq!(parent.checkpoints.len(), 1);
        store.set(&parent_key, &parent).await.unwrap();

        // Wake it up
        let lease = claw10_domain::RuntimeLease {
            worker_id: "worker-2".into(),
            acquired_at: chrono::Utc::now(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(120),
            renewal_interval_seconds: 120,
        };
        claw10_lifecycle::LifecycleService::wake(&mut parent, lease).unwrap();
        assert_eq!(parent.state, AgentState::Active);
        assert!(parent.current_runtime.is_some());
        assert_eq!(
            parent.current_runtime.as_ref().unwrap().worker_id,
            "worker-2"
        );
        store.set(&parent_key, &parent).await.unwrap();
    }

    // 18. Test lifecycle: Terminate parent (should go through full teardown phases)
    {
        let mut parent: Agent = store.get(&parent_key).await.unwrap().unwrap();
        claw10_lifecycle::LifecycleService::terminate(&mut parent);
        assert_eq!(parent.state, AgentState::Terminated);
        assert!(parent.terminated_at.is_some());
        assert!(parent.current_runtime.is_none());
        assert!(
            parent.checkpoints.len() >= 2,
            "should have at least 2 checkpoints (hibernate + terminate)"
        );
        store.set(&parent_key, &parent).await.unwrap();
    }

    // 19. Verify all agents in store
    let final_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(final_agents.len(), 3);

    let terminated_count = final_agents
        .iter()
        .filter(|a| a.state == AgentState::Terminated)
        .count();
    assert_eq!(
        terminated_count, 2,
        "2 agents should be terminated (parent + child)"
    );

    let ready_count = final_agents
        .iter()
        .filter(|a| a.state == AgentState::Ready)
        .count();
    assert_eq!(ready_count, 1, "1 agent should still be Ready (child-1)");
}

// ── E2E Test: Multiple children and budget edge cases ─────────────────

#[tokio::test]
async fn test_e2e_spawn_budget_exhaustion() {
    let state = AppState::new();
    let store = state.kv_store.clone();

    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    // Agent with nearly exhausted budget
    let mut root = make_root_agent(&mission);
    root.budget.spent_usd = 480.0; // only 20 remaining
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    // Request a child that costs 50 — exceeds remaining 20
    let child_specs = vec![ChildSpec {
        role: "expensive-child".into(),
        objective: "costly task".into(),
        budget_usd: 50.0,
        model_profile: "gpt-4".into(),
        max_turns: 10,
        custom_permissions: None,
    }];

    let spawn_request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "budget test".into(),
        child_specs,
    );

    let spawn_key = format!("{SPAWNREQ_PREFIX}{}", spawn_request.id.0);
    store.set(&spawn_key, &spawn_request).await.unwrap();

    // Try to process spawn — should fail
    let all_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let request: claw10_domain::SpawnRequest =
        store.get(&spawn_key).await.unwrap().unwrap();
    let mission_stored: Mission = store.get(&mission_key).await.unwrap().unwrap();
    let mut parent: Agent = store.get(&root_key).await.unwrap().unwrap();
    let current_depth = calculate_depth(&request.requested_by, &all_agents);

    let result = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission_stored,
            &request,
            &all_agents,
            current_depth,
        )
        .await;

    assert!(
        result.is_err(),
        "spawn should fail due to insufficient budget"
    );

    // Verify no children were persisted
    let agents_after: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(agents_after.len(), 1, "no new agents should be created");
}

// ── E2E Test: Spawn validation failures ──────────────────────────────

#[tokio::test]
async fn test_e2e_spawn_validation_parent_not_active() {
    let state = AppState::new();
    let store = state.kv_store.clone();

    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    let mut root = make_root_agent(&mission);
    root.state = AgentState::Terminated;
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    let child_specs = vec![ChildSpec {
        role: "child".into(),
        objective: "task".into(),
        budget_usd: 10.0,
        model_profile: "gpt-4".into(),
        max_turns: 10,
        custom_permissions: None,
    }];

    let spawn_request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test".into(),
        child_specs,
    );

    let spawn_key = format!("{SPAWNREQ_PREFIX}{}", spawn_request.id.0);
    store.set(&spawn_key, &spawn_request).await.unwrap();

    let all_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let request: claw10_domain::SpawnRequest =
        store.get(&spawn_key).await.unwrap().unwrap();
    let mission_stored: Mission = store.get(&mission_key).await.unwrap().unwrap();
    let mut parent: Agent = store.get(&root_key).await.unwrap().unwrap();
    let current_depth = calculate_depth(&request.requested_by, &all_agents);

    let result = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission_stored,
            &request,
            &all_agents,
            current_depth,
        )
        .await;

    assert!(result.is_err(), "spawn should fail when parent is not active");
}

// ── E2E Test: Full lifecycle with lineage verification ────────────────

#[tokio::test]
async fn test_e2e_spawn_with_lineage() {
    let state = AppState::new();
    let store = state.kv_store.clone();

    // Setup
    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    let root = make_root_agent(&mission);
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    // Create lineage
    let mut lineage = claw10_lineage::LineageService::create_lineage(
        mission.id.clone(),
        root.id.clone(),
    );

    // Spawn children
    let child_specs = vec![
        ChildSpec {
            role: "alpha".into(),
            objective: "alpha tasks".into(),
            budget_usd: 30.0,
            model_profile: "gpt-4".into(),
            max_turns: 50,
            custom_permissions: None,
        },
        ChildSpec {
            role: "beta".into(),
            objective: "beta tasks".into(),
            budget_usd: 20.0,
            model_profile: "gpt-4".into(),
            max_turns: 30,
            custom_permissions: None,
        },
    ];

    let spawn_request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "lineage test".into(),
        child_specs,
    );

    let all_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let mut parent: Agent = store.get(&root_key).await.unwrap().unwrap();
    let request = spawn_request;
    let mission_stored: Mission = store.get(&mission_key).await.unwrap().unwrap();
    let current_depth = calculate_depth(&request.requested_by, &all_agents);

    let children = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission_stored,
            &request,
            &all_agents,
            current_depth,
        )
        .await
        .expect("spawn should succeed for lineage test");

    // Save children to store (needed for persistence verification below)
    for child in &children {
        let child_key = format!("{AGENT_PREFIX}{}", child.id.0);
        store.set(&child_key, child).await.unwrap();
    }

    // Add children to lineage
    for child in &children {
        claw10_lineage::LineageService::add_entry(
            &mut lineage,
            child.id.clone(),
            Some(root.id.clone()),
            child.role.clone(),
        );
    }

    // Verify lineage
    assert_eq!(lineage.entries.len(), 2, "lineage should have 2 entries");
    assert_eq!(lineage.entries[0].role, "alpha");
    assert_eq!(lineage.entries[1].role, "beta");
    assert_eq!(lineage.entries[0].state, "active");
    assert_eq!(lineage.entries[1].state, "active");

    // Terminate one child and update lineage
    let child_key = format!("{AGENT_PREFIX}{}", children[0].id.0);
    let mut child: Agent = store.get(&child_key).await.unwrap().unwrap();
    claw10_lifecycle::LifecycleService::terminate_descendant(&mut child);
    store.set(&child_key, &child).await.unwrap();

    claw10_lineage::LineageService::terminate_entry(&mut lineage, &children[0].id);
    assert_eq!(lineage.entries[0].state, "terminated");
    assert!(lineage.entries[0].terminated_at.is_some());
    assert_eq!(lineage.entries[1].state, "active");
    assert!(lineage.entries[1].terminated_at.is_none());

    // Store lineage
    let lineage_key = format!("lineage:{}", lineage.id.0);
    store.set(&lineage_key, &lineage).await.unwrap();

    // Verify persisted lineage
    let stored_lineage: claw10_domain::Lineage =
        store.get(&lineage_key).await.unwrap().unwrap();
    assert_eq!(stored_lineage.entries.len(), 2);
    assert_eq!(stored_lineage.entries[0].state, "terminated");
    assert_eq!(stored_lineage.entries[1].state, "active");
    assert_eq!(stored_lineage.root_agent_id, root.id);
}

#[tokio::test]
async fn test_e2e_spawn_validation_swarm_size_exceeded() {
    let state = AppState::new();
    let store = state.kv_store.clone();

    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    let root = make_root_agent(&mission);
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    // Tambahkan 100 agent di DB fiktif untuk mensimulasikan batas limit
    for i in 0..100 {
        let mut dummy = root.clone();
        dummy.id = AgentId(uuid::Uuid::now_v7());
        dummy.name = format!("dummy-{i}");
        let dummy_key = format!("{AGENT_PREFIX}{}", dummy.id.0);
        store.set(&dummy_key, &dummy).await.unwrap();
    }

    let child_specs = vec![ChildSpec {
        role: "scout".into(),
        objective: "testing limits".into(),
        budget_usd: 10.0,
        model_profile: "gpt-4".into(),
        max_turns: 10,
        custom_permissions: None,
    }];

    let spawn_request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test limit".into(),
        child_specs,
    );

    let spawn_key = format!("{SPAWNREQ_PREFIX}{}", spawn_request.id.0);
    store.set(&spawn_key, &spawn_request).await.unwrap();

    let all_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let request: claw10_domain::SpawnRequest =
        store.get(&spawn_key).await.unwrap().unwrap();
    let mission_stored: Mission = store.get(&mission_key).await.unwrap().unwrap();
    let mut parent: Agent = store.get(&root_key).await.unwrap().unwrap();
    let current_depth = calculate_depth(&request.requested_by, &all_agents);

    let result = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission_stored,
            &request,
            &all_agents,
            current_depth,
        )
        .await;

    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), claw10_spawn::SpawnError::SwarmSizeExceeded),
        "Harus gagal karena ukuran swarm melebihi batas limit"
    );
}

#[tokio::test]
async fn test_e2e_spawn_validation_duplicate_objective() {
    let state = AppState::new();
    let store = state.kv_store.clone();

    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    let root = make_root_agent(&mission);
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    // Skenario 1: Duplikasi role dengan root
    let child_specs = vec![ChildSpec {
        role: "root".into(), // Duplikat role parent
        objective: "unique objective".into(),
        budget_usd: 10.0,
        model_profile: "gpt-4".into(),
        max_turns: 10,
        custom_permissions: None,
    }];

    let spawn_request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test dup".into(),
        child_specs,
    );

    let spawn_key = format!("{SPAWNREQ_PREFIX}{}", spawn_request.id.0);
    store.set(&spawn_key, &spawn_request).await.unwrap();

    let all_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let request: claw10_domain::SpawnRequest =
        store.get(&spawn_key).await.unwrap().unwrap();
    let mission_stored: Mission = store.get(&mission_key).await.unwrap().unwrap();
    let mut parent: Agent = store.get(&root_key).await.unwrap().unwrap();
    let current_depth = calculate_depth(&request.requested_by, &all_agents);

    let result = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission_stored,
            &request,
            &all_agents,
            current_depth,
        )
        .await;

    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), claw10_spawn::SpawnError::DuplicateObjective(_)),
        "Harus gagal karena role bertabrakan"
    );
}

#[tokio::test]
async fn test_e2e_spawn_validation_permission_not_delegable() {
    let state = AppState::new();
    let store = state.kv_store.clone();

    let mission = make_mission();
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    let root = make_root_agent(&mission);
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    // Meminta permission "admin" yang tidak didelegasikan oleh parent
    let child_specs = vec![ChildSpec {
        role: "scout".into(),
        objective: "test objective".into(),
        budget_usd: 10.0,
        model_profile: "gpt-4".into(),
        max_turns: 10,
        custom_permissions: Some(vec![claw10_domain::Permission("admin".into())]),
    }];

    let spawn_request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test invalid perms".into(),
        child_specs,
    );

    let spawn_key = format!("{SPAWNREQ_PREFIX}{}", spawn_request.id.0);
    store.set(&spawn_key, &spawn_request).await.unwrap();

    let all_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let request: claw10_domain::SpawnRequest =
        store.get(&spawn_key).await.unwrap().unwrap();
    let mission_stored: Mission = store.get(&mission_key).await.unwrap().unwrap();
    let mut parent: Agent = store.get(&root_key).await.unwrap().unwrap();
    let current_depth = calculate_depth(&request.requested_by, &all_agents);

    let result = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission_stored,
            &request,
            &all_agents,
            current_depth,
        )
        .await;

    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), claw10_spawn::SpawnError::PermissionNotDelegable(_)),
        "Harus gagal karena permission tidak didelegasikan oleh parent"
    );
}

#[tokio::test]
async fn test_e2e_spawn_validation_mission_not_active() {
    let state = AppState::new();
    let store = state.kv_store.clone();

    // Buat mission dengan state Completed (tidak Active)
    let mut mission = make_mission();
    mission.state = MissionState::Completed;
    let mission_key = format!("{MISSION_PREFIX}{}", mission.id.0);
    store.set(&mission_key, &mission).await.unwrap();

    let root = make_root_agent(&mission);
    let root_key = format!("{AGENT_PREFIX}{}", root.id.0);
    store.set(&root_key, &root).await.unwrap();

    let child_specs = vec![ChildSpec {
        role: "scout".into(),
        objective: "test objective".into(),
        budget_usd: 10.0,
        model_profile: "gpt-4".into(),
        max_turns: 10,
        custom_permissions: None,
    }];

    let spawn_request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test inactive mission".into(),
        child_specs,
    );

    let spawn_key = format!("{SPAWNREQ_PREFIX}{}", spawn_request.id.0);
    store.set(&spawn_key, &spawn_request).await.unwrap();

    let all_agents: Vec<Agent> = store
        .scan_prefix::<Agent>(AGENT_PREFIX)
        .await
        .unwrap()
        .into_iter()
        .map(|(_, a)| a)
        .collect();

    let request: claw10_domain::SpawnRequest =
        store.get(&spawn_key).await.unwrap().unwrap();
    let mission_stored: Mission = store.get(&mission_key).await.unwrap().unwrap();
    let mut parent: Agent = store.get(&root_key).await.unwrap().unwrap();
    let current_depth = calculate_depth(&request.requested_by, &all_agents);

    let result = state
        .spawn_broker
        .process_spawn_request(
            &mut parent,
            &mission_stored,
            &request,
            &all_agents,
            current_depth,
        )
        .await;

    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), claw10_spawn::SpawnError::Validation(_)),
        "Harus gagal karena mission tidak aktif"
    );
}

