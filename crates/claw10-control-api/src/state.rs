use std::sync::Arc;

use claw10_agent::AgentStore;
use claw10_domain::SwarmLimitsConfig;
use claw10_event::{EventBus, InMemoryEventBus};
use claw10_gateway::GatewayService;
use claw10_memory::MemoryService;
use claw10_model_router::router::ModelRouter;
use claw10_scheduler::ScheduleService;
use claw10_spawn::broker::SpawnBroker;
use claw10_store::{InMemoryStore as KvInMemory, Store, StoreExt};
use claw10_telemetry::TelemetryService;
use claw10_tool::registry::ToolRegistry;
use claw10_worker::WorkerService;

pub use crate::store::*;


#[derive(Clone)]
pub struct AppState {
    pub scheduler_service: Arc<ScheduleService>,
    pub worker_service: Arc<WorkerService>,
    pub memory_service: Arc<MemoryService>,
    pub gateway_service: Arc<GatewayService>,
    pub skill_service: Arc<claw10_skill::SkillService>,
    pub artifact_service: Arc<claw10_artifact::ArtifactService>,
    pub spawn_broker: Arc<SpawnBroker>,
    pub event_bus: Arc<dyn EventBus>,
    pub telemetry: TelemetryService,
    pub kv_store: Arc<dyn Store>,
    pub model_router: Option<Arc<ModelRouter>>,
    pub tool_registry: Option<Arc<ToolRegistry>>,
}

impl AppState {
    /// Create AppState dengan in-memory KV store.
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_store(Arc::new(KvInMemory::new()))
    }

    /// Create AppState dengan shared KV store (untuk prod dengan sled).
    #[must_use]
    pub fn new_with_store(kv_store: Arc<dyn Store>) -> Self {
        let limits = SwarmLimitsConfig {
            max_spawn_depth: 5,
            max_children_per_agent: 10,
            max_agents_per_mission: 100,
            max_concurrent_agents: 50,
            max_persistent_children_per_agent: 5,
            max_turns_per_ephemeral_agent: 100,
            max_idle_seconds_ephemeral: 600,
        };

        // AgentStore menggunakan KV store yang sama
        let agent_store = Arc::new(AgentStore::new(Arc::clone(&kv_store)));

        // Event bus: NATS jika env NATS_URL di-set, fallback ke InMemory
        let event_bus: Arc<dyn EventBus> = create_event_bus();
        start_event_subscribers(Arc::clone(&event_bus));



        Self {
            scheduler_service: Arc::new(ScheduleService::new(Arc::clone(&kv_store))),
            worker_service: Arc::new(WorkerService::new(Arc::clone(&kv_store))),
            memory_service: Arc::new(MemoryService::new(Arc::clone(&kv_store))),
            gateway_service: Arc::new(GatewayService::new(Arc::clone(&kv_store))),
            skill_service: Arc::new(claw10_skill::SkillService::new(Arc::clone(&kv_store))),
            artifact_service: Arc::new(claw10_artifact::ArtifactService::new(Arc::clone(&kv_store))),
            spawn_broker: Arc::new(SpawnBroker::new(limits, agent_store, Arc::clone(&event_bus))),
            event_bus,
            telemetry: TelemetryService::default(),
            kv_store,
            model_router: None,
            tool_registry: None,
        }
    }

    /// Create AppState dengan model router dan tool registry untuk agent execution.
    #[must_use]
    pub fn new_with_services(
        kv_store: Arc<dyn Store>,
        model_router: Arc<ModelRouter>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        let mut state = Self::new_with_store(Arc::clone(&kv_store));
        state.model_router = Some(Arc::clone(&model_router));
        state.tool_registry = Some(Arc::clone(&tool_registry));
        start_background_scheduler(
            Arc::clone(&state.scheduler_service),
            Arc::clone(&kv_store),
            Some(Arc::clone(&model_router)),
            Some(Arc::clone(&tool_registry)),
            Arc::clone(&state.worker_service),
        );
        // Mulai background polling getUpdates Telegram jika token dikonfigurasi
        crate::telegram_poller::start_telegram_poller(state.clone());
        auto_register_telegram_if_needed(Arc::clone(&state.kv_store), Arc::clone(&state.gateway_service), Arc::clone(&model_router));
        state
    }
}

