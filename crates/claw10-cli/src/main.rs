use std::net::SocketAddr;
use std::sync::Arc;

use claw10_store::StoreExt;

use clap::{Parser, Subcommand};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Registry;

mod telemetry_layer;

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

#[derive(Parser)]
#[command(
    name = "claw10",
    about = "Claw10 OS - Recursive Agent Swarm Operating System"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the API server
    Serve {
        /// Bind address
        #[arg(default_value = "0.0.0.0:3000")]
        bind: String,
        /// Path to sled database (in-memory if not set)
        #[arg(long)]
        db: Option<String>,
        /// Also start TUI in the same process to share the database
        #[arg(long)]
        tui: bool,
    },
    /// Start the TUI
    Tui {
        /// Path to sled database (in-memory if not set)
        #[arg(long)]
        db: Option<String>,
    },
    /// Run an agent objective directly from CLI with tools execution
    RunAgent {
        /// Path to sled database (in-memory if not set)
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
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    // When no subcommand is provided, start the API server in the background
    // and launch the TUI. This keeps the HTTP API and webhook endpoints
    // reachable while the user interacts with the terminal UI.
    let mut command = cli.command.unwrap_or(Commands::Serve {
        bind: "0.0.0.0:3000".into(),
        db: None,
        tui: true,
    });
    let is_tui = match &command {
        Commands::Tui { .. } => true,
        Commands::Serve { tui, .. } => *tui,
        Commands::Setup { .. } => true,
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
        Commands::Setup { .. } | Commands::Version => false,
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

    if needs_setup {
        eprintln!("Belum ada konfigurasi ditemukan. Menjalankan setup wizard...\n");
        if let Err(e) = run_setup_wizard(false).await {
            eprintln!("Setup gagal: {e}");
            std::process::exit(1);
        }
    }

    // Jika user secara eksplisit memanggil `setup`, jalankan wizard lalu otomatis alihkan ke `serve` (auto run)
    if let Commands::Setup { force } = command {
        if let Err(e) = run_setup_wizard(force).await {
            eprintln!("Setup gagal: {e}");
            std::process::exit(1);
        }
        println!("\nSetup sukses! Menjalankan Claw10 server & TUI otomatis...");
        command = Commands::Serve {
            bind: "0.0.0.0:3000".into(),
            db: None,
            tui: true,
        };
    }

    match command {
        Commands::Serve { bind, db, tui } => {
            let addr: SocketAddr = bind.parse().expect("invalid bind address");

            let kv_store: Arc<dyn claw10_store::Store> = match db {
                Some(path) => {
                    tracing::info!("using sled database at {path}");
                    match claw10_store::SledStore::new(&path) {
                        Ok(store) => Arc::new(store),
                        Err(e) => {
                            eprintln!("Error: Gagal membuka database sled di '{path}'.");
                            eprintln!("Detail: {e}");
                            eprintln!("Pastikan tidak ada proses Claw10 server atau TUI lain yang sedang berjalan menggunakan database ini.");
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    tracing::info!("using in-memory store");
                    Arc::new(claw10_store::InMemoryStore::new())
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
                    // Native providers (e.g. Bedrock) are registered via their factory.
                    if let Some(factory) = config.factory {
                        let name = config.name.to_string();
                        if !registry.list_providers().contains(&name) {
                            tracing::info!("registering native provider: {}", name);
                            registry.register(factory());
                        }
                        continue;
                    }

                    let key = match std::env::var(config.api_key_env) {
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
                                config.name,
                                config.base_url,
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
            tool_registry.register(Box::new(claw10_tool::builtin::ShellTool));
            tool_registry.register(Box::new(claw10_tool::builtin::ReadFileTool));
            tool_registry.register(Box::new(claw10_tool::builtin::WriteFileTool));
            tool_registry.register(Box::new(claw10_tool::builtin::HttpTool));
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
                            eprintln!("Error: Gagal melakukan bind ke {} meskipun telah mencoba membebaskan port.", addr);
                            eprintln!("Detail: {err}");
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: Gagal melakukan bind ke {}: {}", addr, e);
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
                axum::serve(listener, app).await.unwrap();
            }
        }
        Commands::Tui { db } => {
            let result = match db {
                Some(path) => {
                    match claw10_store::SledStore::new(&path) {
                        Ok(store) => {
                            claw10_tui::run_with_store(Arc::new(store)).await
                        }
                        Err(e) => {
                            eprintln!("Error: Gagal membuka database sled di '{path}'.");
                            eprintln!("Detail: {e}");
                            eprintln!("Pastikan tidak ada proses Claw10 server atau TUI lain yang sedang berjalan menggunakan database ini.");
                            std::process::exit(1);
                        }
                    }
                }
                None => claw10_tui::run().await,
            };
            if let Err(e) = result {
                tracing::error!("TUI error: {e}");
            }
        }
        Commands::Version => {
            println!("Claw10 OS v{}", env!("CARGO_PKG_VERSION"));
        }
        Commands::RunAgent { db, objective, model } => {
            let kv_store: Arc<dyn claw10_store::Store> = match db {
                Some(path) => {
                    tracing::info!("using sled database at {path}");
                    match claw10_store::SledStore::new(&path) {
                        Ok(store) => Arc::new(store),
                        Err(e) => {
                            eprintln!("Error: Gagal membuka database sled di '{path}'.");
                            eprintln!("Detail: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    tracing::info!("using in-memory store");
                    Arc::new(claw10_store::InMemoryStore::new())
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
                    // Native providers (e.g. Bedrock) are registered via their factory.
                    if let Some(factory) = config.factory {
                        let name = config.name.to_string();
                        if !registry.list_providers().contains(&name) {
                            registry.register(factory());
                        }
                        continue;
                    }

                    let key = match std::env::var(config.api_key_env) {
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
                                config.name,
                                config.base_url,
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
            tool_registry.register(Box::new(claw10_tool::builtin::ShellTool));
            tool_registry.register(Box::new(claw10_tool::builtin::ReadFileTool));
            tool_registry.register(Box::new(claw10_tool::builtin::WriteFileTool));
            tool_registry.register(Box::new(claw10_tool::builtin::HttpTool));
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
                        eprintln!("Error: Gagal menyimpan agent default: {e}");
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
                    eprintln!("\nError Eksekusi Agent: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Setup { force } => {
            if let Err(e) = run_setup_wizard(force).await {
                eprintln!("Setup failed: {e}");
                std::process::exit(1);
            }
        }
    }
}

async fn run_setup_wizard(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let default_path = std::path::PathBuf::from(&home).join(".claw10").join("config.toml");
    let local_path = std::path::PathBuf::from("claw10.toml");

    let target_path = if local_path.exists() {
        local_path
    } else if default_path.exists() && !force {
        default_path
    } else {
        default_path
    };

    let mut wizard = claw10_tui::SetupWizard::new(target_path);
    wizard.run()?;
    Ok(())
}
