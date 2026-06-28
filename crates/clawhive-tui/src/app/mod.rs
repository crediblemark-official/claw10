use crossterm::event::Event;

use clawhive_control_api::state::AppState;
use clawhive_domain::{Agent, SpawnRequest, Worker};
use clawhive_model_router::types::{ModelFamily, ModelProfile, StreamEvent};

mod commands;
mod events;
mod model_selection;
pub(crate) mod palette;

pub use palette::get_palette_items;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Session,
    Agents,
    Workers,
    SpawnRequests,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Chat,
}

pub enum InputMode {
    Normal,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelectionStep {
    SelectProvider,
    SelectFamily,
    SelectVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandMode {
    None,
    CommandPalette {
        search_query: String,
        selected_index: usize,
        filtered_items: Vec<(String, String, String, String)>, // (category, name, shortcut, action)
    },
    ApiKeyInput {
        key_input: String,
        error_message: String,
    },
    ModelSelection,
}

pub struct TuiApp {
    pub state: AppState,
    pub agents: Vec<Agent>,
    pub workers: Vec<Worker>,
    pub spawn_requests: Vec<SpawnRequest>,
    pub selected_index: usize,
    pub selected_tab: Tab,
    pub should_quit: bool,
    pub status_message: String,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub active_screen: Screen,
    pub chat_history: Vec<(String, String, String)>, // (sender, role/model, message)
    pub command_mode: CommandMode,
    pub active_model: String,
    // Model selection flow state
    pub model_sel_step: ModelSelectionStep,
    pub model_sel_provider: String,
    pub model_sel_families: Vec<ModelFamily>,
    pub model_sel_variants: Vec<ModelProfile>,
    pub model_sel_search: String,
    pub model_sel_index: usize,
    /// If set, model selection was interrupted to ask for API key.
    /// After key is set, resume selecting models for this provider.
    pub model_sel_pending_provider: Option<String>,
    /// Receiver untuk streaming response events dari spawned task.
    pub(crate) stream_rx: Option<tokio::sync::mpsc::UnboundedReceiver<StreamEvent>>,
    /// True saat streaming response sedang berjalan.
    pub(crate) is_streaming: bool,
    /// Offset scroll chat history (jumlah baris yang discroll ke atas).
    pub chat_scroll_offset: usize,
    /// True = auto-scroll ke bawah. False = user sedang scroll manual ke atas.
    pub chat_at_bottom: bool,
}

impl TuiApp {
    #[must_use]
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            agents: Vec::new(),
            workers: Vec::new(),
            spawn_requests: Vec::new(),
            selected_index: 0,
            selected_tab: Tab::Session,
            should_quit: false,
            status_message: "ClawHive OS — Ctrl+P: palette | Esc: home | :help: commands".into(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            active_screen: Screen::Home,
            chat_history: Vec::new(),
            command_mode: CommandMode::None,
            active_model: "Not Configured".into(),
            model_sel_step: ModelSelectionStep::SelectProvider,
            model_sel_provider: String::new(),
            model_sel_families: Vec::new(),
            model_sel_variants: Vec::new(),
            model_sel_search: String::new(),
            model_sel_index: 0,
            model_sel_pending_provider: None,
            stream_rx: None,
            is_streaming: false,
            chat_scroll_offset: 0,
            chat_at_bottom: true,
        }
    }

    pub async fn refresh(&mut self) {
        use clawhive_control_api::store::{AGENT_PREFIX, SPAWNREQ_PREFIX};
        use clawhive_store::StoreExt;

        self.agents = self
            .state
            .kv_store
            .scan_prefix::<Agent>(AGENT_PREFIX)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, a)| a)
            .collect();

        self.workers = match self.state.worker_service.list(None).await {
            Ok(w) => w,
            Err(_) => Vec::new(),
        };

        self.spawn_requests = self
            .state
            .kv_store
            .scan_prefix::<SpawnRequest>(SPAWNREQ_PREFIX)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, r)| r)
            .collect();

        if self.selected_index >= self.current_list_len() {
            self.selected_index = self.current_list_len().saturating_sub(1);
        }
    }

    pub(crate) fn current_list_len(&self) -> usize {
        match self.selected_tab {
            Tab::Session => 0,
            Tab::Agents => self.agents.len(),
            Tab::Workers => self.workers.len(),
            Tab::SpawnRequests => self.spawn_requests.len(),
        }
    }

    pub async fn run(&mut self) -> Result<(), crate::TuiError> {
        use crossterm::ExecutableCommand;
        use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
        use ratatui::backend::CrosstermBackend;
        use ratatui::Terminal;
        use std::io::stdout;
        use crate::ui;

        enable_raw_mode().map_err(|e| crate::TuiError::TermInit(e.to_string()))?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen).map_err(|e| crate::TuiError::TermInit(e.to_string()))?;

        let mut terminal = Terminal::new(CrosstermBackend::new(stdout))
            .map_err(|e| crate::TuiError::TermInit(e.to_string()))?;
        terminal.clear().map_err(|e| crate::TuiError::TermInit(e.to_string()))?;

        let (tx, mut rx) = tokio::sync::mpsc::channel::<Event>(256);

        let tx_clone = tx.clone();
        tokio::spawn(async move {
            use crossterm::event;
            loop {
                if let Ok(event) = event::read() {
                    if tx_clone.send(event).await.is_err() {
                        break;
                    }
                }
            }
        });

        self.refresh().await;
        self.load_saved_api_key().await;

        while !self.should_quit {
            // Flush stream events sebelum render
            self.try_flush_stream().await;

            terminal
                .draw(|f| {
                    let area = f.area();
                    ui::draw(f, area, self);
                })
                .map_err(|e| crate::TuiError::Runtime(e.to_string()))?;

            // Tick 50ms agar streaming terus diflush meski tidak ada keystroke
            tokio::select! {
                Some(event) = rx.recv() => {
                    self.handle_event(event).await;
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                    // Tick kosong — loop kembali untuk flush stream & redraw
                }
            }
        }

        disable_raw_mode().map_err(|e| crate::TuiError::Runtime(e.to_string()))?;
        let _ = std::io::stdout().execute(LeaveAlternateScreen);
        Ok(())
    }
}
