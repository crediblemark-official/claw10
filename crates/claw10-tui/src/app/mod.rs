use std::sync::Arc;

use crossterm::event::Event;

use claw10_agent::events::AgentEvent;
use claw10_agent::runtime::AgentRuntime;
use claw10_control_api::state::AppState;
use claw10_domain::{Agent, AgentId, MissionId, SpawnRequest, Worker, Workspace};
use claw10_model_router::types::{ModelFamily, ModelProfile, StreamEvent};
use claw10_store::{NamespacedStore, Store, StoreExt};

mod commands;
mod events;
mod model_selection;
pub(crate) mod palette;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Session,
    Agents,
    Workers,
    SpawnRequests,
    Missions,
    Tasks,
    Memory,
    Approvals,
    Costs,
    Policies,
    Skills,
    Artifacts,
    Incidents,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Chat,
    WorkspaceSelect,
    Missions,
    Tasks,
    Memory,
    Approvals,
    Costs,
    Policies,
    Skills,
    Artifacts,
    Incidents,
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
    ManualModelInput {
        model_input: String,
        error_message: String,
    },
}

pub struct TuiApp {
    pub state: AppState,
    pub agents: Vec<Agent>,
    pub workers: Vec<Worker>,
    pub spawn_requests: Vec<SpawnRequest>,
    pub missions: Vec<claw10_domain::Mission>,
    pub tasks: Vec<claw10_domain::Task>,
    pub memories: Vec<claw10_domain::Memory>,
    pub approvals: Vec<claw10_domain::approval::ToolApprovalRequest>,
    pub policies: Vec<claw10_domain::PolicyBundle>,
    pub skills: Vec<claw10_domain::Skill>,
    pub artifacts: Vec<claw10_domain::Artifact>,
    pub incidents: Vec<claw10_domain::Incident>,
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
    /// Jika set, model selection diminta API key dulu baru resume provider ini.
    pub model_sel_pending_provider: Option<String>,
    /// Receiver untuk streaming response events dari spawned task.
    pub(crate) stream_rx: Option<tokio::sync::mpsc::UnboundedReceiver<StreamEvent>>,
    /// True saat streaming response sedang berjalan.
    pub(crate) is_streaming: bool,
    /// Status detail streaming (misal: "Berpikir...", "Memanggil tool...")
    pub stream_status: Option<String>,
    /// Indeks spinner untuk animasi progress/status
    pub spinner_tick: usize,
    /// Offset scroll chat history (jumlah baris yang discroll ke atas).
    pub chat_scroll_offset: std::cell::Cell<usize>,
    /// True = auto-scroll ke bawah. False = user sedang scroll manual ke atas.
    pub chat_at_bottom: bool,
    /// Waktu refresh sidebar terakhir
    pub last_refresh: std::time::Instant,
    // ── Agent Runtime fields ─────────────────────────────────────
    /// AgentRuntime yang digunakan untuk full agent reasoning loop.
    pub(crate) agent_runtime: Option<std::sync::Arc<AgentRuntime>>,
    /// ID agent aktif (root agent sesi TUI saat ini).
    pub(crate) active_agent_id: Option<AgentId>,
    /// Receiver AgentEvent dari running agent (untuk update TUI real-time).
    pub(crate) agent_rx: Option<tokio::sync::mpsc::UnboundedReceiver<AgentEvent>>,
    /// Request approval tool yang sedang tertunda (pending).
    pub pending_tool_approval: Option<claw10_domain::approval::ToolApprovalRequest>,
    /// Handle tokio spawn task dari agen yang sedang berjalan untuk interrupt/abort
    pub(crate) agent_task: Option<tokio::task::JoinHandle<()>>,
    /// Indeks item suggestion autocomplete yang sedang dipilih (selected).
    pub suggestion_index: usize,
    /// Daftar suggestions aktif: (teks tampilan, nilai autocomplete/command)
    pub active_suggestions: Vec<(String, String)>,
    // ── Workspace fields ─────────────────────────────────────────
    /// Store global (akar, tanpa namespace) — untuk menyimpan dan membaca metadata workspace.
    pub global_store: Arc<dyn Store>,
    /// Workspace yang sedang aktif.
    pub active_workspace: Option<Workspace>,
    /// Daftar semua workspace tersedia dari database.
    pub workspaces: Vec<Workspace>,
    /// Buffer input untuk form create new workspace di Home screen.
    pub workspace_input: String,
    /// Indeks item workspace yang sedang di-highlight di daftar.
    pub workspace_selected_index: usize,
    /// Menentukan apakah proses internal agent di-expand atau di-collapse.
    pub show_internal_process: bool,
}

