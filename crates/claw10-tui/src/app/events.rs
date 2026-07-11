use std::sync::Arc;

use crossterm::event::{Event, KeyCode, KeyEventKind};

use claw10_agent::events::AgentEvent;
use claw10_model_router::types::{ChatRequest, MessageRole, ModelMessage, StreamEvent};
use crate::app::{CommandMode, ModelSelectionStep, Screen, Tab, TuiApp};
use crate::app::palette::get_palette_items;

impl TuiApp {
    pub(crate) async fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                // 1. Handle Ctrl+C to quit
                if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.should_quit = true;
                    return;
                }

                // 2. Handle Ctrl+P to trigger Command Palette
                if key.code == KeyCode::Char('p') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.command_mode = CommandMode::CommandPalette {
                        search_query: String::new(),
                        selected_index: 0,
                        filtered_items: get_palette_items(),
                    };
                    return;
                }

                // 2b. Handle Ctrl+G / F3 / Ctrl+I / Ctrl+T to toggle show_internal_process
                let is_toggle_key = match key.code {
                    KeyCode::F(3) => true,
                    KeyCode::Char('g') | KeyCode::Char('G') |
                    KeyCode::Char('i') | KeyCode::Char('I') |
                    KeyCode::Char('t') | KeyCode::Char('T') => {
                        key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                    }
                    _ => false,
                };
                if is_toggle_key {
                    self.show_internal_process = !self.show_internal_process;
                    return;
                }

                // 3. Handle key based on current CommandMode
                match &mut self.command_mode {
                    CommandMode::ApiKeyInput { key_input, error_message } => {
                        match key.code {
                            KeyCode::Esc => {
                                if self.model_sel_pending_provider.is_some() {
                                    self.model_sel_pending_provider = None;
                                    self.command_mode = CommandMode::ModelSelection;
                                    self.model_sel_step = ModelSelectionStep::SelectProvider;
                                } else {
                                    self.command_mode = CommandMode::None;
                                }
                            }
                            KeyCode::Enter => {
                                let key = std::mem::take(key_input);

                                // If provider is predetermined (from model selection), use it directly
                                if let Some(provider) = self.model_sel_pending_provider.clone() {
                                    if key.trim().is_empty() {
                                        *error_message = "API key cannot be empty".into();
                                        *key_input = key;
                                    } else {
                                        let actual_key = key.trim().to_string();
                                        self.persist_api_key(&provider, &actual_key).await;
                                        self.register_all_providers().await;
                                        let model_count = self.state.model_router.as_ref()
                                            .and_then(|r| Some(r.registry().list_profiles().len()))
                                            .unwrap_or(0);
                                        self.status_message = format!(
                                            "{provider} API key set. {model_count} model(s) available."
                                        );
                                        self.model_sel_pending_provider = None;
                                        self.model_sel_provider = provider.clone();
                                        self.advance_to_model_list().await;
                                        // Init agent runtime setelah provider baru terdaftar
                                        self.init_agent_runtime().await;
                                        return;
                                    }
                                } else if key.trim().is_empty() {
                                    *error_message = "API key cannot be empty. Use `provider:key` format.".into();
                                    *key_input = key;
                                } else {
                                    let trimmed = key.trim().to_string();
                                    let (provider, actual_key) = if let Some(idx) = trimmed.find(':') {
                                        let (p, k) = trimmed.split_at(idx);
                                        (p.to_string(), k[1..].trim().to_string())
                                    } else {
                                        let catalog = claw10_model_router::providers::provider_configs();
                                        let matched = catalog.iter().find(|c| c.name == trimmed);
                                        match matched {
                                            Some(c) => (c.name.to_string(), String::new()),
                                            None => ("".to_string(), trimmed.clone()),
                                        }
                                    };

                                    if provider.is_empty() {
                                        *error_message = "Use `provider:key` format (e.g. openai:sk-...)".into();
                                        *key_input = trimmed;
                                    } else if actual_key.is_empty() {
                                        *error_message = format!("API key for '{provider}' is empty.");
                                        *key_input = trimmed;
                                    } else {
                                        self.persist_api_key(&provider, &actual_key).await;
                                        self.register_all_providers().await;
                                        let model_count = self.state.model_router.as_ref()
                                            .and_then(|r| Some(r.registry().list_profiles().len()))
                                            .unwrap_or(0);
                                        self.status_message = format!(
                                            "{provider} API key set. {model_count} model(s) available."
                                        );
                                        self.command_mode = CommandMode::None;
                                        // Init agent runtime setelah provider baru terdaftar
                                        self.init_agent_runtime().await;
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                key_input.pop();
                            }
                            KeyCode::Char(c) => {
                                key_input.push(c);
                            }
                            _ => {}
                        }
                    }
                    CommandMode::ManualModelInput { model_input, error_message } => {
                        match key.code {
                            KeyCode::Esc => {
                                self.command_mode = CommandMode::ModelSelection;
                                self.model_sel_index = 0;
                            }
                            KeyCode::Backspace => {
                                model_input.pop();
                            }
                            KeyCode::Enter => {
                                let val = model_input.trim().to_string();
                                if val.is_empty() {
                                    *error_message = "Nama model tidak boleh kosong".into();
                                } else {
                                    let provider_name = self.model_sel_provider.clone();
                                    let profile = claw10_model_router::types::ModelProfile {
                                        id: val.clone(),
                                        provider: provider_name.clone(),
                                        model_name: val.clone(),
                                        context_window: 128_000,
                                        max_output_tokens: 8_192,
                                        cost_per_1m_input: 0.0,
                                        cost_per_1m_output: 0.0,
                                        suitable_for: vec!["general".to_string()],
                                    };

                                    if let Some(router) = &self.state.model_router {
                                        router.inject_profile(profile.clone());
                                    }

                                    self.set_active_model(val.clone());
                                    self.status_message = format!("Model manual ditambahkan: {} (via {})", val, provider_name);
                                    self.command_mode = CommandMode::None;
                                    self.reset_model_selection();
                                }
                            }
                            KeyCode::Char(c) => {
                                model_input.push(c);
                            }
                            _ => {}
                        }
                    }
                    CommandMode::ModelSelection => {
                        match key.code {
                            KeyCode::Esc => {
                                self.command_mode = CommandMode::None;
                                self.reset_model_selection();
                            }
                            KeyCode::Up => {
                                self.model_sel_index = self.model_sel_index.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                let max = self.active_model_list_len();
                                if self.model_sel_index < max {
                                    self.model_sel_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                self.handle_model_selection_enter().await;
                            }
                            KeyCode::Backspace => {
                                self.model_sel_search.pop();
                                // Reset index when search changes
                                self.model_sel_index = 0;
                            }
                            KeyCode::Char(c) => {
                                self.model_sel_search.push(c);
                                self.model_sel_index = 0;
                            }
                            _ => {}
                        }
                    }
                    CommandMode::CommandPalette { search_query, selected_index, filtered_items } => {
                        match key.code {
                            KeyCode::Esc => {
                                self.command_mode = CommandMode::None;
                            }
                            KeyCode::Up => {
                                if *selected_index > 0 {
                                    *selected_index -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if *selected_index < filtered_items.len().saturating_sub(1) {
                                    *selected_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                if !filtered_items.is_empty() {
                                    let action = filtered_items[*selected_index].3.clone();
                                    self.execute_palette_action(&action).await;
                                }
                                if matches!(self.command_mode, CommandMode::CommandPalette { .. }) {
                                    self.command_mode = CommandMode::None;
                                }
                            }
                            KeyCode::Backspace => {
                                search_query.pop();
                                self.update_palette_filter();
                            }
                            KeyCode::Char(c) => {
                                search_query.push(c);
                                self.update_palette_filter();
                            }
                            _ => {}
                        }
                    }
                    CommandMode::None => {
                        // ── Workspace Home Screen Handler ─────────────────────────────────────
                        if self.active_screen == Screen::Home {
                            match key.code {
                                KeyCode::Up => {
                                    if self.workspace_selected_index > 0 {
                                        self.workspace_selected_index -= 1;
                                    }
                                    return;
                                }
                                KeyCode::Down => {
                                    if self.workspace_selected_index < self.workspaces.len().saturating_sub(1) {
                                        self.workspace_selected_index += 1;
                                    }
                                    return;
                                }
                                KeyCode::Enter => {
                                    // Jika ada teks di workspace_input → buat workspace baru
                                    if !self.workspace_input.is_empty() {
                                        let name = self.workspace_input.clone();
                                        self.create_workspace(&name).await;
                                        return;
                                    }
                                    // Jika tidak ada input tapi ada workspace terpilih → select
                                    if !self.workspaces.is_empty() {
                                        let ws = self.workspaces[self.workspace_selected_index].clone();
                                        self.select_workspace(ws).await;
                                        return;
                                    }
                                }
                                KeyCode::Tab => {
                                    // Tab: pilih workspace yang di-highlight langsung
                                    if !self.workspaces.is_empty() {
                                        let ws = self.workspaces[self.workspace_selected_index].clone();
                                        self.select_workspace(ws).await;
                                        return;
                                    }
                                }
                                KeyCode::Backspace => {
                                    self.workspace_input.pop();
                                    return;
                                }
                                KeyCode::Char(c) => {
                                    self.workspace_input.push(c);
                                    return;
                                }
                                KeyCode::Esc => {
                                    self.workspace_input.clear();
                                    return;
                                }
                                KeyCode::Delete => {
                                    if !self.workspaces.is_empty() {
                                        let ws = self.workspaces[self.workspace_selected_index].clone();
                                        self.delete_workspace(ws).await;
                                        return;
                                    }
                                }
                                _ => {}
                            }
                            return;
                        }

                        // ── Read-only list screens handler ────────────────────────────────────
                        if matches!(
                            self.active_screen,
                            Screen::Missions
                                | Screen::Tasks
                                | Screen::Memory
                                | Screen::Approvals
                                | Screen::Costs
                                | Screen::Policies
                                | Screen::Skills
                                | Screen::Artifacts
                                | Screen::Incidents
                        ) {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                                    self.active_screen = Screen::Home;
                                }
                                KeyCode::Up => {
                                    if self.selected_index > 0 {
                                        self.selected_index -= 1;
                                    }
                                }
                                KeyCode::Down => {
                                    let max = self.current_list_len().saturating_sub(1);
                                    if self.selected_index < max {
                                        self.selected_index += 1;
                                    }
                                }
                                KeyCode::Tab => {
                                    self.selected_tab = match self.selected_tab {
                                        Tab::Session => Tab::Agents,
                                        Tab::Agents => Tab::Workers,
                                        Tab::Workers => Tab::SpawnRequests,
                                        Tab::SpawnRequests => Tab::Missions,
                                        Tab::Missions => Tab::Tasks,
                                        Tab::Tasks => Tab::Memory,
                                        Tab::Memory => Tab::Approvals,
                                        Tab::Approvals => Tab::Costs,
                                        Tab::Costs => Tab::Policies,
                                        Tab::Policies => Tab::Skills,
                                        Tab::Skills => Tab::Artifacts,
                                        Tab::Artifacts => Tab::Incidents,
                                        Tab::Incidents => Tab::Session,
                                    };
                                    self.selected_index = 0;
                                    self.active_screen = crate::app::TuiApp::screen_for_tab(self.selected_tab);
                                }
                                _ => {}
                            }
                            return;
                        }

                        // Standard Input / Navigation handling
                        if self.pending_tool_approval.is_some() {
                            match key.code {
                                KeyCode::Left => {
                                    if self.tool_approval_index > 0 {
                                        self.tool_approval_index -= 1;
                                    } else {
                                        self.tool_approval_index = 2;
                                    }
                                }
                                KeyCode::Right => {
                                    if self.tool_approval_index < 2 {
                                        self.tool_approval_index += 1;
                                    } else {
                                        self.tool_approval_index = 0;
                                    }
                                }
                                KeyCode::Enter => {
                                    let state = match self.tool_approval_index {
                                        0 => claw10_domain::approval::ToolApprovalState::Approved,
                                        1 => claw10_domain::approval::ToolApprovalState::AlwaysApproved,
                                        _ => claw10_domain::approval::ToolApprovalState::Denied,
                                    };
                                    self.handle_tool_approval(state).await;
                                }
                                _ => {}
                            }
                            return;
                        }

                        match key.code {
                            KeyCode::Enter => {
                                if !self.active_suggestions.is_empty() {
                                    let selected_val = self.active_suggestions[self.suggestion_index].1.clone();
                                    self.input_buffer = selected_val;
                                    self.update_suggestions();
                                    
                                    // Jika autocomplete-nya lengkap (tidak diakhiri spasi), langsung jalankan command
                                    if !self.input_buffer.ends_with(' ') {
                                        let content = std::mem::take(&mut self.input_buffer);
                                        let trimmed = content.trim();
                                        if let Some(cmd) = trimmed.strip_prefix(':').or_else(|| trimmed.strip_prefix('/')) {
                                            self.execute_command(cmd).await;
                                        }
                                        self.active_suggestions.clear();
                                    }
                                    return;
                                }

                                let content = std::mem::take(&mut self.input_buffer);
                                let trimmed = content.trim();
                                if !trimmed.is_empty() {
                                    if let Some(cmd) = trimmed.strip_prefix(':').or_else(|| trimmed.strip_prefix('/')) {
                                        self.execute_command(cmd).await;
                                    } else {
                                        self.chat_history.push(("User".to_string(), "".to_string(), trimmed.to_string()));
                                        self.save_chat_history().await;
                                        self.active_screen = Screen::Chat;
                                        self.chat_scroll_offset.set(0);
                                        self.chat_at_bottom = true;

                                        // ── Coba pakai AgentRuntime dulu ─────────────────────
                                        if self.agent_runtime.is_some() {
                                            let objective = trimmed.to_string();
                                            let model_label = self.active_model.clone();

                                            if let Some(agent_id) = self.ensure_agent().await {
                                                let history_entries: Vec<String> = self.chat_history.iter()
                                                    .map(|(sender, model, msg)| {
                                                        if model.is_empty() {
                                                            format!("{sender}: {msg}")
                                                        } else {
                                                            format!("{sender} ({model}): {msg}")
                                                        }
                                                    })
                                                    .collect();
                                                let history_json = serde_json::to_string(&history_entries).unwrap_or_default();

                                                self.chat_history.push((
                                                    "Agent".to_string(),
                                                    model_label.clone(),
                                                    String::new(),
                                                ));
                                                self.save_chat_history().await;
                                                self.is_streaming = true;
                                                self.stream_status = Some("Memulai agent...".to_string());

                                                let runtime = Arc::clone(self.agent_runtime.as_ref().unwrap());
                                                let (agent_tx, agent_rx) = tokio::sync::mpsc::unbounded_channel();
                                                self.agent_rx = Some(agent_rx);

                                                let task_handle = tokio::spawn(async move {
                                                    let mut ctx = std::collections::HashMap::new();
                                                    ctx.insert("chat_history".to_string(), history_json);
                                                    match runtime.execute_agent_streaming(
                                                        &agent_id,
                                                        objective,
                                                        ctx,
                                                        None,
                                                        agent_tx.clone(),
                                                        None,
                                                    ).await {

                                                        Ok(_) => {}
                                                        Err(e) => {
                                                            let _ = agent_tx.send(AgentEvent::Error {
                                                                message: format!("AgentRuntime error: {e}"),
                                                            });
                                                        }
                                                    }
                                                });
                                                self.agent_task = Some(task_handle);
                                            } else {
                                                self.chat_history.push((
                                                    "System".to_string(),
                                                    "".into(),
                                                    "Gagal inisialisasi agent. Pastikan API key sudah di-set.".into(),
                                                ));
                                                self.save_chat_history().await;
                                            }
                                        } else {
                                            // ── Fallback: langsung ke model router (no agent) ──
                                            let router_opt = self.state.model_router.as_ref().map(Arc::clone);
                                            match router_opt {
                                                Some(router) => {
                                                    let model_id = self.active_model.clone();
                                                    let mut messages = Vec::new();
                                                    for (sender, model, msg) in &self.chat_history {
                                                        let role = match sender.as_str() {
                                                            "User" => MessageRole::User,
                                                            "Agent" => MessageRole::Assistant,
                                                            "Tool" => MessageRole::Tool,
                                                            "System" => MessageRole::System,
                                                            _ => MessageRole::User,
                                                        };
                                                        if msg.is_empty() {
                                                            continue;
                                                        }
                                                        messages.push(ModelMessage {
                                                            role,
                                                            content: msg.clone(),
                                                            tool_calls: None,
                                                            tool_call_id: None,
                                                            name: if !model.is_empty() { Some(model.clone()) } else { None },
                                                        });
                                                    }

                                                    let request = ChatRequest {
                                                        model: model_id.clone(),
                                                        messages,
                                                        max_tokens: Some(4096),
                                                        temperature: Some(0.7),
                                                        tools: None,
                                                        stop: None,
                                                    };

                                                    self.stop_streaming();

                                                    match router.route_chat_stream(&model_id, request).await {
                                                        Ok(handle) => {
                                                            let label = model_id.clone();
                                                            self.chat_history.push((
                                                                "Agent".to_string(),
                                                                label.clone(),
                                                                String::new(),
                                                            ));
                                                            self.save_chat_history().await;
                                                            self.is_streaming = true;
                                                            self.stream_status = Some("Menghubungi API model...".to_string());

                                                            let (stream_tx, stream_rx) = tokio::sync::mpsc::unbounded_channel();
                                                            self.stream_rx = Some(stream_rx);

                                                            let task_handle = tokio::spawn(async move {
                                                                while let Some(event) = handle.recv().await {
                                                                    if stream_tx.send(event).is_err() {
                                                                        break;
                                                                    }
                                                                }
                                                            });
                                                            self.agent_task = Some(task_handle);
                                                        }
                                                        Err(e) => {
                                                            self.chat_history.push((
                                                                "System".to_string(),
                                                                "".into(),
                                                                format!("Model error: {e}"),
                                                            ));
                                                            self.save_chat_history().await;
                                                        }
                                                    }
                                                }
                                                None => {
                                                    self.chat_history.push((
                                                        "System".to_string(),
                                                        "".into(),
                                                        "Model router belum dikonfigurasi. Set API key via Ctrl+P → Set API Key".into(),
                                                    ));
                                                    self.save_chat_history().await;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Esc => {
                                if self.is_streaming {
                                    self.interrupt_agent();
                                } else if self.active_screen == Screen::Chat {
                                    // Reset workspace aktif → kembali ke Workspace Selector
                                    self.active_workspace = None;
                                    self.chat_history.clear();
                                    self.active_screen = Screen::Home;
                                    self.load_workspaces().await;
                                } else {
                                    self.should_quit = true;
                                }
                            }
                            KeyCode::Backspace => {
                                self.input_buffer.pop();
                                self.update_suggestions();
                            }
                            KeyCode::Char(c) => {
                                if self.active_screen == Screen::Home {
                                    match c {
                                        'c' | 'C' | 'i' | 'I' => {
                                            self.active_screen = Screen::Chat;
                                        }
                                        _ => {
                                            self.input_buffer.push(c);
                                            self.active_screen = Screen::Chat;
                                            self.update_suggestions();
                                        }
                                    }
                                } else {
                                    self.input_buffer.push(c);
                                    self.update_suggestions();
                                }
                            }
                            KeyCode::Tab => {
                                self.selected_tab = match self.selected_tab {
                                    Tab::Session => Tab::Agents,
                                    Tab::Agents => Tab::Workers,
                                    Tab::Workers => Tab::SpawnRequests,
                                    Tab::SpawnRequests => Tab::Missions,
                                    Tab::Missions => Tab::Tasks,
                                    Tab::Tasks => Tab::Memory,
                                    Tab::Memory => Tab::Approvals,
                                    Tab::Approvals => Tab::Costs,
                                    Tab::Costs => Tab::Policies,
                                    Tab::Policies => Tab::Skills,
                                    Tab::Skills => Tab::Artifacts,
                                    Tab::Artifacts => Tab::Incidents,
                                    Tab::Incidents => Tab::Session,
                                };
                                self.selected_index = 0;
                                self.active_screen = crate::app::TuiApp::screen_for_tab(self.selected_tab);
                            }
                            KeyCode::Up => {
                                if self.active_screen == Screen::Chat && !self.active_suggestions.is_empty() {
                                    if self.suggestion_index > 0 {
                                        self.suggestion_index -= 1;
                                    } else {
                                        self.suggestion_index = self.active_suggestions.len().saturating_sub(1);
                                    }
                                } else if self.active_screen == Screen::Chat {
                                    // Scroll chat ke atas (3 baris per langkah)
                                    let next = self.chat_scroll_offset.get().saturating_add(3);
                                    self.chat_scroll_offset.set(next);
                                    self.chat_at_bottom = false;
                                } else if self.selected_index > 0 {
                                    self.selected_index -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if self.active_screen == Screen::Chat && !self.active_suggestions.is_empty() {
                                    let max = self.active_suggestions.len().saturating_sub(1);
                                    if self.suggestion_index < max {
                                        self.suggestion_index += 1;
                                    } else {
                                        self.suggestion_index = 0;
                                    }
                                } else if self.active_screen == Screen::Chat {
                                    // Scroll chat ke bawah (3 baris per langkah)
                                    let current = self.chat_scroll_offset.get();
                                    if current >= 3 {
                                        self.chat_scroll_offset.set(current - 3);
                                    } else {
                                        self.chat_scroll_offset.set(0);
                                        self.chat_at_bottom = true;
                                    }
                                } else {
                                    let max = self.current_list_len().saturating_sub(1);
                                    if self.selected_index < max {
                                        self.selected_index += 1;
                                    }
                                }
                            }
                            KeyCode::PageUp => {
                                if self.active_screen == Screen::Chat {
                                    let next = self.chat_scroll_offset.get().saturating_add(20);
                                    self.chat_scroll_offset.set(next);
                                    self.chat_at_bottom = false;
                                }
                            }
                            KeyCode::PageDown => {
                                if self.active_screen == Screen::Chat {
                                    let current = self.chat_scroll_offset.get();
                                    if current >= 20 {
                                        self.chat_scroll_offset.set(current - 20);
                                    } else {
                                        self.chat_scroll_offset.set(0);
                                        self.chat_at_bottom = true;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn update_palette_filter(&mut self) {
        if let CommandMode::CommandPalette { search_query, selected_index, filtered_items } = &mut self.command_mode {
            let query = search_query.to_lowercase();
            let filtered: Vec<(String, String, String, String)> = get_palette_items()
                .into_iter()
                .filter(|(_, name, _, _)| name.to_lowercase().contains(&query))
                .collect();
            *selected_index = 0;
            *filtered_items = filtered;
        }
    }

    pub(crate) async fn execute_palette_action(&mut self, action: &str) {
        match action {
            "/session_new" => {
                self.stop_streaming();
                self.chat_history.clear();
                self.active_screen = Screen::Home;
            }
            "/session_switch" => {
                self.stop_streaming();
                self.chat_history.clear();
                self.chat_history.push(("System".into(), "".into(), "Sesi baru berhasil dimuat.".into()));
                self.active_screen = Screen::Chat;
            }
            "/model_switch" => {
                self.reset_model_selection();
                self.model_sel_step = ModelSelectionStep::SelectProvider;
                self.command_mode = CommandMode::ModelSelection;
            }
            "/clear_all" => {
                self.clear_app_data().await;
            }
            "/session_share" => {
                self.chat_history.push(("System".into(), "".into(), "Tautan sesi berhasil disalin ke clipboard!".into()));
                self.active_screen = Screen::Chat;
            }
            "/session_rename" => {
                self.chat_history.push(("System".into(), "".into(), "Sesi berhasil diganti namanya.".into()));
                self.active_screen = Screen::Chat;
            }
            act if act.starts_with("/apikey") || act.starts_with("/connect") => {
                let prefix = if let Some(p) = act.strip_prefix("/apikey ").or_else(|| act.strip_prefix("/connect ")) {
                    format!("{p}:")
                } else {
                    String::new()
                };
                self.command_mode = CommandMode::ApiKeyInput {
                    key_input: prefix,
                    error_message: String::new(),
                };
            }
            _ => {}
        }
    }

    /// Process a single stream event, updating chat_history in place.
    pub(crate) async fn handle_stream_event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::TextDelta(delta) => {
                self.stream_status = Some("Menerima respon...".to_string());
                if let Some((_, _, content)) = self.chat_history.last_mut() {
                    content.push_str(&delta);
                }
                self.save_chat_history().await;
            }
            StreamEvent::ToolCallDelta { name, .. } => {
                let tool_name = name.clone().unwrap_or_else(|| "tool".to_string());
                self.stream_status = Some(format!("Menjalankan tool: {}...", tool_name));
            }
            StreamEvent::Usage(_usage) => {
                // Future: show token usage in status bar
            }
            StreamEvent::Done => {
                self.is_streaming = false;
                self.stream_status = None;
                self.stream_rx = None;
                self.agent_task = None;
                // Ensure non-empty response
                if let Some((_, _, content)) = self.chat_history.last_mut() {
                    if content.is_empty() {
                        content.push_str("(empty response)");
                    }
                }
                self.save_chat_history().await;
            }
            StreamEvent::Error(e) => {
                self.is_streaming = false;
                self.stream_status = None;
                self.stream_rx = None;
                self.agent_task = None;
                self.chat_history.push((
                    "System".to_string(),
                    String::new(),
                    format!("Stream error: {e}"),
                ));
                self.save_chat_history().await;
            }
        }
    }

    /// Non-blocking drain of all pending stream events before a draw.
    pub(crate) async fn try_flush_stream(&mut self) {
        let mut rx = match self.stream_rx.take() {
            Some(rx) => rx,
            None => return,
        };
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_millis(1),
                rx.recv(),
            )
            .await
            {
                Ok(Some(ev)) => {
                    self.handle_stream_event(ev).await;
                    if !self.is_streaming {
                        return; // rx already consumed by handle_stream_event
                    }
                }
                _ => break,
            }
        }
        // Put back the non-exhausted receiver
        self.stream_rx = Some(rx);
    }

    /// Stop any active streaming response.
    pub(crate) fn stop_streaming(&mut self) {
        self.is_streaming = false;
        self.stream_rx = None;
        self.agent_task = None;
    }

    /// Proses satu AgentEvent dan update chat_history + stream_status di TUI.
    pub(crate) async fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::SessionStarted { agent_id, .. } => {
                self.stream_status = Some(format!("Agent {} dimulai...", &agent_id[..8]));
            }
            AgentEvent::ModelCall { turn, .. } => {
                self.stream_status = Some(format!("Giliran {} — memanggil model...", turn + 1));
            }
            AgentEvent::TextDelta { delta, .. } => {
                self.stream_status = Some("Menerima respon...".to_string());
                if let Some((_, _, chat_content)) = self.chat_history.last_mut() {
                    chat_content.push_str(&delta);
                }
                self.save_chat_history().await;
            }
            AgentEvent::Thought { content, .. } => {
                // Sinkronisasi absolut thought di akhir turn agar rapi
                self.stream_status = Some("Berpikir...".to_string());
                if let Some((_, _, chat_content)) = self.chat_history.last_mut() {
                    *chat_content = content;
                }
                self.save_chat_history().await;
            }

            AgentEvent::ToolCall { tool, args, result } => {
                if result.is_null() {
                    // Event "sedang memanggil tool"
                    self.stream_status = Some(format!("Menjalankan tool: {}...", tool));
                    self.chat_history.push((
                        "Tool".to_string(),
                        tool.clone(),
                        format!("▶ {}\n{}", tool, format_tool_args(&tool, &args)),
                    ));
                } else {
                    // Event "tool selesai"
                    self.stream_status = Some(format!("Tool {} selesai.", tool));
                    self.chat_history.push((
                        "Tool".to_string(),
                        tool.clone(),
                        format!("✓ {} selesai:\n{}", tool, format_tool_result(&tool, &result)),
                    ));
                }
                self.save_chat_history().await;
            }
            AgentEvent::ObjectiveComplete { summary, .. } => {
                // Pastikan summary tampil di bubble agent (mungkin sudah dari Thought)
                self.stream_status = None;
                self.is_streaming = false;
                self.agent_rx = None;
                self.agent_task = None;
                // Reset active_agent_id agar sesi berikutnya buat agent baru
                self.active_agent_id = None;
                if let Some((_, _, content)) = self.chat_history.last_mut() {
                    if content.is_empty() {
                        content.push_str(&summary);
                    }
                }
                self.save_chat_history().await;
            }
            AgentEvent::BudgetWarning { remaining } => {
                self.stream_status = Some(format!("⚠ Budget tersisa: ${:.4}", remaining));
                self.chat_history.push((
                    "System".to_string(),
                    String::new(),
                    format!("⚠ Peringatan budget: sisa ${:.4}", remaining),
                ));
                self.save_chat_history().await;
            }
            AgentEvent::SessionPaused { reason } => {
                self.stream_status = Some(format!("Agent dijeda: {}", reason));
                self.is_streaming = false;
                self.agent_rx = None;
                self.agent_task = None;
                self.active_agent_id = None;
                self.save_chat_history().await;
            }
            AgentEvent::SessionTerminated { reason } => {
                self.stream_status = None;
                self.is_streaming = false;
                self.agent_rx = None;
                self.agent_task = None;
                self.active_agent_id = None;
                self.chat_history.push((
                    "System".to_string(),
                    String::new(),
                    format!("Agent dihentikan: {}", reason),
                ));
                self.save_chat_history().await;
            }
            AgentEvent::Error { message } => {
                self.stream_status = None;
                self.is_streaming = false;
                self.agent_rx = None;
                self.agent_task = None;
                self.active_agent_id = None;
                self.chat_history.push((
                    "System".to_string(),
                    String::new(),
                    format!("Error agent: {}", message),
                ));
                self.save_chat_history().await;
            }
        }
    }

    /// Non-blocking drain semua AgentEvent yang pending sebelum render.
    pub(crate) async fn try_flush_agent_events(&mut self) {
        let mut rx = match self.agent_rx.take() {
            Some(rx) => rx,
            None => return,
        };
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_millis(1),
                rx.recv(),
            )
            .await
            {
                Ok(Some(ev)) => {
                    self.handle_agent_event(ev).await;
                    if !self.is_streaming {
                        return; // agent selesai, rx sudah di-drop
                    }
                }
                _ => break,
            }
        }
        // Kembalikan receiver jika agent masih berjalan
        if self.is_streaming {
            self.agent_rx = Some(rx);
        }
    }

    pub(crate) async fn handle_tool_approval(&mut self, new_state: claw10_domain::approval::ToolApprovalState) {
        use claw10_store::StoreExt;

        if let Some(ref req) = self.pending_tool_approval {
            let key = format!("tool_approval:{}", req.id);
            let mut updated_req = req.clone();
            updated_req.state = new_state.clone();
            
            if self.state.kv_store.set(&key, &updated_req).await.is_ok() {
                self.chat_history.push((
                    "System".to_string(),
                    "".to_string(),
                    format!("Persetujuan tool: {:?}", new_state),
                ));
                self.pending_tool_approval = None;
                self.refresh().await;
            }
        }
    }
}

fn format_tool_args(_tool: &str, args: &serde_json::Value) -> String {
    match args {
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            let mut parts = Vec::new();
            for (k, v) in map {
                let val_str = match v {
                    serde_json::Value::String(s) => {
                        if s.contains('\n') {
                            format!("{}: \"\"\"\n{}\n\"\"\"", k, s)
                        } else {
                            format!("{}: {:?}", k, s)
                        }
                    }
                    _ => format!("{}: {}", k, v),
                };
                parts.push(val_str);
            }
            if parts.len() == 1 && !parts[0].contains('\n') {
                format!("{{{}}}", parts[0])
            } else {
                format!("{{\n  {}\n}}", parts.join(",\n  ").replace('\n', "\n  "))
            }
        }
        _ => args.to_string(),
    }
}

fn format_tool_result(_tool: &str, result: &serde_json::Value) -> String {
    match result {
        serde_json::Value::Object(map) => {
            if map.contains_key("exit_code") || map.contains_key("stdout") || map.contains_key("stderr") {
                let exit_code = map.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(0);
                let stdout = map.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
                let stderr = map.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                
                let mut out = format!("exit_code: {exit_code}");
                if !stdout.is_empty() {
                    out.push_str(&format!("\nstdout:\n{}", stdout));
                }
                if !stderr.is_empty() {
                    out.push_str(&format!("\nstderr:\n{}", stderr));
                }
                return out;
            }
            
            if map.contains_key("content") && map.len() == 1 {
                if let Some(content) = map.get("content").and_then(|v| v.as_str()) {
                    if content.len() > 1000 {
                        return format!(
                            "content (truncated):\n{}...\n[+{} bytes]", 
                            &content[..1000], 
                            content.len() - 1000
                        );
                    } else {
                        return format!("content:\n{}", content);
                    }
                }
            }
            
            let mut parts = Vec::new();
            for (k, v) in map {
                let val_str = match v {
                    serde_json::Value::String(s) => {
                        if s.contains('\n') {
                            format!("{}:\n{}", k, s)
                        } else {
                            format!("{}: {:?}", k, s)
                        }
                    }
                    _ => format!("{}: {}", k, v),
                };
                parts.push(val_str);
            }
            parts.join("\n")
        }
        serde_json::Value::String(s) => s.clone(),
        _ => result.to_string(),
    }
}
