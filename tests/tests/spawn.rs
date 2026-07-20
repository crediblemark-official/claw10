use std::sync::Arc;

use claw10_agent::AgentStore;
use claw10_auth::credential::CredentialService;
use claw10_auth::identity::IdentityService;
use claw10_domain::{
    Agent, AgentGenome, AgentId, AgentState, AutonomyConfig, Budget, ChildSpec, Credential,
    CredentialKind, IdentityId, LifecycleMode, Lineage, MemoryConfig, Mission, MissionId,
    MissionState, ModelPolicy, NetworkPolicy, Permission, PolicyBundle,
    PolicyBundleId, RiskLevel, RuntimeConfig, SwarmLimitsConfig,
};
use claw10_event::InMemoryEventBus;
use claw10_lineage::LineageService;
use claw10_spawn::broker::SpawnBroker;
use claw10_spawn::descendant::DescendantManager;
use claw10_store::InMemoryStore;

fn make_broker() -> SpawnBroker {
    let store = Arc::new(InMemoryStore::new());
    let agent_store = Arc::new(AgentStore::new(store));
    let event_bus = Arc::new(InMemoryEventBus::new());
    SpawnBroker::new(make_limits(), agent_store, event_bus)
}

fn make_test_mission() -> Mission {
    Mission {
        id: MissionId(uuid::Uuid::now_v7()),
        owner_id: IdentityId(uuid::Uuid::now_v7()),
        objective: "test mission".into(),
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

fn make_root_agent(mission: &Mission) -> Agent {
    let now = chrono::Utc::now();
    let identity = IdentityService::create_agent_identity(&AgentId(uuid::Uuid::now_v7()));

    Agent {
        id: AgentId(uuid::Uuid::now_v7()),
        identity_id: identity.id,
        mission_id: mission.id.clone(),
        parent_agent_id: None,
        lineage_id: claw10_domain::LineageId(uuid::Uuid::now_v7()),
        name: "root-agent".into(),
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
                Permission("execute".into()),
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
            Permission("execute".into()),
        ],
        non_delegable_permissions: vec![],
        current_runtime: None,
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

fn make_lineage(mission: &Mission, root: &Agent) -> Lineage {
    LineageService::create_lineage(mission.id.clone(), root.id.clone())
}

fn make_child_spec(role: &str, budget: f64) -> ChildSpec {
    ChildSpec {
        role: role.into(),
        objective: format!("perform {role} tasks"),
        budget_usd: budget,
        model_profile: "gpt-4".into(),
        max_turns: 50,
        custom_permissions: None,
    }
}

fn make_credential(agent: &Agent) -> Credential {
    CredentialService::issue_credential(
        agent.identity_id.clone(),
        CredentialKind::Token,
        "read write execute".into(),
        3600,
    )
}

fn make_limits() -> SwarmLimitsConfig {
    SwarmLimitsConfig {
        max_spawn_depth: 5,
        max_children_per_agent: 10,
        max_agents_per_mission: 100,
        max_concurrent_agents: 50,
        max_persistent_children_per_agent: 5,
        max_turns_per_ephemeral_agent: 100,
        max_idle_seconds_ephemeral: 600,
    }
}

#[tokio::test]
async fn test_full_spawn_flow() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    let broker = make_broker();
    let root_clone = root.clone();

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "need helpers".into(),
        vec![
            make_child_spec("scout", 50.0),
            make_child_spec("worker", 100.0),
        ],
    );

    let children = broker
        .process_spawn_request(&mut root, &mission, &request, &[root_clone], 0)
        .await
        .expect("spawn should succeed");

    assert_eq!(children.len(), 2);
    assert_eq!(children[0].role, "scout");
    assert_eq!(children[1].role, "worker");

    assert_eq!(root.state, AgentState::Active);
    assert_eq!(children[0].state, AgentState::Ready);
    assert_eq!(children[1].state, AgentState::Ready);

    assert_eq!(children[0].parent_agent_id, Some(root.id.clone()));
    assert_eq!(children[1].parent_agent_id, Some(root.id.clone()));

    assert_eq!(root.budget.spent_usd, 150.0);

    assert!(!children[0].delegable_permissions.is_empty());
    assert!(!children[0].genome.autonomy.can_spawn);
    assert_eq!(children[0].genome.autonomy.max_spawn_depth, 0);
}

#[tokio::test]
async fn test_spawn_validation_fails_when_parent_not_active() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    root.state = AgentState::Terminated;
    let broker = make_broker();
    let root_clone = root.clone();

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test".into(),
        vec![make_child_spec("child", 10.0)],
    );

    let result = broker
        .process_spawn_request(&mut root, &mission, &request, &[root_clone], 0)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_spawn_validation_fails_when_parent_cannot_spawn() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    root.genome.autonomy.can_spawn = false;
    let broker = make_broker();
    let root_clone = root.clone();

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test".into(),
        vec![make_child_spec("child", 10.0)],
    );

    let result = broker
        .process_spawn_request(&mut root, &mission, &request, &[root_clone], 0)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_spawn_validation_fails_when_depth_exceeded() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    let broker = make_broker();
    let root_clone = root.clone();

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test".into(),
        vec![make_child_spec("child", 10.0)],
    );

    let result = broker
        .process_spawn_request(&mut root, &mission, &request, &[root_clone], 5)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_spawn_validation_fails_when_budget_insufficient() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    root.budget.spent_usd = 480.0;
    let broker = make_broker();
    let root_clone = root.clone();

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test".into(),
        vec![make_child_spec("expensive-child", 50.0)],
    );

    let result = broker
        .process_spawn_request(&mut root, &mission, &request, &[root_clone], 0)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_descendant_teardown() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    let mut lineage = make_lineage(&mission, &root);
    let broker = make_broker();
    let root_clone = root.clone();

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "scale".into(),
        vec![
            make_child_spec("child-a", 30.0),
            make_child_spec("child-b", 30.0),
        ],
    );

    let children = broker
        .process_spawn_request(&mut root, &mission, &request, &[root_clone], 0)
        .await
        .expect("spawn should succeed");

    for child in &children {
        LineageService::add_entry(
            &mut lineage,
            child.id.clone(),
            Some(root.id.clone()),
            child.role.clone(),
        );
    }

    let all_agents = {
        let mut all = vec![root.clone()];
        all.extend(children.clone());
        all
    };

    let mut credentials: Vec<Credential> = all_agents.iter().map(|a| make_credential(a)).collect();

    let tasks = vec![];

    let summary = DescendantManager::full_teardown(
        &root,
        &all_agents,
        &tasks,
        &mut credentials,
        &mut lineage,
    )
    .expect("teardown should succeed");

    assert_eq!(summary["descendants_frozen"], 2);
    assert_eq!(summary["credentials_revoked"], 2);
    assert_eq!(summary["lineage_entries_cleaned"], 2);

    let revoked_count = credentials
        .iter()
        .filter(|c| c.revoked_at.is_some())
        .count();
    assert_eq!(revoked_count, 2);

    let terminated_count = lineage
        .entries
        .iter()
        .filter(|e| e.state == "terminated")
        .count();
    assert_eq!(terminated_count, 2);
}

