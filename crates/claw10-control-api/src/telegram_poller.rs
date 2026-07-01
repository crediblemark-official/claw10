use std::collections::HashMap;
use std::sync::Arc;

use claw10_agent::{AgentRuntime, AgentStore};
use claw10_budget::BudgetService;
use claw10_domain::{AgentId, WorkerId};
use claw10_store::StoreExt;

use crate::state::AppState;

/// Mulai background task polling getUpdates Telegram jika TELEGRAM_BOT_TOKEN di-set di env.
/// Pesan masuk yang dideteksi akan diteruskan ke gateway_service lalu dieksekusi oleh agen.
pub fn start_telegram_poller(state: AppState) {
    let Ok(token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        return;
    };
    if token.trim().is_empty() {
        return;
    }

    tokio::spawn(async move {
        // Beri jeda kecil agar state dan DB sudah siap
        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

        // Temukan channel Telegram yang aktif dari KV store
        let channels = match state
            .kv_store
            .scan_prefix::<claw10_domain::Channel>("gateway:channel:")
            .await
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("[Telegram Poller] Gagal scan channel: {e}");
                return;
            }
        };

        let telegram_channel = channels
            .into_iter()
            .find(|(_, ch)| {
                ch.channel_type == claw10_domain::ChannelType::Telegram
                    && ch.is_active
                    && ch.config
                        .get("bot_token")
                        .and_then(|v| v.as_str())
                        .map(|t| t == token)
                        .unwrap_or(false)
            });

        let (channel_id, channel) = match telegram_channel {
            Some(pair) => pair,
            None => {
                tracing::info!("[Telegram Poller] Tidak ditemukan channel Telegram aktif untuk token ini.");
                return;
            }
        };

        let agent_id_str = match channel.config.get("agent_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => {
                tracing::warn!("[Telegram Poller] Channel Telegram tidak memiliki agent_id.");
                return;
            }
        };

        let agent_uuid = match agent_id_str.parse::<uuid::Uuid>() {
            Ok(u) => u,
            Err(_) => {
                tracing::warn!("[Telegram Poller] agent_id tidak valid: {agent_id_str}");
                return;
            }
        };

        let client = match reqwest::Client::builder().build() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("[Telegram Poller] Gagal build HTTP client: {e}");
                return;
            }
        };

        // Hapus webhook aktif agar getUpdates bisa berfungsi
        let del_url = format!("https://api.telegram.org/bot{token}/deleteWebhook");
        if let Err(e) = client.get(&del_url).send().await {
            tracing::warn!("[Telegram Poller] deleteWebhook gagal: {e}");
        } else {
            tracing::info!("[Telegram Poller] Webhook dihapus, memulai polling mode...");
        }

        let mut offset = 0i64;

        loop {
            let url = format!(
                "https://api.telegram.org/bot{token}/getUpdates?offset={offset}&timeout=20"
            );

            let response = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!("[Telegram Poller] Request gagal, retry: {e}");
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    continue;
                }
            };

            let json: serde_json::Value = match response.json().await {
                Ok(j) => j,
                Err(e) => {
                    tracing::debug!("[Telegram Poller] Parse response gagal: {e}");
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue;
                }
            };

            if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
                let desc = json
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                tracing::error!("[Telegram Poller] Error dari Telegram API: {desc}");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }

            let updates = match json.get("result").and_then(|v| v.as_array()) {
                Some(arr) => arr.clone(),
                None => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            for update in &updates {
                if let Some(update_id) = update.get("update_id").and_then(|v| v.as_i64()) {
                    offset = update_id + 1;
                }

                // Ambil teks pesan dan chat_id pengirim
                let message = match update.get("message") {
                    Some(m) => m,
                    None => continue,
                };

                let text = match message.get("text").and_then(|v| v.as_str()) {
                    Some(t) if !t.trim().is_empty() => t.trim().to_string(),
                    _ => continue,
                };

                let from_chat_id = match message
                    .get("chat")
                    .and_then(|c| c.get("id"))
                    .map(|v| v.to_string())
                {
                    Some(id) => id,
                    None => continue,
                };

                let username = message
                    .get("from")
                    .and_then(|f| f.get("username"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("user")
                    .to_string();

                tracing::info!(
                    "[Telegram Poller] Pesan baru dari @{username} (chat_id={from_chat_id}): {text}"
                );

                // Teruskan ke agen dan balas ke Telegram
                let state_clone = state.clone();
                let channel_id_clone = channel_id.clone();
                let agent_uuid_clone = agent_uuid;
                let text_clone = text.clone();
                let from_chat_id_clone = from_chat_id.clone();

                tokio::spawn(async move {
                    if let Err(e) = run_agent_and_reply(
                        state_clone,
                        AgentId(agent_uuid_clone),
                        text_clone,
                        from_chat_id_clone,
                        channel_id_clone,
                    )
                    .await
                    {
                        tracing::warn!("[Telegram Poller] Gagal jalankan agen: {e}");
                    }
                });
            }

            if updates.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        }
    });
}

/// Eksekusi agen berdasarkan pesan masuk dan kirim hasil ke channel gateway (Telegram).
async fn run_agent_and_reply(
    state: AppState,
    agent_id: AgentId,
    objective: String,
    recipient: String,
    channel_id: String,
) -> Result<(), String> {
    let model_router = state
        .model_router
        .clone()
        .ok_or_else(|| "model router not configured".to_string())?;
    let tool_registry = state
        .tool_registry
        .clone()
        .ok_or_else(|| "tool registry not configured".to_string())?;

    let agent_store = AgentStore::new(Arc::clone(&state.kv_store));
    let budget_service = Arc::new(BudgetService);

    let runtime = AgentRuntime::new(
        agent_store,
        model_router,
        tool_registry,
        budget_service,
        Arc::clone(&state.worker_service),
        Some(WorkerId(uuid::Uuid::now_v7())),
    );

    let (session, _events) = runtime
        .execute_agent(&agent_id, objective, HashMap::new(), None, None)
        .await
        .map_err(|e| e.to_string())?;

    let reply_text = session
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, claw10_model_router::types::MessageRole::Assistant))
        .map(|m| m.content.clone())
        .unwrap_or_else(|| "(no response)".into());

    let message = claw10_gateway::Message {
        recipient,
        subject: None,
        body: reply_text,
        metadata: None,
    };

    state
        .gateway_service
        .dispatch(&channel_id, &message)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
