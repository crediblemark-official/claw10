//! Trait `EventBus` dan tipe-tipe pendukungnya.
//!
//! Semua implementasi (InMemory, NATS) harus impl trait ini.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;

use crate::events::ClawHiveEvent;

#[derive(Debug, thiserror::Error)]
pub enum EventBusError {
    #[error("publish gagal: {0}")]
    Publish(String),

    #[error("subscribe gagal: {0}")]
    Subscribe(String),

    #[error("serialisasi event gagal: {0}")]
    Serialization(String),

    #[error("bus error: {0}")]
    Other(String),
}

/// ID subscription untuk bisa di-unsubscribe nanti.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub String);

/// Handler function type untuk event subscriber.
pub type EventHandler =
    Arc<dyn Fn(ClawHiveEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Trait utama event bus — implementasi oleh InMemory dan NATS.
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish satu event ke bus.
    async fn publish(&self, event: ClawHiveEvent) -> Result<(), EventBusError>;

    /// Publish batch events.
    async fn publish_many(&self, events: Vec<ClawHiveEvent>) -> Result<(), EventBusError> {
        for event in events {
            self.publish(event).await?;
        }
        Ok(())
    }

    /// Subscribe ke event dengan subject pattern (e.g. `clawhive.agent.*`).
    /// Mengembalikan SubscriptionId untuk unsubscribe.
    async fn subscribe(
        &self,
        subject_pattern: &str,
        handler: EventHandler,
    ) -> Result<SubscriptionId, EventBusError>;

    /// Unsubscribe berdasarkan ID.
    async fn unsubscribe(&self, id: &SubscriptionId) -> Result<(), EventBusError>;
}
