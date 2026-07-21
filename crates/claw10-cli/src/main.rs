use std::net::SocketAddr;
use std::sync::Arc;

use claw10_store::StoreExt;

use clap::{Parser, Subcommand};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Registry;

mod telemetry_layer;
mod service;
mod setup;
mod update;

/// Load environment variables from `~/.claw10/.env` if the file exists.
/// This makes API keys saved by the setup wizard available to the runtime
/// without requiring the user to manually export them.
fn load_claw10_env() {
    let env_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claw10")
        .join(".env");

    if env_path.exists() {
        match dotenvy::from_path(&env_path) {
            Ok(_) => tracing::debug!("loaded env from {}", env_path.display()),
            Err(e) => tracing::warn!("failed to load {}: {e}", env_path.display()),
        }
    }
}

fn get_default_db_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claw10")
        .join("db")
}

#[derive(Parser)]
#[command(
    name = "claw10",
    version = env!("CARGO_PKG_VERSION"),
    about = "Claw10 OS - Recursive Agent Swarm Operating System"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Start the API server
    Serve {
        /// Bind address
        #[arg(default_value = "0.0.0.0:3000")]
        bind: String,
        /// Path to database (in-memory if not set)
        #[arg(long)]
        db: Option<String>,
        /// Also start TUI in the same process to share the database
        #[arg(long)]
        tui: bool,
    },
    /// Start the TUI
    Tui {
        /// Path to database (in-memory if not set)
        #[arg(long)]
        db: Option<String>,
    },
    /// Run an agent objective directly from CLI with tools execution
    RunAgent {
        /// Path to database (in-memory if not set)
        #[arg(long)]
        db: Option<String>,
        /// The objective description for the agent
        #[arg(long)]
        objective: String,
        /// Optional model override
        #[arg(long)]
        model: Option<String>,
    },
    /// Print version
    Version,
    /// Initial setup wizard: create config file and workspace
    Setup {
        /// Force overwrite existing config
        #[arg(long)]
        force: bool,
    },
    /// Manage the Claw10 systemd user service (Linux)
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Start the Claw10 systemd user service daemon
    Start,
    /// Stop the Claw10 systemd user service daemon
    Stop,
    /// Uninstall Claw10 OS completely from this system
    Uninstall {
        /// Force skip confirmation prompt
        #[arg(long, short)]
        force: bool,
    },
    /// Update Claw10 OS to the latest version
    Update,
    /// Check if a newer version of Claw10 OS is available
    Check,
}

#[derive(Subcommand, Clone, Debug)]
enum ServiceAction {
    /// Install and enable the systemd user service
    Install,
    /// Uninstall and disable the systemd user service
    Uninstall,
    /// Start the systemd user service
    Start,
    /// Stop the systemd user service
    Stop,
    /// Show status of the systemd user service
    Status,
}

