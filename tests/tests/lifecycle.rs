use chrono::{Duration, Utc};
use uuid::Uuid;

use claw10_domain::{
    Agent, AgentGenome, AgentId, AgentState, AutonomyConfig, Budget, CheckpointReason, IdentityId,
    LifecycleMode, MemoryConfig, ModelPolicy, NetworkPolicy, PolicyBundle,
    PolicyBundleId, RuntimeConfig,
};
use claw10_lifecycle::{LifecycleError, LifecycleService};

fn make_test_agent() -> Agent {
    let now = Utc::now();
    Agent {
        id: AgentId(Uuid::now_v7()),
        identity_id: IdentityId(Uuid::now_v7()),
        mission_id: claw10_domain::MissionId(Uuid::now_v7()),
        parent_agent_id: None,
        lineage_id: claw10_domain::LineageId(Uuid::now_v7()),
        name: "test-agent".into(),
        role: "tester".into(),
        genome: AgentGenome {
            id: "test-genome".into(),
            version: "1.0".into(),
            role: "tester".into(),
            lifecycle_modes: vec![LifecycleMode::Persistent],
            model_policy: ModelPolicy {
                preferred_profile: "gpt-4".into(),
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
                preferred_class: "standard".into(),
                network: NetworkPolicy::AllowByDefault,
            },
            verification_required: false,
        },
        state: AgentState::Active,
        lifecycle_mode: LifecycleMode::Persistent,
        persistent_pattern: None,
        budget: Budget {
            allocated_usd: 100.0,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: None,
            recurring_monthly_usd: None,
        },
        delegable_permissions: vec![],
        non_delegable_permissions: vec![],
        current_runtime: Some(claw10_domain::RuntimeLease {
            worker_id: "worker-1".into(),
            acquired_at: now,
            expires_at: now + Duration::seconds(60),
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
        turn_count: 10,
        total_cost_usd: 5.0,
        created_at: now,
        updated_at: now,
        terminated_at: None,
    }
}

fn make_lease(worker_id: &str) -> claw10_domain::RuntimeLease {
    claw10_domain::RuntimeLease {
        worker_id: worker_id.into(),
        acquired_at: Utc::now(),
        expires_at: Utc::now() + Duration::seconds(120),
        renewal_interval_seconds: 120,
    }
}

#[test]
fn test_create_checkpoint() {
    let agent = make_test_agent();
    let cp = LifecycleService::create_checkpoint(&agent, CheckpointReason::PreHibernation);

    assert_eq!(cp.agent_id, agent.id.0.to_string());
    assert_eq!(cp.reason, CheckpointReason::PreHibernation);
    assert!(cp.state_snapshot.get("turn_count").is_some());
}

#[test]
fn test_restore_checkpoint() {
    let mut agent = make_test_agent();
    let cp = LifecycleService::create_checkpoint(&agent, CheckpointReason::PreHibernation);
    agent.turn_count = 99;

    LifecycleService::restore_checkpoint(&mut agent, &cp).unwrap();
    assert_eq!(agent.turn_count, 10); // restored from snapshot
}

#[test]
fn test_hibernate_and_wake() {
    let mut agent = make_test_agent();
    assert_eq!(agent.state, AgentState::Active);
    assert!(agent.current_runtime.is_some());

    let cp = LifecycleService::hibernate(&mut agent).unwrap();
    assert_eq!(agent.state, AgentState::Hibernating);
    assert!(agent.current_runtime.is_none());
    assert_eq!(cp.reason, CheckpointReason::PreHibernation);
    assert_eq!(agent.checkpoints.len(), 1);

    let lease = make_lease("worker-2");
    LifecycleService::wake(&mut agent, lease).unwrap();
    assert_eq!(agent.state, AgentState::Active);
    assert!(agent.current_runtime.is_some());
}

#[test]
fn test_hibernate_twice_fails() {
    let mut agent = make_test_agent();
    LifecycleService::hibernate(&mut agent).unwrap();
    let result = LifecycleService::hibernate(&mut agent);
    assert!(matches!(result, Err(LifecycleError::AlreadyHibernating)));
}

#[test]
fn test_wake_non_hibernating_fails() {
    let mut agent = make_test_agent();
    let lease = make_lease("worker-2");
    let result = LifecycleService::wake(&mut agent, lease);
    assert!(matches!(result, Err(LifecycleError::NotHibernating)));
}

#[test]
fn test_heartbeat_renews_lease() {
    let mut agent = make_test_agent();
    let original_expiry = agent.current_runtime.as_ref().unwrap().expires_at;

    // Sleep a tiny bit so renewed expiry is different
    std::thread::sleep(std::time::Duration::from_millis(10));

    let remaining = LifecycleService::heartbeat(&mut agent).unwrap();
    let new_expiry = agent.current_runtime.as_ref().unwrap().expires_at;

    assert!(new_expiry > original_expiry);
    assert!(remaining.num_seconds() > 0);
}

#[test]
fn test_heartbeat_expired_lease_fails() {
    let mut agent = make_test_agent();
    agent.current_runtime = Some(claw10_domain::RuntimeLease {
        worker_id: "worker-1".into(),
        acquired_at: Utc::now() - Duration::hours(2),
        expires_at: Utc::now() - Duration::seconds(10),
        renewal_interval_seconds: 60,
    });

    let result = LifecycleService::heartbeat(&mut agent);
    assert!(matches!(result, Err(LifecycleError::LeaseExpired)));
}

#[test]
fn test_detect_stale() {
    let fresh = make_test_agent();
    let mut stale = make_test_agent();
    stale.current_runtime = Some(claw10_domain::RuntimeLease {
        worker_id: "stale-worker".into(),
        acquired_at: Utc::now() - Duration::hours(2),
        expires_at: Utc::now() - Duration::seconds(30),
        renewal_interval_seconds: 60,
    });

    let agents = vec![fresh, stale];
    let stale_agents = LifecycleService::detect_stale(&agents, 5);
    assert_eq!(stale_agents.len(), 1);
}

#[test]
fn test_migrate() {
    let mut agent = make_test_agent();
    agent.lifecycle_mode = LifecycleMode::Persistent;

    let cp = LifecycleService::migrate(&mut agent, "worker-3", 300).unwrap();
    assert_eq!(agent.state, AgentState::Active);
    assert_eq!(
        agent.current_runtime.as_ref().unwrap().worker_id,
        "worker-3"
    );
    assert_eq!(cp.reason, CheckpointReason::PreMigration);
}

#[test]
fn test_migrate_non_persistent_fails() {
    let mut agent = make_test_agent();
    agent.lifecycle_mode = LifecycleMode::Ephemeral;

    let result = LifecycleService::migrate(&mut agent, "worker-3", 300);
    assert!(matches!(result, Err(LifecycleError::NotPersistent)));
}

#[test]
fn test_assign_lease() {
    let mut agent = make_test_agent();
    agent.current_runtime = None;

    LifecycleService::assign_lease(&mut agent, "worker-4", 120);
    assert!(agent.current_runtime.is_some());
    assert_eq!(agent.current_runtime.unwrap().worker_id, "worker-4");
}

// ── New tests for B4 improvements ──────────────────────────────

#[test]
fn test_detect_stale_active_no_lease() {
    // Active agent with no runtime lease should be detected as stale
    let mut agent = make_test_agent();
    agent.current_runtime = None;

    let agents = vec![agent];
    let stale = LifecycleService::detect_stale(&agents, 60);
    assert_eq!(stale.len(), 1, "Active agent with no lease should be stale");
}

#[test]
fn test_detect_stale_ignores_hibernating() {
    // Hibernating agent should never be detected as stale
    let mut agent = make_test_agent();
    LifecycleService::hibernate(&mut agent).unwrap();
    assert_eq!(agent.state, AgentState::Hibernating);

    let agents = vec![agent];
    let stale = LifecycleService::detect_stale(&agents, 5);
    assert_eq!(stale.len(), 0, "Hibernating agent should not be stale");
}

#[test]
fn test_wake_corrupted_checkpoint_fallback() {
    // If the latest checkpoint is corrupted, wake should try earlier ones
    let mut agent = make_test_agent();

    // Create a valid checkpoint
    let cp1 = LifecycleService::create_checkpoint(&agent, CheckpointReason::Periodic);
    agent.checkpoints.push(cp1);
    agent.turn_count = 42;

    // Then hibernate (creates checkpoint with turn_count=42)
    LifecycleService::hibernate(&mut agent).unwrap();
    assert_eq!(agent.checkpoints.len(), 2);

    // Corrupt the latest checkpoint (the pre-hibernation one)
    if let Some(latest) = agent.checkpoints.last_mut() {
        latest.state_snapshot = serde_json::json!({
            "state": "InvalidState",
        });
    }

    // Wake should fallback to the earlier (periodic) checkpoint
    let lease = make_lease("worker-fallback");
    LifecycleService::wake(&mut agent, lease).unwrap();
    assert_eq!(
        agent.state,
        AgentState::Active,
        "Should still wake despite corrupted latest checkpoint"
    );
}

#[test]
fn test_wake_all_checkpoints_corrupted_fails() {
    let mut agent = make_test_agent();
    LifecycleService::hibernate(&mut agent).unwrap();

    // Corrupt all checkpoints
    for cp in agent.checkpoints.iter_mut() {
        cp.state_snapshot = serde_json::json!({"state": "Bogus"});
    }

    let lease = make_lease("worker-fail");
    let result = LifecycleService::wake(&mut agent, lease);
    assert!(
        result.is_err(),
        "Wake should fail when all checkpoints are corrupted"
    );
    assert!(result.unwrap_err().to_string().contains("corrupted"));
}

#[test]
fn test_validate_agent_state_active_no_lease() {
    let mut agent = make_test_agent();
    agent.current_runtime = None;
    agent.state = AgentState::Active;

    let issues = LifecycleService::validate_agent_state(&mut agent);
    assert_eq!(issues.len(), 1, "Should detect Active + no lease");
    assert_eq!(agent.state, AgentState::Hibernating, "Should fix to Hibernating");
}

#[test]
fn test_validate_agent_state_hibernating_with_lease() {
    let mut agent = make_test_agent();
    // Hibernate first
    LifecycleService::hibernate(&mut agent).unwrap();
    // Force a lease back (inconsistent)
    agent.current_runtime = Some(claw10_domain::RuntimeLease {
        worker_id: "stray-lease".into(),
        acquired_at: Utc::now(),
        expires_at: Utc::now() + Duration::seconds(60),
        renewal_interval_seconds: 60,
    });

    let issues = LifecycleService::validate_agent_state(&mut agent);
    assert_eq!(issues.len(), 1, "Should detect Hibernating + lease");
    assert!(agent.current_runtime.is_none(), "Should clear the stray lease");
}

#[test]
fn test_validate_agent_state_terminated_with_checkpoints() {
    let mut agent = make_test_agent();
    LifecycleService::hibernate(&mut agent).unwrap();
    LifecycleService::terminate(&mut agent);

    assert_eq!(agent.state, AgentState::Terminated);
    assert!(!agent.checkpoints.is_empty(), "Terminated should have checkpoints");

    let issues = LifecycleService::validate_agent_state(&mut agent);
    assert_eq!(issues.len(), 1, "Should detect terminated + checkpoints");
    assert!(agent.checkpoints.is_empty(), "Should clear checkpoints");
}

#[test]
fn test_validate_agent_state_consistent_no_issues() {
    let mut agent = make_test_agent();
    // Should be consistent: Active + lease
    let issues = LifecycleService::validate_agent_state(&mut agent);
    assert!(issues.is_empty(), "Consistent state should have no issues");
}

#[test]
fn test_create_periodic_checkpoint() {
    let mut agent = make_test_agent();

    // Fill up with 9 dummy checkpoints
    for _ in 0..9 {
        let cp = LifecycleService::create_checkpoint(&agent, CheckpointReason::Periodic);
        agent.checkpoints.push(cp);
    }
    assert_eq!(agent.checkpoints.len(), 9);

    // Create periodic checkpoint (should auto-GC to max 10, adding 1 more means 1 removed)
    let cp = LifecycleService::create_periodic_checkpoint(&mut agent);
    assert_eq!(cp.reason, CheckpointReason::Periodic);

    // With 9 existing + 1 new = 10, max is 10, so 0 removed
    assert_eq!(agent.checkpoints.len(), 10, "Should have max 10 checkpoints after periodic");

    // Add one more — should GC to 10
    let _cp2 = LifecycleService::create_periodic_checkpoint(&mut agent);
    assert_eq!(agent.checkpoints.len(), 10, "Should still be exactly 10 after GC");
}

#[test]
fn test_gc_checkpoints_by_age() {
    let mut agent = make_test_agent();

    // Add some old checkpoints
    let old_cp = LifecycleService::create_checkpoint(&agent, CheckpointReason::Periodic);
    agent.checkpoints.push(old_cp);

    // Sleep 10ms so the next one is newer
    std::thread::sleep(std::time::Duration::from_millis(10));
    let cp = LifecycleService::create_checkpoint(&agent, CheckpointReason::Periodic);
    agent.checkpoints.push(cp);

    assert_eq!(agent.checkpoints.len(), 2);

    // GC with a very short max_age — should remove the old one
    let removed = LifecycleService::gc_checkpoints_by_age(&mut agent, chrono::Duration::milliseconds(5));
    assert_eq!(removed, 1, "Should remove 1 old checkpoint");
    assert_eq!(agent.checkpoints.len(), 1, "Should keep 1 recent checkpoint");
}
