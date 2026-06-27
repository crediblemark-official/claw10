use super::*;

fn make_event(id: &str) -> AuditEvent {
    AuditEvent {
        id: AuditEventId(uuid::Uuid::parse_str(id).unwrap()),
        tenant_id: "tenant-1".into(),
        mission_id: Some("mission-1".into()),
        task_id: Some("task-1".into()),
        agent_id: Some("agent-1".into()),
        parent_agent_id: None,
        lineage_id: Some("lineage-1".into()),
        worker_id: None,
        trace_id: Some("trace-1".into()),
        event_type: "task.completed".into(),
        lifecycle_mode: Some("persistent".into()),
        risk_level: Some("low".into()),
        status: "success".into(),
        cost_usd: 0.0,
        payload: serde_json::json!({"detail": "ok"}),
        timestamp: Utc::now(),
    }
}

fn make_svc() -> AuditService {
    let store = Arc::new(clawhive_store::InMemoryStore::new()) as Arc<dyn Store>;
    AuditService::new(store)
}

#[tokio::test]
async fn test_emit_and_get() {
    let svc = make_svc();
    let event = make_event("00000000-0000-0000-0000-000000000001");
    let emitted = svc.emit_event(event).await.unwrap();
    assert_eq!(emitted.event_type, "task.completed");

    let retrieved = svc.get_event(&emitted.id).await.unwrap().unwrap();
    assert_eq!(retrieved.event_type, "task.completed");
    assert_eq!(retrieved.agent_id.as_deref(), Some("agent-1"));
}

#[tokio::test]
async fn test_get_nonexistent() {
    let svc = make_svc();
    let id = AuditEventId(uuid::Uuid::nil());
    let retrieved = svc.get_event(&id).await.unwrap();
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_list_all() {
    let svc = make_svc();
    let e1 = make_event("00000000-0000-0000-0000-000000000001");
    let e2 = make_event("00000000-0000-0000-0000-000000000002");
    svc.emit_event(e1).await.unwrap();
    svc.emit_event(e2).await.unwrap();

    let all = svc.list_all().await.unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn test_list_by_agent() {
    let svc = make_svc();
    let mut e1 = make_event("00000000-0000-0000-0000-000000000001");
    let mut e2 = make_event("00000000-0000-0000-0000-000000000002");
    e1.agent_id = Some("agent-a".into());
    e2.agent_id = Some("agent-b".into());
    svc.emit_event(e1).await.unwrap();
    svc.emit_event(e2).await.unwrap();

    let by_a = svc.list_by_agent("agent-a").await.unwrap();
    assert_eq!(by_a.len(), 1);
    assert_eq!(by_a[0].agent_id.as_deref(), Some("agent-a"));
}

#[tokio::test]
async fn test_list_by_event_type() {
    let svc = make_svc();
    let mut e1 = make_event("00000000-0000-0000-0000-000000000001");
    let mut e2 = make_event("00000000-0000-0000-0000-000000000002");
    e1.event_type = "spawn.approved".into();
    e2.event_type = "task.completed".into();
    svc.emit_event(e1).await.unwrap();
    svc.emit_event(e2).await.unwrap();

    let completed = svc.list_by_event_type("task.completed").await.unwrap();
    assert_eq!(completed.len(), 1);
}

#[tokio::test]
async fn test_count() {
    let svc = make_svc();
    svc.emit_event(make_event("00000000-0000-0000-0000-000000000001")).await.unwrap();
    svc.emit_event(make_event("00000000-0000-0000-0000-000000000002")).await.unwrap();
    assert_eq!(svc.count().await.unwrap(), 2);
}