impl TuiApp {
    #[must_use]
    pub fn new(state: AppState) -> Self {
        // Simpan reference ke store akar sebelum dipindahkan ke struct
        let global_store = Arc::clone(&state.kv_store);
        Self {
            state,
            agents: Vec::new(),
            workers: Vec::new(),
            spawn_requests: Vec::new(),
            missions: Vec::new(),
            tasks: Vec::new(),
            memories: Vec::new(),
            approvals: Vec::new(),
            policies: Vec::new(),
            skills: Vec::new(),
            artifacts: Vec::new(),
            incidents: Vec::new(),
            selected_index: 0,
            selected_tab: Tab::Session,
            should_quit: false,
            status_message: "Claw10 OS — Esc: Toggle Nav/Chat | Tab: Pindah Tab | Semua konfirmasi via form input (:help)".into(),
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
            stream_status: None,
            spinner_tick: 0,
            chat_scroll_offset: std::cell::Cell::new(0),
            chat_at_bottom: true,
            last_refresh: std::time::Instant::now(),
            agent_runtime: None,
            active_agent_id: None,
            agent_rx: None,
            pending_tool_approval: None,
            agent_task: None,
            suggestion_index: 0,
            active_suggestions: Vec::new(),
            global_store,
            active_workspace: None,
            workspaces: Vec::new(),
            workspace_input: String::new(),
            workspace_selected_index: 0,
            show_internal_process: false,
        }
    }

