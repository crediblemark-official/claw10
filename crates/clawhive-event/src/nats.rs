#![cfg(feature = "nats")]
#![allow(clippy::pedantic)]

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use async_trait::async_trait;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::bus::{EventBus, EventBusError, EventHandler, SubscriptionId};
use crate::events::ClawHiveEvent;

/// Implementasi EventBus berbasis NATS (JetStream-ready / PubSub).
/// Terintegrasi dengan runtime Tokio async menggunakan OS thread untuk sinkronisasi NATS client.
pub struct NatsEventBus {
    client: nats::Connection,
    /// Menyimpan token pembatalan (JoinHandle tokio async loop) untuk setiap subscription.
    subscriptions: Arc<Mutex<HashMap<SubscriptionId, (tokio::task::JoinHandle<()>, nats::Subscription)>>>,
}

impl NatsEventBus {
    /// Membuat instance baru NatsEventBus dan menghubungkan ke NATS server.
    /// Menggunakan env var `NATS_URL` jika tersedia, fallback ke `url` parameter.
    ///
    /// # Errors
    /// Mengembalikan error jika gagal terhubung ke server NATS.
    pub fn new(url: &str) -> Result<Self, EventBusError> {
        let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| url.to_string());
        let client = nats::Options::new()
            .max_reconnects(10)
            .reconnect_buffer_size(100)
            .connect(&nats_url)
            .map_err(|e| EventBusError::Other(format!("Gagal terhubung ke NATS di {nats_url}: {e}")))?;
        tracing::info!("Terhubung ke NATS server di {nats_url}");

        Ok(Self {
            client,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

#[async_trait]
impl EventBus for NatsEventBus {
    async fn publish(&self, event: ClawHiveEvent) -> Result<(), EventBusError> {
        let subject = event.subject().to_string();
        let payload = serde_json::to_vec(&event)
            .map_err(|e| EventBusError::Serialization(e.to_string()))?;

        let client = self.client.clone();

        // Jalankan publish sinkron di tokio blocking thread pool
        tokio::task::spawn_blocking(move || {
            client.publish(&subject, &payload)
        })
        .await
        .map_err(|e| EventBusError::Publish(format!("Tokio spawn_blocking error: {e}")))?
        .map_err(|e| EventBusError::Publish(format!("NATS publish error: {e}")))?;

        let _ = ui_trace_publish(event);
        Ok(())
    }

    async fn subscribe(
        &self,
        subject_pattern: &str,
        handler: EventHandler,
    ) -> Result<SubscriptionId, EventBusError> {
        let sub = self.client.subscribe(subject_pattern)
            .map_err(|e| EventBusError::Subscribe(format!("NATS subscribe error: {e}")))?;

        let id = SubscriptionId(Uuid::now_v7().to_string());
        let (tx, mut rx) = mpsc::unbounded_channel::<nats::Message>();

        // 1. Spawn OS Thread untuk mem-polling blocking iterator NATS message
        let sub_clone = sub.clone();
        thread::spawn(move || {
            for msg in sub_clone.messages() {
                if tx.send(msg).is_err() {
                    break; // Receiver di-drop, matikan loop thread
                }
            }
        });

        // 2. Spawn Tokio async task untuk memproses data dari channel secara asinkron
        let handler_clone = Arc::clone(&handler);
        let join_handle = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let event_res = serde_json::from_slice::<ClawHiveEvent>(&msg.data);
                match event_res {
                    Ok(event) => {
                        let h = Arc::clone(&handler_clone);
                        tokio::spawn(async move {
                            h(event).await;
                        });
                    }
                    Err(e) => {
                        tracing::error!("NATS event bus gagal melakukan deserialisasi event: {e}");
                    }
                }
            }
        });

        // Simpan handle agar bisa di-unsubscribe/abort nanti
        self.subscriptions.lock().await.insert(id.clone(), (join_handle, sub));

        Ok(id)
    }

    async fn unsubscribe(&self, id: &SubscriptionId) -> Result<(), EventBusError> {
        let mut subs = self.subscriptions.lock().await;
        if let Some((join_handle, sub)) = subs.remove(id) {
            // Abort tokio loop task
            join_handle.abort();

            // Panggil unsubscribe sinkron dari NATS client untuk melepas interest di server-side
            let _ = tokio::task::spawn_blocking(move || {
                sub.unsubscribe()
            })
            .await;
        }

        Ok(())
    }
}

// Trace helper untuk debugging internal
fn ui_trace_publish(event: ClawHiveEvent) -> ClawHiveEvent {
    tracing::debug!("Event published to NATS: {:?}", event.subject());
    event
}

#[cfg(test)]
#[path = "nats_test.rs"]
mod tests;
