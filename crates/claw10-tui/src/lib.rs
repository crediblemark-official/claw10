mod app;
pub mod setup_service;
pub(crate) mod setup_wizard;
pub(crate) mod spawn_tool;
pub(crate) mod tui_agent;
mod ui;

pub use setup_wizard::SetupWizard;

use std::sync::Arc;

use claw10_control_api::state::AppState;
use claw10_store::Store;

pub use app::TuiApp;

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("Terminal initialization failed: {0}")]
    TermInit(String),
    #[error("Runtime error: {0}")]
    Runtime(String),
}

/// Run the TUI application with an in-memory store.
pub async fn run() -> Result<(), TuiError> {
    let state = AppState::new();
    let mut app = TuiApp::new(state);
    app.run().await
}

/// Run the TUI application with a shared KV store (sled).
pub async fn run_with_store(kv_store: Arc<dyn Store>) -> Result<(), TuiError> {
    let state = AppState::new_with_store(kv_store);
    let mut app = TuiApp::new(state);
    app.run().await
}
