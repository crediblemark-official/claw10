use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use claw10_agent::{AgentRuntime, AgentStore};
use claw10_budget::BudgetService;
use claw10_domain::{AgentId, WorkerId};
use claw10_store::StoreExt;

use crate::state::AppState;

/// Shared shutdown flag for the Telegram poller and heartbeat loop.
static TELEGRAM_SHUTDOWN: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();

fn shutdown_flag() -> Arc<AtomicBool> {
    TELEGRAM_SHUTDOWN
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Signal the Telegram poller to shut down gracefully.
pub fn signal_telegram_shutdown() {
    shutdown_flag().store(true, Ordering::SeqCst);
}

/// Mulai background task polling getUpdates Telegram jika TELEGRAM_BOT_TOKEN di-set di env.
/// Pesan masuk yang dideteksi akan diteruskan ke gateway_service lalu dieksekusi oleh agen.
pub fn start_telegram_poller(state: AppState) {
    let Ok(token) = std::env::var("TELEGRAM_BOT_TOKEN") else {
        return;
    };
    if token.trim().is_empty() {
        return;
    }

    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
    if chat_id.trim().is_empty() {
        tracing::info!("[Telegram Poller] TELEGRAM_CHAT_ID belum dikonfigurasi. Poller backend dinonaktifkan.");
        return;
    }

    let shutdown = shutdown_flag();

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

        let (_channel_id, channel) = match telegram_channel {
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

        // Daftarkan slash commands ke Telegram Bot agar muncul popup '/'
        let cmd_url = format!("https://api.telegram.org/bot{token}/setMyCommands");
        let cmd_payload = serde_json::json!({
            "commands": [
                {"command": "start", "description": "Show welcome message"},
                {"command": "help", "description": "Show available commands"},
                {"command": "agents", "description": "List running agents"},
                {"command": "agent", "description": "Select agent (/agent <name>)"},
                {"command": "new", "description": "Reset session (clear history)"}
            ]
        });
        let client_cmd = client.clone();
        tokio::spawn(async move {
            if let Err(e) = client_cmd.post(&cmd_url).json(&cmd_payload).send().await {
                tracing::warn!("[Telegram Poller] setMyCommands gagal: {e}");
            } else {
                tracing::info!("[Telegram Poller] Slash commands berhasil didaftarkan ke Telegram.");
            }
        });

        // Mulai background Heartbeat Loop secara terpisah
        let state_heartbeat = state.clone();
        let agent_id_heartbeat = AgentId(agent_uuid);
        let channel_id_heartbeat = channel.id.clone();
        let recipient_heartbeat = chat_id.clone();
        let heartbeat_shutdown = shutdown.clone();

        tokio::spawn(async move {
            // Jeda awal agar inisialisasi server selesai
            tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;

            loop {
                // Periksa apakah service di-shutdown
                if heartbeat_shutdown.load(Ordering::SeqCst) {
                    tracing::info!("[Heartbeat] Shutdown signal diterima. Menghentikan heartbeat loop.");
                    break;
                }

                // 1. Baca daftar heartbeat tasks secara dinamis dari database KV store
                let heartbeat_tasks = state_heartbeat
                    .kv_store
                    .get::<Vec<String>>("profile:heartbeat_tasks")
                    .await
                    .unwrap_or_default()
                    .unwrap_or_default();

                if !heartbeat_tasks.is_empty() {
                    tracing::info!("[Heartbeat] Mengeksekusi {} heartbeat tasks...", heartbeat_tasks.len());
                    for task_obj in &heartbeat_tasks {
                        let task_obj = task_obj.trim().to_string();
                        if task_obj.is_empty() {
                            continue;
                        }

                        tracing::info!("[Heartbeat] Menjalankan task: '{}'", task_obj);
                        
                        // Jalankan agen untuk menyelesaikan tugas heartbeat ini
                        match execute_heartbeat_task(
                            state_heartbeat.clone(),
                            agent_id_heartbeat.clone(),
                            task_obj.clone(),
                        ).await {
                            Ok(report) => {
                                // Kirim respon jika ada temuan penting
                                if !report.trim().is_empty() && !report.contains("(no response)") {
                                    let message = claw10_gateway::Message {
                                        recipient: recipient_heartbeat.clone(),
                                        subject: Some("Heartbeat Laporan".to_string()),
                                        body: format!("💓 *[Heartbeat Laporan]*\nTugas: _{}_\n\n{}", task_obj, report),
                                        metadata: None,
                                    };
                                    let _ = state_heartbeat.gateway_service.dispatch(&channel_id_heartbeat, &message).await;
                                }
                            }
                            Err(e) => {
                                tracing::error!("[Heartbeat] Gagal menjalankan task '{}': {}", task_obj, e);
                            }
                        }
                    }
                }

                // Jalankan setiap 60 detik
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        });

        let mut offset = 0i64;

        loop {
            if shutdown.load(Ordering::SeqCst) {
                tracing::info!("[Telegram Poller] Shutdown signal diterima. Menghentikan loop poller.");
                break;
            }

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
                let channel_id_clone = channel.id.clone();
                let agent_uuid_clone = agent_uuid;
                let text_clone = text.clone();
                let from_chat_id_clone = from_chat_id.clone();

                let token_clone = token.clone();

                tokio::spawn(async move {
                    // Kirim typing action segera setelah pesan diterima
                    send_typing_action(&token_clone, &from_chat_id_clone).await;

                    let bootstrap_completed = state_clone
                        .kv_store
                        .get::<bool>("config:bootstrap_completed")
                        .await
                        .unwrap_or_default()
                        .unwrap_or(false);

                    if !bootstrap_completed {
                        if let Err(e) = run_bootstrap_interview(
                            state_clone,
                            AgentId(agent_uuid_clone),
                            text_clone,
                            from_chat_id_clone,
                            channel_id_clone,
                        )
                        .await {
                            tracing::error!("[Bootstrap] Error pada wawancara: {e}");
                        }
                    } else {
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
        AgentStore::new(Arc::clone(&state.kv_store)),
        model_router.clone(),
        tool_registry,
        budget_service,
        Arc::clone(&state.worker_service),
        Some(WorkerId(uuid::Uuid::now_v7())),
    );

    let agent = agent_store
        .get_or_not_found(&agent_id)
        .await
        .map_err(|e| e.to_string())?;

    let profile_id = agent.genome.model_policy.preferred_profile.clone();
    let fallbacks = agent.genome.model_policy.fallback_profiles.clone();

    let (session, _events) = runtime
        .execute_agent(&agent_id, objective.clone(), HashMap::new(), None, None)
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
        body: reply_text.clone(),
        metadata: None,
    };

    state
        .gateway_service
        .dispatch(&channel_id, &message)
        .await
        .map_err(|e| e.to_string())?;

    // Pemicu Memory Distillation secara asinkron di latar belakang
    let state_clone = state.clone();
    let agent_id_clone = agent_id.clone();
    let model_router_clone = model_router.clone();
    let user_msg = objective;
    let assistant_msg = reply_text;

    tokio::spawn(async move {
        if let Err(e) = perform_memory_distillation(
            state_clone,
            agent_id_clone,
            model_router_clone,
            profile_id,
            fallbacks,
            user_msg,
            assistant_msg,
        )
        .await
        {
            tracing::error!("[Memory Distillation] Gagal mendistilasikan memori: {e}");
        }
    });

    Ok(())
}

/// Helper untuk melakukan penyaringan (distilasi) memori agen secara asinkron menggunakan LLM.
async fn perform_memory_distillation(
    state: AppState,
    agent_id: AgentId,
    model_router: Arc<claw10_model_router::router::ModelRouter>,
    profile_id: String,
    fallbacks: Vec<String>,
    user_msg: String,
    assistant_msg: String,
) -> Result<(), String> {
    let distilled_key = format!("memory:distilled:agent:{}", agent_id.0);
    
    // 1. Ambil distilled memory lama dari KV store
    let old_memory = state
        .kv_store
        .get::<String>(&distilled_key)
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    // 2. Susun prompt untuk memicu perangkuman semantik dari berkas .icvs
    let prompts = claw10_icvs::IcvsCompiler::compile_prompt(
        claw10_prompt::distillation::DISTILLATION_SOURCE,
        "distillation",
    ).unwrap_or_default();

    let system_content = prompts.get(0)
        .map(|p| p.content.clone())
        .unwrap_or_else(|| "Anda adalah asisten pemelihara memori yang sangat ringkas.".to_string());

    let raw_user_content = prompts.get(1)
        .map(|p| p.content.clone())
        .unwrap_or_else(|| "Tugas Anda adalah merangkum obrolan.".to_string());

    let formatted_user_content = raw_user_content
        .replace("{old_memory}", if old_memory.trim().is_empty() { "(Kosong)" } else { &old_memory })
        .replace("{user_msg}", &user_msg)
        .replace("{assistant_msg}", &assistant_msg);

    let chat_request = claw10_model_router::types::ChatRequest {
        model: profile_id.clone(),
        messages: vec![
            claw10_model_router::types::ModelMessage {
                role: claw10_model_router::types::MessageRole::System,
                content: system_content,
                content_parts: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            claw10_model_router::types::ModelMessage {
                role: claw10_model_router::types::MessageRole::User,
                content: formatted_user_content,
                content_parts: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }
        ],
        max_tokens: Some(512),
        temperature: Some(0.3),
        tools: None,
        stop: None,
    };

    // 3. Panggil LLM
    let response = model_router
        .route_with_fallback(&profile_id, &fallbacks, chat_request)
        .await
        .map_err(|e| e.to_string())?;

    let new_memory = response.message.content.trim().to_string();
    if !new_memory.is_empty() {
        // 4. Simpan ke database
        state
            .kv_store
            .set(&distilled_key, &new_memory)
            .await
            .map_err(|e| e.to_string())?;
        tracing::info!("[Memory Distillation] Memori berhasil didistilasikan untuk agen {}. Hasil:\n{}", agent_id.0, new_memory);
    }

    Ok(())
}

/// Helper untuk mengeksekusi satu tugas heartbeat secara terisolasi.
async fn execute_heartbeat_task(
    state: AppState,
    agent_id: AgentId,
    objective: String,
) -> Result<String, String> {
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

    // Jalankan eksekusi agen (dengan batas max turns 10)
    let (session, _events) = runtime
        .execute_agent(&agent_id, objective, HashMap::new(), None, None)
        .await
        .map_err(|e| e.to_string())?;

    let report_text = session
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, claw10_model_router::types::MessageRole::Assistant))
        .map(|m| m.content.clone())
        .unwrap_or_else(|| "".into());

    Ok(report_text)
}

/// Menjalankan wawancara bootstrap secara interaktif dengan operator.
async fn run_bootstrap_interview(
    state: AppState,
    agent_id: AgentId,
    user_message: String,
    recipient: String,
    channel_id: String,
) -> Result<(), String> {
    let model_router = state
        .model_router
        .clone()
        .ok_or_else(|| "model router not configured".to_string())?;

    let agent_store = AgentStore::new(Arc::clone(&state.kv_store));
    let agent = agent_store
        .get_or_not_found(&agent_id)
        .await
        .map_err(|e| e.to_string())?;

    let profile_id = agent.genome.model_policy.preferred_profile.clone();
    let fallbacks = agent.genome.model_policy.fallback_profiles.clone();

    // 2. Ambil riwayat wawancara dari database KV store
    let history_key = format!("bootstrap:history:{}", recipient);
    let mut history = state
        .kv_store
        .get::<String>(&history_key)
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    // Tambahkan input terbaru dari user ke riwayat
    history.push_str(&format!("Operator: {}\n", user_message));

    // 3. Kompilasi bootstrap system prompt dari berkas .icvs
    let prompts = claw10_icvs::IcvsCompiler::compile_prompt(
        claw10_prompt::bootstrap::BOOTSTRAP_SOURCE,
        "bootstrap",
    ).unwrap_or_default();

    let system_prompt = prompts.get(0)
        .map(|p| p.content.clone())
        .unwrap_or_else(|| "Anda adalah asisten AI yang ramah. Lakukan wawancara.".to_string());

    let distillation_prompt_template = prompts.get(1)
        .map(|p| p.content.clone())
        .unwrap_or_else(|| "Ekstrak JSON.".to_string());

    // 4. Susun pesan chat untuk membalas ke user
    let mut messages = vec![
        claw10_model_router::types::ModelMessage {
            role: claw10_model_router::types::MessageRole::System,
            content: system_prompt,
            content_parts: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    ];

    // Masukkan percakapan riwayat ke model messages
    for line in history.lines() {
        if line.starts_with("Operator: ") {
            messages.push(claw10_model_router::types::ModelMessage {
                role: claw10_model_router::types::MessageRole::User,
                content: line["Operator: ".len()..].to_string(),
                content_parts: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        } else if line.starts_with("Asisten: ") {
            messages.push(claw10_model_router::types::ModelMessage {
                role: claw10_model_router::types::MessageRole::Assistant,
                content: line["Asisten: ".len()..].to_string(),
                content_parts: None,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }
    }

    let chat_request = claw10_model_router::types::ChatRequest {
        model: profile_id.clone(),
        messages,
        max_tokens: Some(512),
        temperature: Some(0.5),
        tools: None,
        stop: None,
    };

    // Panggil model
    let response = model_router
        .route_with_fallback(&profile_id, &fallbacks, chat_request)
        .await
        .map_err(|e| e.to_string())?;

    let reply_text = response.message.content.trim().to_string();

    // Simpan balasan asisten ke riwayat
    history.push_str(&format!("Asisten: {}\n", reply_text));
    state
        .kv_store
        .set(&history_key, &history)
        .await
        .map_err(|e| e.to_string())?;

    // 5. Kirim balasan ke user di Telegram
    let message = claw10_gateway::Message {
        recipient: recipient.clone(),
        subject: None,
        body: reply_text,
        metadata: None,
    };

    state
        .gateway_service
        .dispatch(&channel_id, &message)
        .await
        .map_err(|e| e.to_string())?;

    // 6. Jalankan pemecahan profiling (distilasi) secara asinkron untuk mendeteksi apakah data sudah lengkap
    let state_clone = state.clone();
    let history_clone = history;
    let recipient_clone = recipient;
    let channel_id_clone = channel_id;
    let model_router_clone = model_router;
    let profile_id_clone = profile_id;
    let fallbacks_clone = fallbacks;

    tokio::spawn(async move {
        let dist_prompt = distillation_prompt_template.replace("{dialog}", &history_clone);
        let request = claw10_model_router::types::ChatRequest {
            model: profile_id_clone.clone(),
            messages: vec![
                claw10_model_router::types::ModelMessage {
                    role: claw10_model_router::types::MessageRole::System,
                    content: "Anda adalah asisten data ekstraksi JSON.".to_string(),
                    content_parts: None,
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                },
                claw10_model_router::types::ModelMessage {
                    role: claw10_model_router::types::MessageRole::User,
                    content: dist_prompt,
                    content_parts: None,
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                }
            ],
            max_tokens: Some(512),
            temperature: Some(0.0),
            tools: None,
            stop: None,
        };

        match model_router_clone.route_with_fallback(&profile_id_clone, &fallbacks_clone, request).await {
            Ok(res) => {
                let toon_text = res.message.content.trim().to_string();
                let mut op_name = String::new();
                let mut agent_name = String::new();
                let mut agent_soul = String::new();

                for line in toon_text.lines() {
                    let line = line.trim();
                    if line.starts_with("operator_name:") {
                        op_name = line["operator_name:".len()..].trim().trim_matches('"').to_string();
                    } else if line.starts_with("agent_name:") {
                        agent_name = line["agent_name:".len()..].trim().trim_matches('"').to_string();
                    } else if line.starts_with("agent_soul:") {
                        agent_soul = line["agent_soul:".len()..].trim().trim_matches('"').to_string();
                    }
                }

                if !op_name.is_empty() && !agent_name.is_empty() && !agent_soul.is_empty() {
                    // Semua data lengkap! Simpan profil dinamis ke DB
                    let _ = state_clone.kv_store.set("profile:operator:name", &op_name).await;
                    let _ = state_clone.kv_store.set("profile:operator:timezone", &"WIB".to_string()).await;
                    let _ = state_clone.kv_store.set("profile:operator:language", &"Bahasa Indonesia".to_string()).await;
                    let _ = state_clone.kv_store.set("profile:operator:style", &"Santai, langsung ke poin".to_string()).await;

                    let _ = state_clone.kv_store.set("profile:agent:name", &agent_name).await;
                    let _ = state_clone.kv_store.set("profile:agent:soul", &agent_soul).await;

                    let _ = state_clone.kv_store.set("config:bootstrap_completed", &true).await;

                    // Bersihkan riwayat wawancara
                    let _ = state_clone.kv_store.delete(&history_key).await;

                    // Kirim pesan sukses
                    let welcome_msg = format!(
                        "🎉 *[Inisialisasi Sukses]*\n\n\
                        Profil berhasil dikonfigurasi secara dinamis!\n\n\
                        👤 *Operator:* {}\n\
                        🤖 *Nama Agen:* {}\n\
                        🧬 *Soul/Gaya:* {}\n\n\
                        Mulai sekarang saya siap membantu Anda. Silakan kirimkan perintah pertama Anda!",
                        op_name, agent_name, agent_soul
                    );
                    let message = claw10_gateway::Message {
                        recipient: recipient_clone,
                        subject: None,
                        body: welcome_msg,
                        metadata: None,
                    };
                    if let Err(e) = state_clone.gateway_service.dispatch(&channel_id_clone, &message).await {
                        tracing::error!("[Bootstrap] Gagal mengirim welcome message: {e}");
                    } else {
                        tracing::info!("[Bootstrap] Setup profil sukses: Operator={}, Agent={}", op_name, agent_name);
                    }
                } else {
                    tracing::info!("[Bootstrap] Distillation berjalan, profil belum lengkap: op_name='{}', agent_name='{}', agent_soul='{}'", op_name, agent_name, agent_soul);
                }
            }
            Err(e) => {
                tracing::error!("[Bootstrap] Gagal menjalankan distillation model: {e}");
            }
        }
    });

    Ok(())
}

/// Mengirimkan chat action status mengetik ("typing") ke chat Telegram tertentu.
async fn send_typing_action(token: &str, chat_id: &str) {
    let client = reqwest::Client::new();
    let url = format!("https://api.telegram.org/bot{}/sendChatAction", token);
    let payload = serde_json::json!({
        "chat_id": chat_id,
        "action": "typing"
    });
    let _ = client.post(&url).json(&payload).send().await;
}
