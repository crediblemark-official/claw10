use super::*;

#[tokio::test]
async fn test_in_memory_store_roundtrip() {
    let store = InMemoryStore::new();
    store.set("test:key", &42u32).await.unwrap();
    let val: Option<u32> = store.get("test:key").await.unwrap();
    assert_eq!(val, Some(42));
}

#[tokio::test]
async fn test_in_memory_store_not_found() {
    let store = InMemoryStore::new();
    let val: Option<u32> = store.get("nonexistent").await.unwrap();
    assert!(val.is_none());
}

#[tokio::test]
async fn test_in_memory_store_delete() {
    let store = InMemoryStore::new();
    store.set("test:key", &42u32).await.unwrap();
    store.delete("test:key").await.unwrap();
    let val: Option<u32> = store.get("test:key").await.unwrap();
    assert!(val.is_none());
}

#[tokio::test]
async fn test_in_memory_store_exists() {
    let store = InMemoryStore::new();
    store.set("test:key", &42u32).await.unwrap();
    assert!(store.exists("test:key").await.unwrap());
    assert!(!store.exists("nonexistent").await.unwrap());
}

#[tokio::test]
async fn test_in_memory_store_scan_prefix() {
    let store = InMemoryStore::new();
    store.set("agent:a1", &"agent-one").await.unwrap();
    store.set("agent:a2", &"agent-two").await.unwrap();
    store.set("task:t1", &"task-one").await.unwrap();

    let results: Vec<(String, String)> = store.scan_prefix("agent:").await.unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].1, "agent-one");
    assert_eq!(results[1].1, "agent-two");
}

#[tokio::test]
async fn test_in_memory_store_clear() {
    let store = InMemoryStore::new();
    store.set("test:key", &42u32).await.unwrap();
    store.clear().await.unwrap();
    let val: Option<u32> = store.get("test:key").await.unwrap();
    assert!(val.is_none());
}

#[tokio::test]
async fn test_sled_store_roundtrip() {
    let store = SledStore::new_temporary().unwrap();
    store.set("test:key", &42u32).await.unwrap();
    let val: Option<u32> = store.get("test:key").await.unwrap();
    assert_eq!(val, Some(42));
}

#[tokio::test]
async fn test_sled_store_scan_prefix() {
    let store = SledStore::new_temporary().unwrap();
    store.set("agent:a1", &"agent-one").await.unwrap();
    store.set("agent:a2", &"agent-two").await.unwrap();
    store.set("task:t1", &"task-one").await.unwrap();

    let results: Vec<(String, String)> = store.scan_prefix("agent:").await.unwrap();
    assert_eq!(results.len(), 2);
}