#[tokio::test]
async fn test_spawn_validation_fails_when_swarm_size_exceeded() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    let broker = make_broker();

    // Buat dummy agents sebanyak batas limit (100)
    let mut all_agents = Vec::new();
    for _ in 0..100 {
        all_agents.push(root.clone());
    }

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test limit".into(),
        vec![make_child_spec("scout", 50.0)],
    );

    let result = broker
        .process_spawn_request(&mut root, &mission, &request, &all_agents, 0)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, claw10_spawn::SpawnError::SwarmSizeExceeded),
        "Harus gagal karena ukuran swarm melebihi batas limit"
    );
}

#[tokio::test]
async fn test_spawn_validation_fails_when_duplicate_objective() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    let broker = make_broker();
    let root_clone = root.clone();

    // Skenario 1: Objective child exact-match dengan nama agen yang sudah ada -> harus gagal
    let mut spec_exact = make_child_spec("scout", 50.0);
    spec_exact.objective = "root-agent".into(); // exact match with root agent's name
    let request_dup = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test exact dup".into(),
        vec![spec_exact],
    );

    let result_dup = broker
        .process_spawn_request(&mut root, &mission, &request_dup, &[root_clone.clone()], 0)
        .await;

    assert!(result_dup.is_err());
    assert!(
        matches!(result_dup.unwrap_err(), claw10_spawn::SpawnError::DuplicateObjective(_)),
        "Harus gagal karena objective sama persis dengan nama agen yang sudah ada"
    );

    // Skenario 2: Role sama tetapi objective berbeda -> HARUS sukses (tidak dicek berdasarkan role)
    let mut spec_diff_role = make_child_spec("root", 50.0); // role = "root"
    spec_diff_role.objective = "different objective".into();
    let request_diff = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test diff role".into(),
        vec![spec_diff_role],
    );

    let result_diff = broker
        .process_spawn_request(&mut root, &mission, &request_diff, &[root_clone.clone()], 0)
        .await;

    assert!(result_diff.is_ok(),
        "Role yang sama seharusnya tidak dicek lagi. Yang dicek hanya exact-match objective"
    );

    // Skenario 3: Objective substring dari nama agen -> HARUS sukses (tidak dicek substring)
    let mut spec_substr = make_child_spec("scout", 50.0);
    spec_substr.objective = "root".into(); // "root-agent" contains "root" as substring
    let request_substr = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test substr".into(),
        vec![spec_substr],
    );

    let result_substr = broker
        .process_spawn_request(&mut root, &mission, &request_substr, &[root_clone], 0)
        .await;

    assert!(result_substr.is_ok(),
        "Substring match seharusnya tidak dicek lagi. Yang dicek hanya exact-match objective"
    );
}

#[tokio::test]
async fn test_spawn_validation_fails_when_permission_not_delegable() {
    let mission = make_test_mission();
    let mut root = make_root_agent(&mission);
    let broker = make_broker();
    let root_clone = root.clone();

    // Buat child spec yang meminta custom permission yang tidak dimiliki parent (misal "admin")
    let mut spec = make_child_spec("scout", 50.0);
    spec.custom_permissions = Some(vec![Permission("admin".into())]);

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test permission".into(),
        vec![spec],
    );

    let result = broker
        .process_spawn_request(&mut root, &mission, &request, &[root_clone], 0)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, claw10_spawn::SpawnError::PermissionNotDelegable(_)),
        "Harus gagal karena permission tidak didelegasikan oleh parent"
    );
}

#[tokio::test]
async fn test_spawn_validation_fails_when_mission_not_active() {
    let mut mission = make_test_mission();
    mission.state = MissionState::Completed; // Ubah menjadi tidak Active
    let mut root = make_root_agent(&mission);
    let broker = make_broker();
    let root_clone = root.clone();

    let request = SpawnBroker::create_request(
        mission.id.clone(),
        root.id.clone(),
        "test inactive mission".into(),
        vec![make_child_spec("scout", 50.0)],
    );

    let result = broker
        .process_spawn_request(&mut root, &mission, &request, &[root_clone], 0)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, claw10_spawn::SpawnError::Validation(_)),
        "Harus gagal karena mission tidak aktif"
    );
}

