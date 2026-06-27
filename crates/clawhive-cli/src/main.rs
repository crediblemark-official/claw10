use std::net::SocketAddr;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Registry;

#[derive(Parser)]
#[command(
    name = "clawhive",
    about = "ClawHive OS - Recursive Agent Swarm Operating System"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
    /// Print version
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let is_tui = match &cli.command {
        Commands::Tui { .. } => true,
        Commands::Serve { tui, .. } => *tui,
        Commands::Version => false,
    };

    // Ensure logs directory exists
    let _ = std::fs::create_dir_all("logs");

    // Rolling file appender — daily rotation
    let file_appender = tracing_appender::rolling::daily("logs", "clawhive.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Layer 1: file output (non-ANSI)
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    // Layer 2: stderr output (human-readable, ANSI)
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_ansi(true);

    let env_filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());

    if is_tui {
        Registry::default()
            .with(env_filter)
            .with(file_layer)
            .init();
    } else {
        Registry::default()
            .with(env_filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    }

    match cli.command {
        Commands::Serve { bind, db, tui } => {
            let addr: SocketAddr = bind.parse().expect("invalid bind address");

            let kv_store: Arc<dyn clawhive_store::Store> = match db {
                Some(path) => {
                    tracing::info!("using sled database at {path}");
                    match clawhive_store::SledStore::new(&path) {
                        Ok(store) => Arc::new(store),
                        Err(e) => {
                            eprintln!("Error: Gagal membuka database sled di '{path}'.");
                            eprintln!("Detail: {e}");
                            eprintln!("Pastikan tidak ada proses ClawHive server atau TUI lain yang sedang berjalan menggunakan database ini.");
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    tracing::info!("using in-memory store");
                    Arc::new(clawhive_store::InMemoryStore::new())
                }
            };

            let state = clawhive_control_api::AppState::new_with_store(Arc::clone(&kv_store));
            let app = clawhive_control_api::build_router(state);

            tracing::info!("ClawHive API server starting on {}", addr);
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

            if tui {
                // Jalankan API server di background task
                tokio::spawn(async move {
                    if let Err(e) = axum::serve(listener, app).await {
                        tracing::error!("Server error: {e}");
                    }
                });
                // Jalankan TUI di thread utama
                if let Err(e) = clawhive_tui::run_with_store(kv_store).await {
                    tracing::error!("TUI error: {e}");
                }
            } else {
                axum::serve(listener, app).await.unwrap();
            }
        }
        Commands::Tui { db } => {
            let result = match db {
                Some(path) => {
                    match clawhive_store::SledStore::new(&path) {
                        Ok(store) => {
                            clawhive_tui::run_with_store(Arc::new(store)).await
                        }
                        Err(e) => {
                            eprintln!("Error: Gagal membuka database sled di '{path}'.");
                            eprintln!("Detail: {e}");
                            eprintln!("Pastikan tidak ada proses ClawHive server atau TUI lain yang sedang berjalan menggunakan database ini.");
                            std::process::exit(1);
                        }
                    }
                }
                None => clawhive_tui::run().await,
            };
            if let Err(e) = result {
                tracing::error!("TUI error: {e}");
            }
        }
        Commands::Version => {
            println!("ClawHive OS v{}", env!("CARGO_PKG_VERSION"));
        }
    }
}