/// Auto-register Telegram bot jika diset di env
fn auto_register_telegram_if_needed(
    kv_store: Arc<dyn Store>,
    gateway_service: Arc<claw10_gateway::GatewayService>,
    model_router: Arc<ModelRouter>,
) {
    if let Ok(token) = std::env::var("TELEGRAM_BOT_TOKEN") {
        if token.trim().is_empty() {
            return;
        }
        let store_clone = Arc::clone(&kv_store);
        let gateway_clone = Arc::clone(&gateway_service);
        let router_clone = Arc::clone(&model_router);
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            if let Ok(mut agents) = store_clone.scan_prefix::<claw10_domain::Agent>("agent:").await {
                let agent = if agents.is_empty() {
                    let preferred_model = get_preferred_model_from_config(&router_clone);
                    tracing::info!("Menggunakan model profile untuk default agent: {preferred_model}");

                    // Jika kosong, buat default agent
                    let new_agent = claw10_domain::Agent {
                        id: claw10_domain::AgentId(uuid::Uuid::now_v7()),
                        identity_id: claw10_domain::IdentityId(uuid::Uuid::now_v7()),
                        mission_id: claw10_domain::MissionId(uuid::Uuid::now_v7()),
                        parent_agent_id: None,
                        lineage_id: claw10_domain::LineageId(uuid::Uuid::now_v7()),
                        name: "default-agent".into(),
                        role: "Assistant".into(),
                        genome: claw10_domain::AgentGenome {
                            id: "default-genome".into(),
                            version: "1.0.0".into(),
                            role: "Assistant".into(),
                            lifecycle_modes: vec![claw10_domain::LifecycleMode::Ephemeral],
                            model_policy: claw10_domain::ModelPolicy {
                                preferred_profile: preferred_model,
                                fallback_profiles: vec![],
                                max_context_tokens: 128_000,
                            },
                            autonomy: claw10_domain::AutonomyConfig {
                                can_spawn: false,
                                max_spawn_depth: 0,
                                max_children: 0,
                            },
                            delegable_permissions: vec![],
                            non_delegable_permissions: vec![],
                            memory: claw10_domain::MemoryConfig {
                                default_read_scopes: vec![],
                                default_write_scope: None,
                            },
                            runtime: claw10_domain::RuntimeConfig {
                                preferred_class: "local".into(),
                                network: claw10_domain::NetworkPolicy::AllowByDefault,
                            },
                            verification_required: false,
                        },
                        state: claw10_domain::AgentState::Ready,
                        lifecycle_mode: claw10_domain::LifecycleMode::Ephemeral,
                        persistent_pattern: None,
                        budget: claw10_domain::Budget {
                            allocated_usd: 100.0,
                            spent_usd: 0.0,
                            soft_limit_usd: None,
                            hard_limit_usd: Some(100.0),
                            recurring_monthly_usd: None,
                        },
                        delegable_permissions: vec![],
                        non_delegable_permissions: vec![],
                        current_runtime: None,
                        checkpoints: vec![],
                        subscriptions: vec![],
                        schedules: vec![],
                        policy_bundle: claw10_domain::PolicyBundle {
                            id: claw10_domain::PolicyBundleId(uuid::Uuid::now_v7()),
                            name: "default-policy".into(),
                            version: "1.0.0".into(),
                            rules: vec![claw10_domain::PolicyRule {
                                id: claw10_domain::PolicyRuleId(uuid::Uuid::now_v7()),
                                subject: claw10_domain::PolicySubject::Role("*".into()),
                                effect: claw10_domain::PolicyEffect::Allow,
                                action: "*".into(),
                                resource: "*".into(),
                                condition: None,
                                priority: 1,
                            }],
                            is_active: true,
                            signed_by: None,
                            signature: None,
                            activated_at: Some(chrono::Utc::now()),
                            created_at: chrono::Utc::now(),
                        },
                        turn_count: 0,
                        total_cost_usd: 0.0,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                        terminated_at: None,
                    };
                    let agent_key = format!("agent:{}", new_agent.id.0);
                    if let Err(e) = store_clone.set(&agent_key, &new_agent).await {
                        tracing::error!("Gagal membuat default agent: {e}");
                        return;
                    }
                    tracing::info!("Created default agent: {}", new_agent.id.0);
                    new_agent
                } else {
                    agents.remove(0).1
                };

                let mut exists = false;
                if let Ok(channels) = store_clone.scan_prefix::<claw10_domain::Channel>("gateway:channel:").await {
                    for (_, ch) in channels {
                        if ch.channel_type == claw10_domain::ChannelType::Telegram
                            && let Some(bot_token) = ch.config.get("bot_token").and_then(|v| v.as_str())
                                && bot_token == token {
                                    exists = true;
                                    break;
                                }
                    }
                }
                if !exists {
                    let chat_id = std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
                    let config = serde_json::json!({
                        "bot_token": token,
                        "chat_id": chat_id,
                        "agent_id": agent.id.0.to_string(),
                    });
                    let channel = gateway_clone.register_channel(claw10_domain::ChannelType::Telegram, config).await;
                    tracing::info!("Auto-registered Telegram bot channel ID: {} with Chat ID: {}", channel.id, chat_id);
                }
            }
        });
    }
}