fn main() {
    let cli = Cli::parse();
    // When no subcommand is provided, start the API server in the background
    // and launch the TUI. This keeps the HTTP API and webhook endpoints
    // reachable while the user interacts with the terminal UI.
    let command = cli.command.unwrap_or(Commands::Serve {
        bind: "0.0.0.0:3000".into(),
        db: None,
        tui: true,
    });

    // Pre-fetch provider catalog before entering tokio runtime.
    // This avoids "Cannot start a runtime from within a runtime" panics.
    let needs_providers = matches!(
        &command,
        Commands::Serve { .. } | Commands::Tui { .. } | Commands::Setup { .. } | Commands::RunAgent { .. }
    );
    if needs_providers {
        claw10_model_router::providers::init_providers_sync();
    }

    let is_tui = match &command {
        Commands::Tui { .. } => true,
        Commands::Serve { tui, .. } => *tui,
        Commands::Setup { .. } => true,
        Commands::Service { .. } => false,
        Commands::Start | Commands::Stop => false,
        Commands::Uninstall { .. } => false,
        Commands::Update => false,
        Commands::Check => false,
        Commands::RunAgent { .. } => false,
        Commands::Version => false,
    };

    // Load local environment variables from ~/.claw10/.env so that API keys
    // written by the setup wizard are available to all subcommands.
    load_claw10_env();

    // Ensure logs directory exists
    let _ = std::fs::create_dir_all("logs");

    // Rolling file appender — daily rotation
    let file_appender = tracing_appender::rolling::daily("logs", "claw10.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Layer 1: file output (non-ANSI)
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    // Layer 2: stderr output (human-readable, ANSI)
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_ansi(true);

    // Layer 3: structured JSON telemetry log for Vector consumption.
    // Only captures events with target "claw10_telemetry".
    let telemetry_appender = tracing_appender::rolling::daily("logs", "claw10-telemetry.json");
    let (telemetry_non_blocking, _telemetry_guard) =
        tracing_appender::non_blocking(telemetry_appender);
    let telemetry_layer = telemetry_layer::TelemetryLayer::new(telemetry_non_blocking);

    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());

    if is_tui {
        Registry::default()
            .with(env_filter)
            .with(file_layer)
            .with(telemetry_layer)
            .init();
    } else {
        Registry::default()
            .with(env_filter)
            .with(file_layer)
            .with(stderr_layer)
            .with(telemetry_layer)
            .init();
    }

    // Auto-detect first-run: if no config exists and not setup/version, redirect to setup
    let needs_setup = match &command {
        Commands::Setup { .. } | Commands::Version | Commands::Start | Commands::Stop | Commands::Uninstall { .. } | Commands::Update | Commands::Check => false,
        _ => {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            let candidates = [
                std::path::PathBuf::from("claw10.toml"),
                std::path::PathBuf::from(&home).join(".config").join("claw10").join("config.toml"),
                std::path::PathBuf::from(&home).join(".claw10").join("config.toml"),
            ];
            !candidates.iter().any(|p| p.exists())
        }
    };

    // Create tokio runtime and run async body
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async move {
        if needs_setup {
            eprintln!("Belum ada konfigurasi ditemukan. Menjalankan setup wizard...\n");
            if let Err(e) = setup::run_setup_wizard(false).await {
                tracing::error!("Setup gagal: {e}");
                eprintln!("Setup gagal: {e}");
                std::process::exit(1);
            }
        }

        // Jalankan auto-update asinkron di background saat startup
        let cmd_clone = command.clone();
        tokio::spawn(async move {
            if !needs_setup && !is_tui && !matches!(cmd_clone, Commands::Uninstall { .. } | Commands::Setup { .. } | Commands::Version | Commands::Update | Commands::Check) {
                let _ = update::check_and_perform_update(true).await;
            }
        });

        // Jika user secara eksplisit memanggil `setup`, jalankan wizard lalu otomatis alihkan ke `serve` (auto run)
        if let Commands::Setup { force } = command {
            if let Err(e) = setup::run_setup_wizard(force).await {
                tracing::error!("Setup gagal: {e}");
                eprintln!("Setup gagal: {e}");
                std::process::exit(1);
            }
            println!("\nSetup sukses! Menginstal dan menjalankan Claw10 server daemon di background...");
            service::handle_service_command(service::ServiceAction::Install);
            service::handle_service_command(service::ServiceAction::Start);
            println!("\n✓ Claw10 server daemon aktif dan berjalan otomatis sebagai systemd user service!");
            println!("Gunakan 'claw10 service status' untuk memantau status service.");
            std::process::exit(0);
        }

    match command {
        Commands::Serve { bind, db, tui } => {
            let addr: SocketAddr = bind.parse().expect("invalid bind address");

            let db_path = match db {
                Some(path) => std::path::PathBuf::from(path),
                None => get_default_db_path(),
            };

            let kv_store: Arc<dyn claw10_store::Store> = {
                if let Some(parent) = db_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                tracing::info!("using JSON file store at {}", db_path.display());
                match claw10_store::JsonFileStore::new(&db_path) {
                    Ok(store) => Arc::new(store),
                    Err(e) => {
                        tracing::warn!("Gagal membuka store di '{}': {e}", db_path.display());
                        tracing::warn!("Menggunakan in-memory store sebagai fallback. Data tidak akan disimpan secara persisten.");
                        Arc::new(claw10_store::InMemoryStore::new())
                    }
                }
            };

            let mut registry = claw10_model_router::provider::ModelRegistry::new();

            // 1. Try config file (claw10.toml) for alias/custom providers
            if let Some(cfg) = claw10_model_router::config::discover_config() {
                let builtin = claw10_model_router::providers::provider_configs();

                // Pre-load KV store entries (sync adapter for closure)
                let mut kv_map: std::collections::HashMap<String, String> =
                    std::collections::HashMap::new();
                for slot in &builtin {
                    let store_key = format!("config:{}_api_key", slot.name);
                    if let Ok(Some(val)) = kv_store.get::<String>(&store_key).await {
                        let trimmed = val.trim().to_string();
                        if !trimmed.is_empty() {
                            kv_map.insert(store_key, trimmed);
                        }
                    }
                }

                let kv_get = |key: &str| kv_map.get(key).cloned();
                let (resolved, errors) =
                    claw10_model_router::config::resolve_providers(Some(&cfg), builtin, kv_get);
                for e in &errors {
                    tracing::warn!("config error: {e:?}");
                }
                for r in &resolved {
                    tracing::info!("registering provider: {} (from config)", r.name);
                }
                registry.register_resolved_providers(resolved);
            } else {
                // 2. Fallback: env var → KV store for every known provider
                for config in claw10_model_router::providers::provider_configs() {
                    let key = match std::env::var(&config.api_key_env) {
                        Ok(k) if !k.is_empty() => Some(k),
                        _ => {
                            let store_key = format!("config:{}_api_key", config.name);
                            match kv_store.get::<String>(&store_key).await {
                                Ok(Some(k)) if !k.trim().is_empty() => Some(k.trim().to_string()),
                                _ => None,
                            }
                        }
                    };
                    if let Some(key) = key {
                        tracing::info!("registering provider: {}", config.name);
                        registry.register(Box::new(
                            claw10_model_router::openai_compat::OpenAiCompatibleProvider::with_config(
                                &config.name,
                                &config.base_url,
                                key,
                                config.models.clone(),
                            ),
                        ));
                    }
                }
            }

            let model_router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));

            // Auto-fetch models secara asinkron di background untuk semua registered providers (modular)
            model_router.start_auto_fetch();

            // Register built-in tools
            let mut tool_registry = claw10_tool::registry::ToolRegistry::new();
            tool_registry.register(Box::new(claw10_tool::builtin::ShellTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::BrowserTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::WindowTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::ProcessTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::ScreenshotTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::InputTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::DeclareArtifactTool::new(Arc::clone(&kv_store))));
            let tool_registry = Arc::new(tool_registry);


            load_claw10_env();

            let state = claw10_control_api::AppState::new_with_services(
                Arc::clone(&kv_store),
                model_router,
                tool_registry,
            );
            let app = claw10_control_api::build_router(state);

            tracing::info!("Claw10 API server starting on {}", addr);
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(ref e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                    tracing::warn!("Port {} terpakai. Mencoba menghentikan server Claw10 lama...", addr);
                    
                    let port = addr.port();
                    let _ = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!("fuser -k {}/tcp || kill -9 $(lsof -t -i:{})", port, port))
                        .output();
                        
                    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;
                    
                    match tokio::net::TcpListener::bind(addr).await {
                        Ok(l) => {
                            tracing::info!("Berhasil mengambil alih port {}!", addr);
                            l
                        }
                        Err(err) => {
                            tracing::error!("Gagal melakukan bind ke {} meskipun telah mencoba membebaskan port: {err}", addr);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Gagal melakukan bind ke {}: {e}", addr);
                    std::process::exit(1);
                }
            };

            if tui {
                // Jalankan API server di background task
                tokio::spawn(async move {
                    if let Err(e) = axum::serve(listener, app).await {
                        tracing::error!("Server error: {e}");
                    }
                });
                // Jalankan TUI di thread utama
                if let Err(e) = claw10_tui::run_with_store(kv_store).await {
                    tracing::error!("TUI error: {e}");
                }
            } else {
                if let Err(e) = axum::serve(listener, app).await {
                    tracing::error!("Server error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Tui { db } => {
            let db_path = match db {
                Some(path) => std::path::PathBuf::from(path),
                None => get_default_db_path(),
            };

            let result = {
                if let Some(parent) = db_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match claw10_store::JsonFileStore::new(&db_path) {
                    Ok(store) => {
                        claw10_tui::run_with_store(Arc::new(store)).await
                    }
                    Err(e) => {
                        tracing::warn!("Gagal membuka store di '{}': {e}", db_path.display());
                        tracing::warn!("Menggunakan in-memory store sebagai fallback. Data tidak akan disimpan secara persisten.");
                        claw10_tui::run().await
                    }
                }
            };
            if let Err(e) = result {
                tracing::error!("TUI error: {e}");
            }
        }
        Commands::Version => {
            println!("Claw10 OS v{}", env!("CARGO_PKG_VERSION"));
        }
        Commands::RunAgent { db, objective, model } => {
            let db_path = match db {
                Some(path) => std::path::PathBuf::from(path),
                None => get_default_db_path(),
            };

            let kv_store: Arc<dyn claw10_store::Store> = {
                if let Some(parent) = db_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                tracing::info!("using JSON file store at {}", db_path.display());
                match claw10_store::JsonFileStore::new(&db_path) {
                    Ok(store) => Arc::new(store),
                    Err(e) => {
                        tracing::warn!("Gagal membuka store di '{}': {e}", db_path.display());
                        tracing::warn!("Menggunakan in-memory store sebagai fallback. Data tidak akan disimpan secara persisten.");
                        Arc::new(claw10_store::InMemoryStore::new())
                    }
                }
            };

            // Setup router & registry
            let mut registry = claw10_model_router::provider::ModelRegistry::new();
            if let Some(cfg) = claw10_model_router::config::discover_config() {
                let builtin = claw10_model_router::providers::provider_configs();
                let mut kv_map = std::collections::HashMap::new();
                for slot in &builtin {
                    let store_key = format!("config:{}_api_key", slot.name);
                    if let Ok(Some(val)) = kv_store.get::<String>(&store_key).await {
                        let trimmed = val.trim().to_string();
                        if !trimmed.is_empty() {
                            kv_map.insert(store_key, trimmed);
                        }
                    }
                }
                let kv_get = |key: &str| kv_map.get(key).cloned();
                let (resolved, _) = claw10_model_router::config::resolve_providers(Some(&cfg), builtin, kv_get);
                registry.register_resolved_providers(resolved);
            } else {
                for config in claw10_model_router::providers::provider_configs() {
                    let key = match std::env::var(&config.api_key_env) {
                        Ok(k) if !k.is_empty() => Some(k),
                        _ => {
                            let store_key = format!("config:{}_api_key", config.name);
                            match kv_store.get::<String>(&store_key).await {
                                Ok(Some(k)) if !k.trim().is_empty() => Some(k.trim().to_string()),
                                _ => None,
                            }
                        }
                    };
                    if let Some(key) = key {
                        registry.register(Box::new(
                            claw10_model_router::openai_compat::OpenAiCompatibleProvider::with_config(
                                &config.name,
                                &config.base_url,
                                key,
                                config.models.clone(),
                            ),
                        ));
                    }
                }
            }

            let model_router = Arc::new(claw10_model_router::router::ModelRouter::new(registry));

            // Setup tools
            let mut tool_registry = claw10_tool::registry::ToolRegistry::new();
            tool_registry.register(Box::new(claw10_tool::builtin::ShellTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::BrowserTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::WindowTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::ProcessTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::ScreenshotTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::InputTool::new()));
            tool_registry.register(Box::new(claw10_tool::builtin::DeclareArtifactTool::new(Arc::clone(&kv_store))));
            let tool_registry = Arc::new(tool_registry);


            // Services
            let worker_service = Arc::new(claw10_worker::WorkerService::new(Arc::clone(&kv_store)));
            let budget_service = Arc::new(claw10_budget::BudgetService);
            let agent_store = claw10_agent::store::AgentStore::new(Arc::clone(&kv_store));

            // Ensure minimal worker exists
            let worker_name = "cli-worker".to_string();
            let worker = worker_service.register(
                worker_name,
                claw10_domain::WorkerType::Local,
                vec![],
                "1.0.0".to_string(),
            ).await;
            let worker_id = worker.id.clone();

            // Dapatkan model aktif (override atau ambil dari registry)
            let active_model = match model {
                Some(m) => m,
                None => {
                    let profiles = model_router.registry().list_profiles();
                    if profiles.is_empty() {
                        eprintln!("Error: Tidak ada provider model LLM yang terkonfigurasi.");
                        eprintln!("Pasang API key terlebih dahulu menggunakan TUI (Ctrl+P -> Set API Key) atau via env var.");
                        std::process::exit(1);
                    }
                    profiles[0].id.clone()
                }
            };

            // Dapatkan atau buat default Agent
            let agent_id = match agent_store.list(claw10_agent::store::AgentQuery::default()).await {
                Ok(ref list) if !list.is_empty() => list[0].id.clone(),
                _ => {
                    // Create default agent
                    let now = chrono::Utc::now();
                    let new_agent = claw10_domain::Agent {
                        id: claw10_domain::AgentId(uuid::Uuid::now_v7()),
                        identity_id: claw10_domain::IdentityId(uuid::Uuid::now_v7()),
                        mission_id: claw10_domain::MissionId(uuid::Uuid::now_v7()),
                        parent_agent_id: None,
                        lineage_id: claw10_domain::LineageId(uuid::Uuid::now_v7()),
                        name: "cli-agent".into(),
                        role: "Specialist".into(),
                        genome: claw10_domain::AgentGenome {
                            id: "cli-genome".into(),
                            version: "1.0.0".into(),
                            role: "Specialist".into(),
                            lifecycle_modes: vec![claw10_domain::LifecycleMode::Ephemeral],
                            model_policy: claw10_domain::ModelPolicy {
                                preferred_profile: active_model.clone(),
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
                            name: "default".into(),
                            version: "1.0.0".into(),
                            rules: vec![],
                            is_active: true,
                            signed_by: None,
                            signature: None,
                            activated_at: None,
                            created_at: now,
                        },
                        turn_count: 0,
                        total_cost_usd: 0.0,
                        created_at: now,
                        updated_at: now,
                        terminated_at: None,
                    };
                    let id = new_agent.id.clone();
                    if let Err(e) = agent_store.save(&new_agent).await {
                        tracing::error!("Gagal menyimpan agent default: {e}");
                        std::process::exit(1);
                    }
                    id
                }
            };

            // Inisialisasi runtime
            let runtime = claw10_agent::runtime::AgentRuntime::new(
                agent_store,
                model_router,
                tool_registry,
                budget_service,
                worker_service,
                Some(worker_id),
            );

            println!("=== Claw10 Agent CLI Executor ===");
            println!("Objective: \"{}\"", objective);
            println!("Model: {}", active_model);
            println!("Memulai eksekusi agent...");

            let mut context = std::collections::HashMap::new();
            context.insert("mission_statement".to_string(), "CLI SWARM EXECUTION".to_string());

            match runtime.execute_agent(&agent_id, objective, context, None, None).await {

                Ok((session, events)) => {
                    println!("\n--- Eksekusi Selesai ---");
                    for event in events {
                        match event {
                            claw10_agent::events::AgentEvent::Thought { content, .. } => {
                                println!("\n[Thought]\n{}", content.trim());
                            }
                            claw10_agent::events::AgentEvent::ModelCall { tokens, cost, .. } => {
                                println!("[Model Call] Tokens used: {} | Cost: ${:.5}", tokens, cost);
                            }
                            claw10_agent::events::AgentEvent::ToolCall { tool, args, result } => {
                                println!("[Tool Call] Running tool '{}' with args: {}", tool, args);
                                println!("[Tool Response] Output:\n{}", result);
                            }
                            claw10_agent::events::AgentEvent::ObjectiveComplete { summary, evidence } => {
                                println!("\n[Objective Complete]");
                                println!("Ringkasan: {}", summary);
                                println!("Bukti (Evidence): {:?}", evidence);
                            }
                            claw10_agent::events::AgentEvent::Error { message } => {
                                println!("[Error Event] {}", message);
                            }
                            _ => {}
                        }
                    }
                    println!("\n--- Ringkasan Sesi ---");
                    println!("Status Sesi: {:?}", session.state);
                    println!("Total Turn: {}", session.turn_count);
                    println!("Total Tokens: {}", session.total_tokens);
                    println!("Estimasi Biaya: ${:.5}", session.total_cost_usd);
                }
                Err(e) => {
                    tracing::error!("Eksekusi agent gagal: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Setup { force } => {
            if let Err(e) = setup::run_setup_wizard(force).await {
                tracing::error!("Setup gagal: {e}");
                std::process::exit(1);
            }
        }
        Commands::Service { action } => {
            let act = match action {
                ServiceAction::Install => service::ServiceAction::Install,
                ServiceAction::Uninstall => service::ServiceAction::Uninstall,
                ServiceAction::Start => service::ServiceAction::Start,
                ServiceAction::Stop => service::ServiceAction::Stop,
                ServiceAction::Status => service::ServiceAction::Status,
            };
            service::handle_service_command(act);
        }
        Commands::Start => {
            service::handle_service_command(service::ServiceAction::Start);
        }
        Commands::Stop => {
            service::handle_service_command(service::ServiceAction::Stop);
        }
        Commands::Update => {
            if let Err(e) = update::check_and_perform_update(false).await {
                tracing::error!("Update gagal: {e}");
                std::process::exit(1);
            }
        }
        Commands::Check => {
            if let Err(e) = update::check_version_only().await {
                tracing::error!("Gagal memeriksa versi: {e}");
                std::process::exit(1);
            }
        }
        Commands::Uninstall { force } => {
            if let Err(e) = setup::perform_uninstall(force).await {
                tracing::error!("Uninstall gagal: {e}");
                std::process::exit(1);
            }
        }
    }
    });
}


