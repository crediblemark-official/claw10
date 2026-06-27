use super::*;
use clawhive_domain::{
    AgentId, Budget, IdentityId, LifecycleMode, Memory,
    MemoryId, MemorySource, MemoryStatus, MemoryType, Mission, MissionId,
    MissionState, OrganizationId, PolicyBundle, PolicyBundleId,
    PolicyEffect, PolicyRule, PolicyRuleId, PolicySubject, RetryPolicy, RiskLevel,
    Skill, SkillCostProfile, SkillId, SkillState, Task, TaskId, TaskState,
};
use chrono::Utc;

fn dummy_task() -> Task {
    Task {
        id: TaskId(uuid::Uuid::now_v7()),
        mission_id: MissionId(uuid::Uuid::now_v7()),
        parent_task_id: None,
        owner_id: AgentId(uuid::Uuid::now_v7()),
        objective: "test task".into(),
        dependencies: vec![],
        risk: RiskLevel("medium".into()),
        budget: Budget::default(),
        deadline: None,
        input: serde_json::Value::Null,
        output_contract: serde_json::Value::Null,
        evidence_contract: vec![],
        retry_policy: RetryPolicy {
            max_retries: 0,
            backoff_seconds: 0,
        },
        idempotency_key: None,
        lifecycle_mode: "persistent".into(),
        state: TaskState::Created,
        evidence: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn dummy_mission() -> Mission {
    Mission {
        id: MissionId(uuid::Uuid::now_v7()),
        organization_id: OrganizationId(uuid::Uuid::now_v7()),
        owner_id: IdentityId(uuid::Uuid::now_v7()),
        objective: "an objective".into(),
        scope: None,
        lifecycle_mode: LifecycleMode::Persistent,
        campaign_end: None,
        review_interval_days: None,
        budget: Budget::default(),
        risk: RiskLevel("medium".into()),
        require_evidence: false,
        minimum_verifiers: 1,
        state: MissionState::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn test_build_context_with_task_and_mission() {
    let pipeline = ContextPipeline::new(PipelineConfig::default());
    let task = dummy_task();
    let mission = dummy_mission();
    let sources = ContextSources {
        task: Some(&task),
        mission: Some(&mission),
        ..ContextSources::default()
    };
    let context = pipeline.build_context(sources).await.unwrap();
    assert!(!context.is_empty());
    assert!(context.contains("test task"));
    assert!(context.contains("an objective"));
}

#[tokio::test]
async fn test_build_context_empty_sources() {
    let pipeline = ContextPipeline::new(PipelineConfig::default());
    let sources = ContextSources::default();
    let context = pipeline.build_context(sources).await.unwrap();
    assert!(context.contains("[TOON v1]"));
}

#[tokio::test]
async fn test_token_budget_trimming() {
    let config = PipelineConfig {
        max_token_budget: 1,
        ..Default::default()
    };
    let pipeline = ContextPipeline::new(config);
    let history = vec!["very long message to trigger trimming".into()];
    let sources = ContextSources {
        history: &history,
        ..ContextSources::default()
    };
    let result = pipeline.build_context(sources).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_selective_sources() {
    let config = PipelineConfig {
        include_mission: false,
        include_memories: false,
        include_policy: false,
        include_skills: false,
        include_history: false,
        include_tools: false,
        include_agent: false,
        include_lineage: false,
        include_workers: false,
        include_evidence: false,
        ..Default::default()
    };
    let pipeline = ContextPipeline::new(config);
    let task = dummy_task();
    let sources = ContextSources {
        task: Some(&task),
        ..ContextSources::default()
    };
    let context = pipeline.build_context(sources).await.unwrap();
    assert!(context.contains("test task"));
}

#[tokio::test]
async fn test_context_with_memories() {
    let pipeline = ContextPipeline::new(PipelineConfig::default());
    let memory = Memory {
        id: MemoryId(uuid::Uuid::now_v7()),
        tenant_id: "t1".into(),
        scope: "test".into(),
        memory_type: MemoryType::Semantic,
        content: "key insight".into(),
        source: MemorySource {
            agent_id: AgentId(uuid::Uuid::nil()),
            task_id: TaskId(uuid::Uuid::nil()),
            evidence_id: None,
        },
        confidence: 0.9,
        classification: "public".into(),
        status: MemoryStatus::Active,
        verified_by: vec![],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let sources = ContextSources {
        memories: &[memory],
        ..ContextSources::default()
    };
    let context = pipeline.build_context(sources).await.unwrap();
    assert!(context.contains("[memory]"));
    assert!(context.contains("key insight"));
}

#[tokio::test]
async fn test_context_with_policies() {
    let pipeline = ContextPipeline::new(PipelineConfig::default());
    let policy = PolicyBundle {
        id: PolicyBundleId(uuid::Uuid::now_v7()),
        name: "main".into(),
        version: "1.0".into(),
        is_active: true,
        rules: vec![PolicyRule {
            id: PolicyRuleId(uuid::Uuid::now_v7()),
            subject: PolicySubject::Role("admin".into()),
            effect: PolicyEffect::Allow,
            action: "execute:*".into(),
            resource: "tool:http".into(),
            condition: None,
            priority: 100,
        }],
        signed_by: None,
        signature: None,
        activated_at: Some(Utc::now()),
        created_at: Utc::now(),
    };
    let sources = ContextSources {
        policies: &[policy],
        ..ContextSources::default()
    };
    let context = pipeline.build_context(sources).await.unwrap();
    assert!(context.contains("[policy]"));
}

#[tokio::test]
async fn test_context_with_skills() {
    let pipeline = ContextPipeline::new(PipelineConfig::default());
    let skill = Skill {
        id: SkillId(uuid::Uuid::now_v7()),
        name: "search".into(),
        purpose: "search web".into(),
        version: "1.0".into(),
        input_schema: serde_json::Value::Null,
        output_schema: serde_json::Value::Null,
        steps: vec![],
        required_tools: vec![],
        required_permissions: vec![],
        state: SkillState::Active,
        signature: None,
        cost_profile: SkillCostProfile {
            estimated_cost_usd: 0.01,
            average_duration_seconds: 1.0,
        },
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let sources = ContextSources {
        skills: &[skill],
        ..ContextSources::default()
    };
    let context = pipeline.build_context(sources).await.unwrap();
    assert!(context.contains("[skills]"));
}

#[test]
fn test_estimate_tokens() {
    assert_eq!(estimate_tokens("a"), 1);
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("abcdefgh"), 2);
}
