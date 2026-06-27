use super::*;
use clawhive_domain::agent::AgentId;
use clawhive_domain::memory::{MemoryId, MemorySource, MemoryType};
use clawhive_domain::mission::MissionId;
use clawhive_domain::model::RiskLevel;
use clawhive_domain::task::{RetryPolicy, TaskId};

fn make_test_task() -> Task {
    Task {
        id: TaskId(uuid::Uuid::nil()),
        mission_id: MissionId(uuid::Uuid::nil()),
        parent_task_id: None,
        owner_id: AgentId(uuid::Uuid::nil()),
        objective: "Test payment flow".to_string(),
        dependencies: Vec::new(),
        risk: RiskLevel("medium".to_string()),
        budget: clawhive_domain::budget::Budget {
            allocated_usd: 0.0,
            spent_usd: 0.0,
            soft_limit_usd: None,
            hard_limit_usd: None,
            recurring_monthly_usd: None,
        },
        deadline: None,
        input: serde_json::Value::Null,
        output_contract: serde_json::Value::Null,
        evidence_contract: Vec::new(),
        retry_policy: RetryPolicy {
            max_retries: 0,
            backoff_seconds: 0,
        },
        idempotency_key: None,
        lifecycle_mode: "ephemeral".to_string(),
        state: clawhive_domain::task::TaskState::Created,
        evidence: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[test]
fn test_encode_task() {
    let task = make_test_task();
    let encoded = ToonEncoder::encode_task(&task);
    assert!(encoded.contains("Test payment flow"));
}

#[test]
fn test_encode_memories_empty() {
    let encoded = ToonEncoder::encode_memories(&[]);
    assert_eq!(encoded, "");
}

#[test]
fn test_encode_memories() {
    let memories = vec![Memory {
        id: MemoryId(uuid::Uuid::nil()),
        tenant_id: "test".to_string(),
        scope: "test".to_string(),
        memory_type: MemoryType::Semantic,
        content: "Use transactions".to_string(),
        source: MemorySource {
            agent_id: AgentId(uuid::Uuid::nil()),
            task_id: TaskId(uuid::Uuid::nil()),
            evidence_id: None,
        },
        confidence: 0.95,
        classification: "public".to_string(),
        status: clawhive_domain::memory::MemoryStatus::Active,
        verified_by: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }];
    let encoded = ToonEncoder::encode_memories(&memories);
    assert!(encoded.contains("Use transactions"));
    assert!(encoded.contains("confidence: 0.95"));
}

#[test]
fn test_build_context_empty() {
    let ctx = ToonEncoder::build_context(None, None, &[], &[], &[], None, &[]);
    assert!(ctx.starts_with("[TOON v1]"));
}

#[test]
fn test_build_context_with_data() {
    let task = make_test_task();
    let ctx = ToonEncoder::build_context(
        Some(&task),
        None,
        &[],
        &[],
        &[],
        None,
        &[],
    );
    assert!(ctx.contains("[task]"));
}

#[test]
fn test_encode_skills() {
    let skills = vec![clawhive_domain::skill::Skill {
        id: clawhive_domain::skill::SkillId(uuid::Uuid::nil()),
        name: "web-search".into(),
        purpose: "search".into(),
        version: "1.0".into(),
        input_schema: serde_json::Value::Null,
        output_schema: serde_json::Value::Null,
        steps: vec![],
        required_tools: vec![],
        required_permissions: vec![],
        state: clawhive_domain::skill::SkillState::Active,
        signature: None,
        cost_profile: clawhive_domain::skill::SkillCostProfile {
            estimated_cost_usd: 0.01,
            average_duration_seconds: 1.0,
        },
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }];
    let encoded = ToonEncoder::encode_skills(&skills);
    assert!(encoded.contains("web-search"));
    assert!(encoded.contains("0.01"));
}

#[test]
fn test_encode_history() {
    let history = vec!["user: hello".into(), "assistant: hi".into()];
    let encoded = ToonEncoder::encode_history(&history);
    assert!(encoded.contains("hello"));
    assert!(encoded.contains("hi"));
}

#[test]
fn test_encode_tools() {
    let tools = vec!["http".into(), "shell".into()];
    let encoded = ToonEncoder::encode_tools(&tools);
    assert!(encoded.contains("http"));
    assert!(encoded.contains("shell"));
}
