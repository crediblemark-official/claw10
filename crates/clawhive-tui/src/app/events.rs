use std::sync::Arc;

use crossterm::event::{Event, KeyCode, KeyEventKind};

use clawhive_model_router::types::{ChatRequest, MessageRole, ModelMessage, StreamEvent};
use crate::app::{CommandMode, ModelSelectionStep, Screen, Tab, TuiApp};
use crate::app::palette::{get_palette_items, provider_api_key_env};

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
                                        let env_var = provider_api_key_env(&provider);
                                        unsafe { std::env::set_var(&env_var, &actual_key) };
                                        self.register_all_providers().await;
                                        self.persist_api_key(&provider, &actual_key).await;
                                        let model_count = self.state.model_router.as_ref()
                                            .and_then(|r| Some(r.registry().list_profiles().len()))
                                            .unwrap_or(0);
                                        self.status_message = format!(
                                            "{provider} API key set. {model_count} model(s) available."
                                        );
                                        self.model_sel_pending_provider = None;
                                        self.model_sel_provider = provider.clone();
                                        self.advance_to_model_list().await;
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
                                        let catalog = clawhive_model_router::providers::provider_configs();
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
                                        let env_var = provider_api_key_env(&provider);
                                        unsafe { std::env::set_var(&env_var, &actual_key) };
                                        self.register_all_providers().await;
                                        self.persist_api_key(&provider, &actual_key).await;
                                        let model_count = self.state.model_router.as_ref()
                                            .and_then(|r| Some(r.registry().list_profiles().len()))
                                            .unwrap_or(0);
                                        self.status_message = format!(
                                            "{provider} API key set. {model_count} model(s) available."
                                        );
                                        self.command_mode = CommandMode::None;
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
                        // Standard Input / Navigation handling
                        match key.code {
                            KeyCode::Enter => {
                                let content = std::mem::take(&mut self.input_buffer);
                                let trimmed = content.trim();
                                if !trimmed.is_empty() {
                                    if let Some(cmd) = trimmed.strip_prefix(':').or_else(|| trimmed.strip_prefix('/')) {
                                        self.execute_command(cmd).await;
                                    } else {
                                        self.chat_history.push(("User".to_string(), "".to_string(), trimmed.to_string()));
                                        self.active_screen = Screen::Chat;
                                        // Reset scroll ke bawah agar respons baru langsung terlihat
                                        self.chat_scroll_offset = 0;
                                        self.chat_at_bottom = true;

                                        let router_opt = self
                                            .state
                                            .model_router
                                            .as_ref()
                                            .map(Arc::clone);
                                        match router_opt {
                                            Some(router) => {
                                                let model_id = self.active_model.clone();
                                                let request = ChatRequest {
                                                    model: model_id.clone(),
                                                    messages: vec![ModelMessage {
                                                        role: MessageRole::User,
                                                        content: trimmed.to_string(),
                                                        tool_calls: None,
                                                        tool_call_id: None,
                                                        name: None,
                                                    }],
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
                                                        self.is_streaming = true;

                                                        let (stream_tx, stream_rx) =
                                                            tokio::sync::mpsc::unbounded_channel();
                                                        self.stream_rx = Some(stream_rx);

                                                        tokio::spawn(async move {
                                                            while let Some(event) = handle.recv().await {
                                                                if stream_tx.send(event).is_err() {
                                                                    break;
                                                                }
                                                            }
                                                        });
                                                    }
                                                    Err(e) => {
                                                        self.chat_history.push((
                                                            "System".to_string(),
                                                            "".into(),
                                                            format!("Model error: {e}"),
                                                        ));
                                                    }
                                                }
                                            }
                                            None => {
                                                self.chat_history.push((
                                                    "System".to_string(),
                                                    "".into(),
                                                    "Model router not configured. Set API key via Ctrl+P → Set API Key".into(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Esc => {
                                if self.active_screen == Screen::Chat {
                                    self.active_screen = Screen::Home;
                                } else {
                                    self.should_quit = true;
                                }
                            }
                            KeyCode::Backspace => {
                                self.input_buffer.pop();
                            }
                            KeyCode::Char(c) => {
                                self.input_buffer.push(c);
                            }
                            KeyCode::Tab => {
                                self.selected_tab = match self.selected_tab {
                                    Tab::Session => Tab::Agents,
                                    Tab::Agents => Tab::Workers,
                                    Tab::Workers => Tab::SpawnRequests,
                                    Tab::SpawnRequests => Tab::Session,
                                };
                                self.selected_index = 0;
                            }
                            KeyCode::Up => {
                                if self.active_screen == Screen::Chat {
                                    // Scroll chat ke atas (3 baris per langkah)
                                    self.chat_scroll_offset = self.chat_scroll_offset.saturating_add(3);
                                    self.chat_at_bottom = false;
                                } else if self.selected_index > 0 {
                                    self.selected_index -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if self.active_screen == Screen::Chat {
                                    // Scroll chat ke bawah (3 baris per langkah)
                                    if self.chat_scroll_offset >= 3 {
                                        self.chat_scroll_offset -= 3;
                                    } else {
                                        self.chat_scroll_offset = 0;
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
                                    self.chat_scroll_offset = self.chat_scroll_offset.saturating_add(20);
                                    self.chat_at_bottom = false;
                                }
                            }
                            KeyCode::PageDown => {
                                if self.active_screen == Screen::Chat {
                                    if self.chat_scroll_offset >= 20 {
                                        self.chat_scroll_offset -= 20;
                                    } else {
                                        self.chat_scroll_offset = 0;
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
    pub(crate) fn handle_stream_event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::TextDelta(delta) => {
                if let Some((_, _, content)) = self.chat_history.last_mut() {
                    content.push_str(&delta);
                }
            }
            StreamEvent::ToolCallDelta { .. } => {
                // Future: render tool call state
            }
            StreamEvent::Usage(_usage) => {
                // Future: show token usage in status bar
            }
            StreamEvent::Done => {
                self.is_streaming = false;
                self.stream_rx = None;
                // Ensure non-empty response
                if let Some((_, _, content)) = self.chat_history.last_mut() {
                    if content.is_empty() {
                        content.push_str("(empty response)");
                    }
                }
            }
            StreamEvent::Error(e) => {
                self.is_streaming = false;
                self.stream_rx = None;
                self.chat_history.push((
                    "System".to_string(),
                    String::new(),
                    format!("Stream error: {e}"),
                ));
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
                    self.handle_stream_event(ev);
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
    }
}
