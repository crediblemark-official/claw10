use super::*;

fn make_svc() -> ArtifactService {
    let store = Arc::new(clawhive_store::InMemoryStore::new()) as Arc<dyn Store>;
    ArtifactService::new(store)
}

fn make_task_id() -> TaskId {
    TaskId(uuid::Uuid::now_v7())
}

fn make_agent_id() -> AgentId {
    AgentId(uuid::Uuid::now_v7())
}

#[tokio::test]
async fn test_store_and_get_artifact() {
    let svc = make_svc();
    let task_id = make_task_id();
    let agent_id = make_agent_id();

    let artifact = svc
        .store_artifact(task_id.clone(), agent_id.clone(), "report.txt".into(), "text/plain".into(), b"hello world".to_vec())
        .await
        .unwrap();

    assert_eq!(artifact.name, "report.txt");
    assert_eq!(artifact.size_bytes, 11);

    let retrieved = svc.get_artifact(&artifact.id).await.unwrap().unwrap();
    assert_eq!(retrieved.name, "report.txt");
    assert_eq!(retrieved.content_hash, artifact.content_hash);
}

#[tokio::test]
async fn test_get_content() {
    let svc = make_svc();
    let content = b"hello world".to_vec();
    let artifact = svc
        .store_artifact(make_task_id(), make_agent_id(), "f.txt".into(), "text/plain".into(), content.clone())
        .await
        .unwrap();

    let stored = svc.get_content(&artifact.id).await.unwrap().unwrap();
    assert_eq!(stored, content);
}

#[tokio::test]
async fn test_verify_content_correct() {
    let svc = make_svc();
    let artifact = svc
        .store_artifact(make_task_id(), make_agent_id(), "f.txt".into(), "text/plain".into(), b"data".to_vec())
        .await
        .unwrap();

    assert!(svc.verify_content(&artifact.id).await.unwrap());
}

#[tokio::test]
async fn test_verify_content_fails_after_corruption() {
    let svc = make_svc();
    let artifact = svc
        .store_artifact(make_task_id(), make_agent_id(), "f.txt".into(), "text/plain".into(), b"original".to_vec())
        .await
        .unwrap();

    // Corrupt content directly in store
    svc.store
        .set_raw(&ArtifactService::content_key(&artifact.id), b"tampered".to_vec())
        .await
        .unwrap();

    assert!(!svc.verify_content(&artifact.id).await.unwrap());
}

#[tokio::test]
async fn test_delete_artifact() {
    let svc = make_svc();
    let artifact = svc
        .store_artifact(make_task_id(), make_agent_id(), "f.txt".into(), "text/plain".into(), b"data".to_vec())
        .await
        .unwrap();

    svc.delete_artifact(&artifact.id).await.unwrap();
    assert!(svc.get_artifact(&artifact.id).await.unwrap().is_none());
    assert!(svc.get_content(&artifact.id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_list_by_task() {
    let svc = make_svc();
    let task_a = make_task_id();
    let task_b = make_task_id();
    let agent = make_agent_id();

    svc.store_artifact(task_a.clone(), agent.clone(), "a1.txt".into(), "text/plain".into(), b"a1".to_vec())
        .await.unwrap();
    svc.store_artifact(task_a.clone(), agent.clone(), "a2.txt".into(), "text/plain".into(), b"a2".to_vec())
        .await.unwrap();
    svc.store_artifact(task_b.clone(), agent.clone(), "b1.txt".into(), "text/plain".into(), b"b1".to_vec())
        .await.unwrap();

    let by_a = svc.list_by_task(&task_a).await.unwrap();
    assert_eq!(by_a.len(), 2);

    let by_b = svc.list_by_task(&task_b).await.unwrap();
    assert_eq!(by_b.len(), 1);
}

#[tokio::test]
async fn test_list_by_agent() {
    let svc = make_svc();
    let task = make_task_id();
    let agent_a = make_agent_id();
    let agent_b = make_agent_id();

    svc.store_artifact(task.clone(), agent_a.clone(), "a1.txt".into(), "text/plain".into(), b"a1".to_vec())
        .await.unwrap();
    svc.store_artifact(task.clone(), agent_a.clone(), "a2.txt".into(), "text/plain".into(), b"a2".to_vec())
        .await.unwrap();
    svc.store_artifact(task.clone(), agent_b.clone(), "b1.txt".into(), "text/plain".into(), b"b1".to_vec())
        .await.unwrap();

    let by_a = svc.list_by_agent(&agent_a).await.unwrap();
    assert_eq!(by_a.len(), 2);

    let by_b = svc.list_by_agent(&agent_b).await.unwrap();
    assert_eq!(by_b.len(), 1);
}

#[tokio::test]
async fn test_count() {
    let svc = make_svc();
    let agent = make_agent_id();
    svc.store_artifact(make_task_id(), agent.clone(), "f1".into(), "text/plain".into(), b"1".to_vec())
        .await.unwrap();
    svc.store_artifact(make_task_id(), agent.clone(), "f2".into(), "text/plain".into(), b"2".to_vec())
        .await.unwrap();
    assert_eq!(svc.count().await.unwrap(), 2);
}