fn get_preferred_model_from_config(model_router: &ModelRouter) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/rasyiqi".to_string());
    let base_path = std::path::PathBuf::from(home);

    // 1. Coba baca dari config.toml
    let mut config_path = base_path.clone();
    config_path.push(".claw10");
    config_path.push("config.toml");
    
    if config_path.exists()
        && let Ok(content) = std::fs::read_to_string(&config_path) {
            let mut in_alias_default = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed == "[alias.default]" {
                    in_alias_default = true;
                    continue;
                }
                if trimmed.starts_with('[') {
                    in_alias_default = false;
                }
                if in_alias_default && trimmed.starts_with("model")
                    && let Some(val) = trimmed.split('=').nth(1) {
                        let model = val.trim().trim_matches('"').trim_matches('\'').to_string();
                        if !model.is_empty() {
                            return model;
                        }
                    }
            }
        }

    // 2. Jika tidak ada di config.toml, ambil model terdaftar pertama dari router registry
    let profiles = model_router.registry().list_profiles();
    if let Some(first_profile) = profiles.first() {
        return first_profile.id.clone();
    }

    // 3. Fallback jika tidak ada profile yang terdaftar sama sekali
    "openai/gpt-4o".to_string()
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Membuat event bus berdasarkan konfigurasi lingkungan.
/// Jika env `NATS_URL` di-set, gunakan `NatsEventBus`, fallback ke `InMemoryEventBus`.
fn create_event_bus() -> Arc<dyn EventBus> {
    let nats_url = std::env::var("NATS_URL");
    if let Ok(url) = nats_url {
        #[cfg(feature = "nats")]
        {
            match claw10_event::NatsEventBus::new(&url) {
                Ok(bus) => {
                    tracing::info!("Menggunakan NatsEventBus dengan NATS_URL={url}");
                    return Arc::new(bus);
                }
                Err(e) => {
                    tracing::warn!("Gagal konek NATS ({url}), fallback ke InMemoryEventBus: {e}");
                }
            }
        }
        #[cfg(not(feature = "nats"))]
        {
            tracing::warn!("NATS_URL={url} di-set tapi fitur `nats` tidak aktif. Aktifkan dengan `--features nats`. Fallback ke InMemoryEventBus.");
        }
    }
    Arc::new(InMemoryEventBus::new())
}

