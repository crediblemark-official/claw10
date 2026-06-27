//! `InMemoryEventBus` — implementasi event bus untuk testing dan development lokal.
//!
//! Tidak memerlukan NATS server. Semua event di-broadcast ke subscribers yang
//! sedang aktif secara in-process via tokio channels.
//!
//! Pattern matching subject menggunakan wildcard `*` (satu segment) dan `>` (semua segment).

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::bus::{EventBus, EventBusError, EventHandler, SubscriptionId};
use crate::events::ClawHiveEvent;

struct Subscription {
    pattern: String,
    handler: EventHandler,
}

/// In-memory event bus untuk testing dan local dev.
/// Thread-safe melalui `Arc<RwLock<...>>`.
pub struct InMemoryEventBus {
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
    /// Semua event yang pernah dipublish (untuk inspeksi dalam test).
    published: Arc<RwLock<Vec<ClawHiveEvent>>>,
}

impl InMemoryEventBus {
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            published: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Ambil semua event yang sudah dipublish (untuk inspeksi dalam test).
    pub async fn published_events(&self) -> Vec<ClawHiveEvent> {
        self.published.read().await.clone()
    }

    /// Cek apakah subject event cocok dengan pattern subscriber.
    /// Mendukung `*` (satu segment) dan `>` (sisa semua segment).
    fn matches(pattern: &str, subject: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('.').collect();
        let subject_parts: Vec<&str> = subject.split('.').collect();

        let mut p_idx = 0;
        let mut s_idx = 0;

        while p_idx < pattern_parts.len() && s_idx < subject_parts.len() {
            let p = pattern_parts[p_idx];

            if p == ">" {
                // `>` cocok dengan sisa semua segment
                return true;
            } else if p == "*" {
                // `*` cocok dengan satu segment apapun
                p_idx += 1;
                s_idx += 1;
            } else if p == subject_parts[s_idx] {
                p_idx += 1;
                s_idx += 1;
            } else {
                return false;
            }
        }

        p_idx == pattern_parts.len() && s_idx == subject_parts.len()
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, event: ClawHiveEvent) -> Result<(), EventBusError> {
        // Simpan ke history
        self.published.write().await.push(event.clone());

        let subject = event.subject();
        let subs = self.subscriptions.read().await;

        for sub in subs.values() {
            if Self::matches(&sub.pattern, subject) {
                let handler = Arc::clone(&sub.handler);
                let event_clone = event.clone();
                // Fire-and-forget: spawn task untuk tiap handler
                tokio::spawn(async move {
                    let fut: Pin<Box<dyn std::future::Future<Output = ()> + Send>> =
                        handler(event_clone);
                    fut.await;
                });
            }
        }

        Ok(())
    }

    async fn subscribe(
        &self,
        subject_pattern: &str,
        handler: EventHandler,
    ) -> Result<SubscriptionId, EventBusError> {
        let id = SubscriptionId(Uuid::now_v7().to_string());
        let sub = Subscription {
            pattern: subject_pattern.to_string(),
            handler,
        };
        self.subscriptions
            .write()
            .await
            .insert(id.0.clone(), sub);
        Ok(id)
    }

    async fn unsubscribe(&self, id: &SubscriptionId) -> Result<(), EventBusError> {
        self.subscriptions.write().await.remove(&id.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_subject_matching() {
        assert!(InMemoryEventBus::matches(
            "clawhive.agent.*",
            "clawhive.agent.spawned"
        ));
        assert!(InMemoryEventBus::matches(
            "clawhive.>",
            "clawhive.agent.spawned"
        ));
        assert!(InMemoryEventBus::matches(
            "clawhive.agent.spawned",
            "clawhive.agent.spawned"
        ));
        assert!(!InMemoryEventBus::matches(
            "clawhive.task.*",
            "clawhive.agent.spawned"
        ));
        assert!(!InMemoryEventBus::matches(
            "clawhive.agent.hibernated",
            "clawhive.agent.spawned"
        ));
    }

    #[tokio::test]
    async fn test_publish_and_receive() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let bus = Arc::new(InMemoryEventBus::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        bus.subscribe(
            "clawhive.agent.*",
            Arc::new(move |_event| {
                let c = Arc::clone(&counter_clone);
                Box::pin(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                })
            }),
        )
        .await
        .unwrap();

        bus.publish(ClawHiveEvent::AgentSpawned {
            agent_id: Uuid::now_v7(),
            parent_agent_id: None,
            mission_id: Uuid::now_v7(),
            role: "specialist".into(),
            lifecycle_mode: "ephemeral".into(),
            timestamp: Utc::now(),
        })
        .await
        .unwrap();

        // Tunggu handler selesai
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_unsubscribe() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let bus = Arc::new(InMemoryEventBus::new());
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let sub_id = bus
            .subscribe(
                "clawhive.>",
                Arc::new(move |_event| {
                    let c = Arc::clone(&counter_clone);
                    Box::pin(async move {
                        c.fetch_add(1, Ordering::SeqCst);
                    })
                }),
            )
            .await
            .unwrap();

        bus.unsubscribe(&sub_id).await.unwrap();

        bus.publish(ClawHiveEvent::AgentSpawned {
            agent_id: Uuid::now_v7(),
            parent_agent_id: None,
            mission_id: Uuid::now_v7(),
            role: "specialist".into(),
            lifecycle_mode: "ephemeral".into(),
            timestamp: Utc::now(),
        })
        .await
        .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        // Handler sudah di-unsubscribe, counter harus tetap 0
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_published_history() {
        let bus = InMemoryEventBus::new();

        bus.publish(ClawHiveEvent::AgentHibernated {
            agent_id: Uuid::now_v7(),
            checkpoint_id: Uuid::now_v7(),
            timestamp: Utc::now(),
        })
        .await
        .unwrap();

        let events = bus.published_events().await;
        assert_eq!(events.len(), 1);
    }
}