    /// Load semua workspace dari database global ke `self.workspaces`.
    pub async fn load_workspaces(&mut self) {
        // Selalu gunakan global_store (non-namespaced) untuk membaca daftar workspace
        if let Ok(items) = self.global_store.scan_prefix::<Workspace>("workspace:").await {
            let mut workspaces: Vec<Workspace> = items.into_iter().map(|(_, ws)| ws).collect();
            // Urutkan dari yang paling terakhir digunakan
            workspaces.sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at));
            self.workspaces = workspaces;
        }
    }

    /// Buat workspace baru dan langsung pilih (masuk ke chat).
    pub async fn create_workspace(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }

        let ws = Workspace::new(name, None);
        let key = ws.store_key();

        // Simpan metadata workspace ke global_store (tidak pernah ter-namespace)
        let _ = self.global_store.set(&key, &ws).await;

        // Buat namespaced store di atas global_store
        let ns = NamespacedStore::new(Arc::clone(&self.global_store), ws.namespace());
        let ns_store: Arc<dyn Store> = Arc::new(ns);

        // Pertahankan model_router dan tool_registry agar konfigurasi model tidak hilang
        let old_model_router = self.state.model_router.clone();
        let old_tool_registry = self.state.tool_registry.clone();

        // Ganti active store di AppState dengan namespaced store
        self.state = claw10_control_api::state::AppState::new_with_store(ns_store);
        self.state.model_router = old_model_router;
        self.state.tool_registry = old_tool_registry;

        self.active_workspace = Some(ws.clone());
        self.workspace_input.clear();
        self.workspaces.insert(0, ws);

        // Reset runtime untuk sesi baru di workspace ini
        self.init_agent_runtime().await;
        self.load_chat_history().await;
        self.active_screen = Screen::Chat;
    }

    /// Pilih workspace yang sudah ada dan masuk ke chat.
    pub async fn select_workspace(&mut self, ws: Workspace) {
        // Perbarui last_used_at di global_store (tidak pernah ter-namespace)
        let mut ws_updated = ws.clone();
        ws_updated.last_used_at = chrono::Utc::now();
        let _ = self.global_store.set(&ws_updated.store_key(), &ws_updated).await;

        // Buat namespaced store di atas global_store
        let ns = NamespacedStore::new(Arc::clone(&self.global_store), ws_updated.namespace());
        let ns_store: Arc<dyn Store> = Arc::new(ns);

        // Pertahankan model_router dan tool_registry agar konfigurasi model tidak hilang
        let old_model_router = self.state.model_router.clone();
        let old_tool_registry = self.state.tool_registry.clone();

        self.state = claw10_control_api::state::AppState::new_with_store(ns_store);
        self.state.model_router = old_model_router;
        self.state.tool_registry = old_tool_registry;

        self.active_workspace = Some(ws_updated);
        self.init_agent_runtime().await;
        self.load_chat_history().await;
        self.active_screen = Screen::Chat;
    }

    /// Hapus workspace terpilih dari database global.
    pub async fn delete_workspace(&mut self, ws: Workspace) {
        let _ = self.global_store.delete(&ws.store_key()).await;
        self.workspaces.retain(|w| w.id != ws.id);
        if self.workspace_selected_index >= self.workspaces.len() {
            self.workspace_selected_index = self.workspaces.len().saturating_sub(1);
        }
        self.status_message = format!("Workspace '{}' berhasil dihapus.", ws.name);
    }

    /// Muat riwayat chat dari database workspace aktif.
    pub async fn load_chat_history(&mut self) {
        self.chat_history.clear();
        if let Ok(Some(history)) = self.state.kv_store.get::<Vec<(String, String, String)>>("chat_history").await {
            self.chat_history = history;
        }
    }

    /// Simpan riwayat chat aktif ke database workspace aktif.
    pub async fn save_chat_history(&self) {
        let _ = self.state.kv_store.set("chat_history", &self.chat_history).await;
    }

    pub async fn refresh(&mut self) {
        use claw10_control_api::store::{AGENT_PREFIX, MISSION_PREFIX, SPAWNREQ_PREFIX, TASK_PREFIX};
        use claw10_store::StoreExt;

        use claw10_domain::{Agent, AgentState};

        // Proses spawn request yang Approved sebelum refresh data list
        self.process_approved_spawns().await;

        // Dapatkan mission_id dari agent aktif saat ini (jika ada)
        let current_mission_id = if let Some(ref active_id) = self.active_agent_id {
            let key = format!("{AGENT_PREFIX}{}", active_id.0);
            self.state.kv_store.get::<Agent>(&key).await.ok().flatten().map(|a| a.mission_id)
        } else {
            None
        };

        self.agents = self
            .state
            .kv_store
            .scan_prefix::<Agent>(AGENT_PREFIX)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, a)| a)
            .filter(|a| {
                // Sembunyikan agent yang sudah Terminated
                if a.state == AgentState::Terminated {
                    return false;
                }
                // Tampilkan agen global "cli-agent"
                if a.name == "cli-agent" {
                    return true;
                }
                // Jika sudah ada sesi aktif, tampilkan hanya agen yang memiliki mission_id sama
                if let Some(ref mid) = current_mission_id {
                    a.mission_id == *mid
                } else {
                    // Jika belum ada agent aktif (startup), sembunyikan Root Agent lama
                    a.name != "TUI Root Agent"
                }
            })
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

        // Load data untuk TUI screens baru (read-only list)
        self.missions = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::Mission>(MISSION_PREFIX)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, m)| m)
            .collect();

        self.tasks = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::Task>(TASK_PREFIX)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, t)| t)
            .collect();

        self.memories = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::Memory>("memory:")
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, m)| m)
            .collect();

        self.approvals = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::approval::ToolApprovalRequest>("tool_approval:")
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, a)| a)
            .collect();

        self.policies = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::PolicyBundle>("policy:bundle:")
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, p)| p)
            .collect();

        self.skills = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::Skill>("skill:")
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, s)| s)
            .collect();

        self.artifacts = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::Artifact>("artifact:meta:")
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, a)| a)
            .collect();

        // Incident tidak selalu disimpan dengan prefix di store saat ini.
        // Jika ada, gunakan prefix "incident:"; jika tidak, tampilkan list kosong.
        self.incidents = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::Incident>("incident:")
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(_, i)| i)
            .collect();

        // Deteksi pending tool approval
        self.pending_tool_approval = self
            .approvals
            .iter()
            .find(|r| r.state == claw10_domain::approval::ToolApprovalState::Pending)
            .cloned();

        if self.selected_index >= self.current_list_len() {
            self.selected_index = self.current_list_len().saturating_sub(1);
        }
    }

    pub(crate) fn current_list_len(&self) -> usize {
        match self.selected_tab {
            Tab::Session => 1,
            Tab::Agents => self.agents.len(),
            Tab::Workers => self.workers.len(),
            Tab::SpawnRequests => self.spawn_requests.len(),
            Tab::Missions => self.missions.len(),
            Tab::Tasks => self.tasks.len(),
            Tab::Memory => self.memories.len(),
            Tab::Approvals => self.approvals.len(),
            Tab::Costs => self.agents.len(),
            Tab::Policies => self.policies.len(),
            Tab::Skills => self.skills.len(),
            Tab::Artifacts => self.artifacts.len(),
            Tab::Incidents => self.incidents.len(),
        }
    }

    /// Map tab yang dipilih ke screen yang sesuai. Tab lama tetap di Chat
    /// (sidebar), tab baru membuka screen full-page read-only.
    pub(crate) fn screen_for_tab(tab: Tab) -> Screen {
        match tab {
            Tab::Session | Tab::Agents | Tab::Workers | Tab::SpawnRequests => Screen::Chat,
            Tab::Missions => Screen::Missions,
            Tab::Tasks => Screen::Tasks,
            Tab::Memory => Screen::Memory,
            Tab::Approvals => Screen::Approvals,
            Tab::Costs => Screen::Costs,
            Tab::Policies => Screen::Policies,
            Tab::Skills => Screen::Skills,
            Tab::Artifacts => Screen::Artifacts,
            Tab::Incidents => Screen::Incidents,
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
        self.load_workspaces().await;
        self.load_saved_api_key().await;
        
        // Load model aktif terakhir dari database
        if let Ok(Some(last_model)) = self.global_store.get::<String>("last_active_model").await {
            self.active_model = self.resolve_model_id_without_provider(&last_model);
        } else {
            // Fallback ke model pertama jika ada
            if let Some(router) = &self.state.model_router {
                let profiles = router.registry().list_profiles();
                if !profiles.is_empty() {
                    self.active_model = profiles[0].id.clone();
                }
            }
        }

        self.init_agent_runtime().await;

        while !self.should_quit {
            self.spinner_tick = self.spinner_tick.wrapping_add(1);
            // Refresh data dari database secara periodik setiap 1 detik
            if self.last_refresh.elapsed() >= std::time::Duration::from_secs(1) {
                self.refresh().await;
                self.last_refresh = std::time::Instant::now();
            }

            // Flush stream events dan agent events sebelum render
            self.try_flush_stream().await;
            self.try_flush_agent_events().await;

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

    /// Inisialisasi AgentRuntime dari services yang tersedia di AppState.
    /// Dipanggil setelah model router berhasil dikonfigurasi.
    pub(crate) async fn init_agent_runtime(&mut self) {
        let router = match self.state.model_router.as_ref() {
            Some(r) => std::sync::Arc::clone(r),
            None => return, // belum ada model router, skip
        };

        // Picu pembaruan latar belakang secara asinkron untuk mengambil profile model riil dari API server
        let router_clone = std::sync::Arc::clone(&router);
        tokio::spawn(async move {
            let providers = router_clone.registry().list_providers();
            for provider_name in providers {
                if let Ok(provider) = router_clone.registry().get_provider(&provider_name) {
                    if let Ok(models) = provider.fetch_models().await {
                        router_clone.inject_profiles(models);
                    }
                }
            }
        });

        let tool_registry = match self.state.tool_registry.as_ref() {
            Some(t) => std::sync::Arc::clone(t),
            None => {
                // Buat registry kosong jika belum ada
                std::sync::Arc::new(claw10_tool::registry::ToolRegistry::new())
            }
        };
        let worker_service = std::sync::Arc::clone(&self.state.worker_service);
        let kv_store = std::sync::Arc::clone(&self.state.kv_store);

        match crate::tui_agent::build_tui_runtime(kv_store, router, tool_registry, worker_service).await {
            Ok((runtime, _worker_id)) => {
                self.agent_runtime = Some(std::sync::Arc::new(runtime));
            }
            Err(e) => {
                tracing::warn!("Gagal init AgentRuntime: {e}");
            }
        }
    }

    /// Resolves a dynamic model ID by trying to match prefix from registry or fallback (iterating over all mapping rules).
    pub(crate) fn resolve_model_id_without_provider(&self, name: &str) -> String {
        if name.contains('/') {
            return name.to_string();
        }

        // 1. Coba cari kecocokan di registry model_router
        if let Some(router) = &self.state.model_router {
            let matched = router.registry().list_profiles().iter().find(|p| {
                let p_id_lower = p.id.to_lowercase();
                let name_lower = name.to_lowercase();
                p_id_lower.contains(&name_lower) || name_lower.contains(&p_id_lower)
            }).map(|p| p.id.clone());
            if let Some(mid) = matched {
                return mid;
            }
        }

        // 2. Jika tidak ada, panggil compile-time static resolver
        let resolved = claw10_model_router::models::resolve_static_model(name, "nvidia");
        if resolved.starts_with("nvidia/") {
            let openrouter_resolved = claw10_model_router::models::resolve_static_model(name, "openrouter");
            if !openrouter_resolved.starts_with("openrouter/") {
                return openrouter_resolved;
            }
        }
        resolved
    }

    pub(crate) fn set_active_model(&mut self, model_id: String) {
        let resolved = self.resolve_model_id_without_provider(&model_id);
        self.active_model = resolved.clone();
        let store = std::sync::Arc::clone(&self.global_store);
        tokio::spawn(async move {
            let _ = store.set("last_active_model", &resolved).await;
        });
    }

    pub(crate) fn update_suggestions(&mut self) {
        self.active_suggestions.clear();
        
        if self.input_buffer.starts_with('/') {
            if self.input_buffer.starts_with("/model") {
                if let Some(router) = &self.state.model_router {
                    let profiles = router.registry().list_profiles();
                    let search = if self.input_buffer.len() > 7 {
                        self.input_buffer[7..].trim().to_lowercase()
                    } else {
                        String::new()
                    };
                    
                    self.active_suggestions = profiles
                        .iter()
                        .filter(|p| search.is_empty() || p.id.to_lowercase().contains(&search))
                        .map(|p| (p.id.clone(), format!("/model {}", p.id)))
                        .collect();
                }
            } else {
                let query = self.input_buffer.to_lowercase();
                let all_commands = vec![
                    ("/model <id>".to_string(), "/model ".to_string()),
                    ("/help".to_string(), "/help".to_string()),
                    ("/refresh".to_string(), "/refresh".to_string()),
                    ("/clear".to_string(), "/clear".to_string()),
                    ("/workspace".to_string(), "/workspace".to_string()),
                    ("/q".to_string(), "/q".to_string()),
                ];
                
                self.active_suggestions = all_commands
                    .into_iter()
                    .filter(|(cmd_name, _)| cmd_name.starts_with(&query) || query == "/")
                    .collect();
            }
        }
        
        let max = self.active_suggestions.len().saturating_sub(1);
        if self.suggestion_index > max {
            self.suggestion_index = 0;
        }
    }

    pub(crate) async fn clear_app_data(&mut self) {
        use claw10_domain::Agent;
        use claw10_store::StoreExt;

        // Cari active_mission_id dari active_agent_id saat ini sebelum di-reset
        let active_mission_id = if let Some(agent_id) = &self.active_agent_id {
            if let Ok(Some(agent)) = self.state.kv_store.get::<Agent>(&format!("agent:{}", agent_id.0)).await {
                Some(agent.mission_id)
            } else {
                None
            }
        } else {
            None
        };

        // 1. Bersihkan memory history & active agent ID
        self.chat_history.clear();
        self.active_agent_id = None;
        self.stop_streaming();
        
        // 2. Inisialisasi ulang AgentRuntime agar memory context kosong
        self.init_agent_runtime().await;
        
        // 3. Bersihkan database yang terikat pada sesi aktif saat ini saja
        if let Some(ref mission_id) = active_mission_id {
            // Hapus agents yang terikat pada mission aktif ini
            if let Ok(keys) = self.state.kv_store.scan_prefix::<claw10_domain::Agent>("agent:").await {
                for (key, agent) in keys {
                    if agent.mission_id == *mission_id {
                        let _ = self.state.kv_store.delete(&key).await;
                        
                        // Hapus always allow milik agent tersebut
                        let allow_prefix = format!("always_allow:{}:", agent.id.0);
                        if let Ok(allow_keys) = self.state.kv_store.scan_prefix::<serde_json::Value>(&allow_prefix).await {
                            for (allow_key, _) in allow_keys {
                                let _ = self.state.kv_store.delete(&allow_key).await;
                            }
                        }
                    }
                }
            }
            
            // Hapus mission aktif ini
            let mission_key = format!("mission:{}", mission_id.0);
            let _ = self.state.kv_store.delete(&mission_key).await;
            
            // Hapus spawn requests yang terikat pada mission aktif ini
            if let Ok(keys) = self.state.kv_store.scan_prefix::<claw10_domain::SpawnRequest>("spawnreq:").await {
                for (key, req) in keys {
                    if req.mission_id == *mission_id {
                        let _ = self.state.kv_store.delete(&key).await;
                    }
                }
            }
            
            // Hapus session terkait jika ada di database
            let session_key = format!("session:{}", mission_id.0);
            let _ = self.state.kv_store.delete(&session_key).await;
        }
        
        // Refresh data sidebar TUI agar kosong
        self.refresh().await;
        
        self.chat_history.push((
            "System".to_string(),
            "".to_string(),
            "Cache, history, dan context untuk sesi ini berhasil dibersihkan.".to_string(),
        ));
    }

    /// Pastikan agent aktif sudah ada di store. Jika belum, buat dan simpan.
    /// Mengembalikan AgentId yang siap digunakan.
    pub(crate) async fn ensure_agent(&mut self) -> Option<AgentId> {
        if let Some(id) = self.active_agent_id.clone() {
            return Some(id);
        }

        // Pastikan runtime sudah ada
        if self.agent_runtime.is_none() {
            return None;
        }

        let agent_id = AgentId(uuid::Uuid::now_v7());
        let mission_id = MissionId(uuid::Uuid::now_v7());
        let agent = crate::tui_agent::make_default_agent(
            agent_id.clone(),
            &self.active_model,
            mission_id,
        );

        // Simpan agent ke store melalui AgentStore yang disimpan di runtime
        // Kita perlu menyimpan via KV store langsung
        use claw10_store::StoreExt;
        let key = format!("agent:{}", agent_id.0);
        if let Err(e) = self.state.kv_store.set(&key, &agent).await {
            tracing::warn!("Gagal simpan agent TUI: {e}");
            return None;
        }

        self.active_agent_id = Some(agent_id.clone());
        Some(agent_id)
    }

    /// Deteksi spawn requests yang Approved, proses via SpawnBroker,
    /// jalankan reasoning loop anak-anaknya, dan ubah state request ke Completed.
    pub(crate) async fn process_approved_spawns(&mut self) {
            use claw10_control_api::store::{AGENT_PREFIX, MISSION_PREFIX, SPAWNREQ_PREFIX};
        use claw10_domain::{Agent, Mission, SpawnState};
        use claw10_store::StoreExt;

        let requests = self
            .state
            .kv_store
            .scan_prefix::<claw10_domain::SpawnRequest>(SPAWNREQ_PREFIX)
            .await
            .unwrap_or_default();

        for (key, mut req) in requests {
            if req.state != SpawnState::Approved {
                continue;
            }

            // Resolve model profile sebelum passing ke broker
            let mut resolved_req = req.clone();
            for child in &mut resolved_req.children {
                if child.model_profile == "default" {
                    child.model_profile = self.active_model.clone();
                }
                child.model_profile = self.resolve_model_id_without_provider(&child.model_profile);
            }

            // Load parent agent
            let parent_key = format!("{AGENT_PREFIX}{}", req.requested_by.0);
            let Some(mut parent) = self.state.kv_store.get::<Agent>(&parent_key).await.unwrap_or_default() else {
                tracing::warn!("Parent agent {} not found for spawn request {}", req.requested_by.0, req.id.0);
                req.state = SpawnState::Failed;
                req.updated_at = chrono::Utc::now();
                let _ = self.state.kv_store.set(&key, &req).await;
                continue;
            };

            // Load mission
            let mission_key = format!("{MISSION_PREFIX}{}", req.mission_id.0);
            let Some(mission) = self.state.kv_store.get::<Mission>(&mission_key).await.unwrap_or_default() else {
                tracing::warn!("Mission {} not found for spawn request {}", req.mission_id.0, req.id.0);
                req.state = SpawnState::Failed;
                req.updated_at = chrono::Utc::now();
                let _ = self.state.kv_store.set(&key, &req).await;
                continue;
            };

            // Load all agents untuk depth/swarm validation
            let all_agents: Vec<Agent> = self
                .state
                .kv_store
                .scan_prefix_unsorted::<Agent>(AGENT_PREFIX)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|(_, a)| a)
                .collect();

            let current_depth = Self::calculate_spawn_depth(&req.requested_by, &all_agents);

            // Proses via broker
            match self
                .state
                .spawn_broker
                .process_spawn_request(&mut parent, &mission, &resolved_req, &all_agents, current_depth)
                .await
            {
                Ok(children) => {
                    req.state = SpawnState::Completed;
                    req.updated_at = chrono::Utc::now();
                    let _ = self.state.kv_store.set(&key, &req).await;

                    for (idx, child) in children.iter().enumerate() {
                        let objective = req
                            .children
                            .get(idx)
                            .map(|c| c.objective.clone())
                            .unwrap_or_default();

                        self.chat_history.push((
                            "System".to_string(),
                            "".to_string(),
                            format!(
                                "Spawned child agent '{}' (role: {}) untuk objective: '{}'",
                                child.name, child.role, objective
                            ),
                        ));

                        // Jalankan child agent jika runtime siap
                        if let Some(runtime) = &self.agent_runtime {
                            let runtime_clone = std::sync::Arc::clone(runtime);
                            let child_id = child.id.clone();
                            let (agent_tx, agent_rx) = tokio::sync::mpsc::unbounded_channel();

                            // Set receiver ke TUI agar output streaming masuk ke UI chat
                            self.agent_rx = Some(agent_rx);
                            self.is_streaming = true;
                            self.stream_status = Some(format!("Executing child agent {}...", child.name));

                            tokio::spawn(async move {
                                let ctx = std::collections::HashMap::new();
                                if let Err(e) = runtime_clone
                                    .execute_agent_streaming(&child_id, objective, ctx, None, agent_tx.clone(), None)
                                    .await

                                {
                                    let _ = agent_tx.send(AgentEvent::Error {
                                        message: format!("Child Agent error: {e}"),
                                    });
                                }
                            });
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Spawn broker failed for request {}: {e}", req.id.0);
                    req.state = SpawnState::Failed;
                    req.updated_at = chrono::Utc::now();
                    let _ = self.state.kv_store.set(&key, &req).await;

                    self.chat_history.push((
                        "System".to_string(),
                        "".to_string(),
                        format!("Spawn request {} failed: {e}", req.id.0),
                    ));
                }
            }
        }
    }

    /// Hitung depth agen relatif terhadap root (parent=None).
    fn calculate_spawn_depth(agent_id: &claw10_domain::AgentId, agents: &[Agent]) -> u32 {
        let mut depth = 0;
        let mut current = agent_id.clone();
        while let Some(agent) = agents.iter().find(|a| a.id == current) {
            match &agent.parent_agent_id {
                Some(pid) => {
                    depth += 1;
                    current = pid.clone();
                }
                None => break,
            }
        }
        depth
    }
}