/// Spawn a background task yang poll schedule setiap 30 detik dan
/// benar-benar mengeksekusi agent via AgentRuntime saat schedule due.
pub fn start_background_scheduler(
    scheduler_service: Arc<ScheduleService>,
    kv_store: Arc<dyn Store>,
    model_router: Option<Arc<claw10_model_router::router::ModelRouter>>,
    tool_registry: Option<Arc<claw10_tool::registry::ToolRegistry>>,
    worker_service: Arc<claw10_worker::WorkerService>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let now = chrono::Utc::now();

            // ── 1. Liveness check: Deteksi stale worker (offline) & hibernate agent-nya ──
            if let Ok(stale_workers) = worker_service.detect_stale(30).await {
                for worker in stale_workers {
                    tracing::warn!(
                        "Liveness Daemon: Worker {} (name: {}) stale. Menandai Offline.",
                        worker.id.0,
                        worker.name
                    );
                    let _ = worker_service.mark_offline(&worker.id).await;

                    // Cari agent yang terikat pada worker yang mati ini, lalu hibernate
                    if let Ok(all_agents) = kv_store.scan_prefix::<claw10_domain::Agent>("agent:").await {
                        for (_, mut agent) in all_agents {
                            if agent.state == claw10_domain::AgentState::Active
                                && let Some(ref lease) = agent.current_runtime
                                    && lease.worker_id == worker.id.0.to_string() {
                                        tracing::warn!(
                                            "Liveness Daemon: Agent {} terikat pada worker {} yang mati. Hibernating agent...",
                                            agent.id.0,
                                            worker.id.0
                                        );
                                        if let Ok(checkpoint) = claw10_lifecycle::LifecycleService::hibernate(&mut agent) {
                                            let _ = kv_store.set(&format!("agent:{}", agent.id.0), &agent).await;
                                            let _ = kv_store.set(&format!("checkpoint:{}", checkpoint.id.0), &checkpoint).await;
                                        }
                                    }
                        }
                    }
                }
            }

            // ── 2. Liveness check: Deteksi agent stale (lease expired) & hibernate ──
            if let Ok(all_agents) = kv_store.scan_prefix::<claw10_domain::Agent>("agent:").await {
                let agent_list: Vec<claw10_domain::Agent> = all_agents.into_iter().map(|(_, a)| a).collect();
                let stale_agents = claw10_lifecycle::LifecycleService::detect_stale(&agent_list, 60);

                for agent in stale_agents {
                    tracing::warn!(
                        "Liveness Daemon: Agent {} runtime lease expired. Hibernating...",
                        agent.id.0
                    );
                    let mut agent_to_hibernate = agent.clone();
                    if let Ok(checkpoint) = claw10_lifecycle::LifecycleService::hibernate(&mut agent_to_hibernate) {
                        let _ = kv_store.set(&format!("agent:{}", agent_to_hibernate.id.0), &agent_to_hibernate).await;
                        let _ = kv_store.set(&format!("checkpoint:{}", checkpoint.id.0), &checkpoint).await;
                    }
                }
            }

            // ── 3. Cron Schedules Trigger ──
            match scheduler_service.get_due_schedules(&now).await {
                Ok(due) => {
                    for ds in &due {
                        tracing::info!(
                            "Scheduler: agent {} due (action: {:?}, cron: {})",
                            ds.agent_id.0,
                            ds.schedule.action,
                            ds.schedule.cron,
                        );

                        let should_execute = matches!(
                            ds.schedule.action,
                            claw10_domain::ScheduleAction::Wake
                                | claw10_domain::ScheduleAction::Review
                        );

                        if !should_execute {
                            continue;
                        }

                        // Bangun AgentRuntime dan eksekusi agent
                        let Some(ref mr) = model_router else {
                            tracing::warn!("Scheduler: model_router belum dikonfigurasi, skip");
                            continue;
                        };
                        let Some(ref tr) = tool_registry else {
                            tracing::warn!("Scheduler: tool_registry belum dikonfigurasi, skip");
                            continue;
                        };

                        let agent_id = ds.agent_id.clone();
                        let action = format!("{:?}", ds.schedule.action);
                        let store_clone = Arc::clone(&kv_store);
                        let mr_clone = Arc::clone(mr);
                        let tr_clone = Arc::clone(tr);
                        let ws_clone = Arc::clone(&worker_service);

                        tokio::spawn(async move {
                            let agent_store =
                                claw10_agent::AgentStore::new(Arc::clone(&store_clone));
                            let budget_service = Arc::new(claw10_budget::BudgetService);

                            // Daftarkan worker ephemeral untuk sesi ini
                            let worker = ws_clone
                                .register(
                                    format!("scheduler-{}", agent_id.0),
                                    claw10_domain::WorkerType::Local,
                                    vec![],
                                    "1.0.0".to_string(),
                                )
                                .await;

                            let runtime = claw10_agent::AgentRuntime::new(
                                agent_store,
                                mr_clone,
                                tr_clone,
                                budget_service,
                                ws_clone,
                                Some(worker.id),
                            );

                            let objective =
                                format!("Scheduled {action}: review state and take action");
                            match runtime
                                .execute_agent(&agent_id, objective, Default::default(), None, None)
                                .await

                            {
                                Ok((session, _)) => {
                                    tracing::info!(
                                        "Scheduler: agent {} selesai ({:?})",
                                        agent_id.0,
                                        session.state
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Scheduler: gagal eksekusi agent {}: {e}",
                                        agent_id.0
                                    );
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("Scheduler poll error: {e}");
                }
            }
        }
    });
}


/// Men-subscribe event dari bus untuk mendemonstrasikan reaktivitas asinkron.
fn start_event_subscribers(event_bus: Arc<dyn EventBus>) {
    tokio::spawn(async move {
        let sub_res = event_bus
            .subscribe(
                "claw10.agent.>",
                Arc::new(|event| {
                    Box::pin(async move {
                        tracing::info!(
                            "Event Subscriber: Menerima event domain: {:?}",
                            event.subject()
                        );
                    })
                }),
            )
            .await;

        match sub_res {
            Ok(sub_id) => {
                tracing::info!(
                    "Event Subscriber: Berhasil berlangganan ke event bus (Sub ID: {})",
                    sub_id.0
                );
            }
            Err(e) => {
                tracing::warn!("Event Subscriber: Gagal berlangganan ke event bus: {e}");
            }
        }
    });
}


