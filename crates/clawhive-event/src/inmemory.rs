//! `InMemoryEventBus` — implementasi event bus untuk testing dan development lokal.
//!
//! Tidak memerlukan NATS server. Semua event di-broadcast ke subscribers yang
//! sedang aktif secara in-process via tokio channels.
//!
//! Pattern matching subject menggunakan wildcard `*` (satu segment) dan `>` (semua segment).

use std::collections::{HashMap, VecDeque};
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

/// Kapasitas default history event yang disimpan (ring buffer).
const DEFAULT_HISTORY_CAPACITY: usize = 1000;

/// In-memory event bus untuk testing dan local dev.
/// Thread-safe melalui `Arc<RwLock<...>>`.
pub struct InMemoryEventBus {
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
    /// History event terakhir yang dipublish (ring buffer, bounded).
    published: Arc<RwLock<VecDeque<ClawHiveEvent>>>,
    history_capacity: usize,
}

impl InMemoryEventBus {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HISTORY_CAPACITY)
    }

    /// Buat bus dengan kapasitas history tertentu.
    /// Set ke 0 untuk menonaktifkan penyimpanan history.
    #[must_use]
    pub fn with_capacity(history_capacity: usize) -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            published: Arc::new(RwLock::new(VecDeque::with_capacity(
                history_capacity.min(DEFAULT_HISTORY_CAPACITY),
            ))),
            history_capacity,
        }
    }

    /// Ambil event yang sudah dipublish (terbaru sampai kapasitas history).
    pub async fn published_events(&self) -> Vec<ClawHiveEvent> {
        self.published.read().await.iter().cloned().collect()
    }

    /// Cek apakah subject event cocok dengan pattern subscriber.
    /// Mendukung `*` (satu segment) dan `>` (sisa semua segment).
    fn matches(pattern: &str, subject: &str) -> bool {
        let mut pattern_parts = pattern.split('.');
        let mut subject_parts = subject.split('.');

        loop {
            match (pattern_parts.next(), subject_parts.next()) {
                (Some(">"), _) => return true,
                (Some("*"), Some(_)) => continue,
                (Some(p), Some(s)) if p == s => continue,
                (None, None) => return true,
                _ => return false,
            }
        }
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
        // Simpan ke history (bounded ring buffer)
        if self.history_capacity > 0 {
            let mut published = self.published.write().await;
            if published.len() >= self.history_capacity {
                published.pop_front();
            }
            published.push_back(event.clone());
        }

        let subject = event.subject();

        // Snapshot matching handlers supaya lock tidak dipegang saat spawn task
        let handlers: Vec<EventHandler> = {
            let subs = self.subscriptions.read().await;
            subs.values()
                .filter(|sub| Self::matches(&sub.pattern, subject))
                .map(|sub| Arc::clone(&sub.handler))
                .collect()
        };

        for handler in handlers {
            let event_clone = event.clone();
            tokio::spawn(async move {
                let fut: Pin<Box<dyn std::future::Future<Output = ()> + Send>> =
                    handler(event_clone);
                fut.await;
            });
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
#[path = "inmemory_test.rs"]
mod tests;
