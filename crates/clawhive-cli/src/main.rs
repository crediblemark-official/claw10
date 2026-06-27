use std::net::SocketAddr;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clawhive", about = "ClawHive OS - Recursive Agent Swarm Operating System")]
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
    },
    /// Print version
    Version,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { bind } => {
            let addr: SocketAddr = bind.parse().expect("invalid bind address");
            let state = clawhive_control_api::AppState::new();
            let app = clawhive_control_api::build_router(state);

            tracing::info!("ClawHive API server starting on {}", addr);
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        }
        Commands::Version => {
            println!("ClawHive OS v{}", env!("CARGO_PKG_VERSION"));
        }
    }
}
