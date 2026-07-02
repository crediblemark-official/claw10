use super::*;

fn make_svc() -> SkillService {
    let store = Arc::new(claw10_store::InMemoryStore::new()) as Arc<dyn Store>;
    SkillService::new(store)
}

async fn sample_skill(svc: &SkillService) -> Skill {
    svc.create_skill(
        "web-search".into(),
        "Search the web".into(),
        "1.0.0".into(),
        serde_json::json!({"query": "string"}),
        serde_json::json!({"results": "array"}),
        vec!["search".into()],
        vec!["http".into()],
        vec![],
        SkillCostProfile {
            estimated_cost_usd: 0.01,
            average_duration_seconds: 2.0,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn test_create_skill() {
    let svc = make_svc();
    let skill = sample_skill(&svc).await;
    assert_eq!(skill.name, "web-search");
    assert_eq!(skill.state, SkillState::Candidate);
}

#[tokio::test]
async fn test_full_lifecycle() {
    let svc = make_svc();
    let skill = sample_skill(&svc).await;

    let s = svc.transition_state(&skill.id, SkillState::Scanning).await.unwrap();
    assert_eq!(s.state, SkillState::Scanning);

    let s = svc.transition_state(&skill.id, SkillState::Testing).await.unwrap();
    assert_eq!(s.state, SkillState::Testing);

    let s = svc.transition_state(&skill.id, SkillState::Review).await.unwrap();
    assert_eq!(s.state, SkillState::Review);

    let s = svc.transition_state(&skill.id, SkillState::Approved).await.unwrap();
    assert_eq!(s.state, SkillState::Approved);

    let s = svc.transition_state(&skill.id, SkillState::Staged).await.unwrap();
    assert_eq!(s.state, SkillState::Staged);

    // Must sign before Active
    svc.sign_skill(&skill.id, "sig-abc123".into()).await.unwrap();
    let s = svc.transition_state(&skill.id, SkillState::Active).await.unwrap();
    assert_eq!(s.state, SkillState::Active);

    let s = svc.transition_state(&skill.id, SkillState::Deprecated).await.unwrap();
    assert_eq!(s.state, SkillState::Deprecated);

    let s = svc.transition_state(&skill.id, SkillState::Retired).await.unwrap();
    assert_eq!(s.state, SkillState::Retired);
}

#[tokio::test]
async fn test_invalid_transition() {
    let svc = make_svc();
    let skill = sample_skill(&svc).await;

    // Candidate -> Active (skip all intermediate)
    let result = svc.transition_state(&skill.id, SkillState::Active).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_unsigned_cannot_activate() {
    let svc = make_svc();
    let skill = sample_skill(&svc).await;

    svc.transition_state(&skill.id, SkillState::Scanning).await.unwrap();
    svc.transition_state(&skill.id, SkillState::Testing).await.unwrap();
    svc.transition_state(&skill.id, SkillState::Review).await.unwrap();
    svc.transition_state(&skill.id, SkillState::Approved).await.unwrap();
    svc.transition_state(&skill.id, SkillState::Staged).await.unwrap();

    // No signature -> cannot activate
    let result = svc.transition_state(&skill.id, SkillState::Active).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SkillError::Unsigned));
}

#[tokio::test]
async fn test_list_skills_with_filter() {
    let svc = make_svc();
    let s1 = sample_skill(&svc).await;
    let _s2 = sample_skill(&svc).await;

    svc.transition_state(&s1.id, SkillState::Scanning).await.unwrap();
    // _s2 stays Candidate

    let candidates = svc.list_skills(Some(SkillState::Candidate)).await.unwrap();
    assert_eq!(candidates.len(), 1);

    let all = svc.list_skills(None).await.unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn test_list_by_required_tool() {
    let svc = make_svc();
    let _ = sample_skill(&svc).await; // requires "http"

    let with_http = svc.list_by_required_tool("http").await.unwrap();
    assert_eq!(with_http.len(), 1);

    let with_db = svc.list_by_required_tool("database").await.unwrap();
    assert_eq!(with_db.len(), 0);
}

#[tokio::test]
async fn test_count_by_state() {
    let svc = make_svc();
    let s1 = sample_skill(&svc).await;
    let _s2 = sample_skill(&svc).await;
    svc.transition_state(&s1.id, SkillState::Scanning).await.unwrap();

    let counts = svc.count_by_state().await.unwrap();
    assert_eq!(*counts.get("Candidate").unwrap_or(&0), 1);
    assert_eq!(*counts.get("Scanning").unwrap_or(&0), 1);
}

#[tokio::test]
async fn test_create_version() {
    let svc = make_svc();
    let skill = sample_skill(&svc).await;

    let ver = svc
        .create_version(&skill.id, "1.0.0".into(), None, "initial release".into())
        .await
        .unwrap();
    assert_eq!(ver.version, "1.0.0");
    assert!(ver.previous_version.is_none());
}

#[tokio::test]
async fn test_sign_skill() {
    let svc = make_svc();
    let skill = sample_skill(&svc).await;
    let signed = svc.sign_skill(&skill.id, "test-signature".into()).await.unwrap();
    assert_eq!(signed.signature.as_deref(), Some("test-signature"));
}
