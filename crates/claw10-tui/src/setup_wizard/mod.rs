use std::path::PathBuf;
use std::collections::HashMap;

use crate::setup_service::{ProviderOption, BindingEvent};

mod draw;
mod handlers;

/// Build provider list dynamically from models.dev catalog + Custom Provider.
fn build_providers() -> Vec<ProviderOption> {
    let catalog = claw10_model_router::providers::provider_configs();
    let mut providers: Vec<ProviderOption> = catalog.into_iter().map(|c| ProviderOption {
        name: c.notes.clone(),
        slot: c.name.clone(),
        env_var: c.api_key_env.clone(),
        base_url: c.base_url.clone(),
    }).collect();
    providers.push(ProviderOption {
        name: "Custom Provider".into(),
        slot: "custom".into(),
        env_var: "CUSTOM_API_KEY".into(),
        base_url: String::new(),
    });
    providers
}

pub struct SetupWizard {
    pub(crate) step: Step,
    pub(crate) providers: Vec<ProviderOption>,
    pub(crate) selected: usize,
    pub(crate) api_key: String,
    pub(crate) custom_model: String,
    pub(crate) custom_url: String,
    pub(crate) config_path: PathBuf,
    pub(crate) error_msg: String,
    pub(crate) scroll: usize,
    pub(crate) fetched_models: Vec<String>,
    pub(crate) static_models: Vec<String>,
    pub(crate) model_search: String,
    pub(crate) model_list_selected: usize,
    pub(crate) fetch_failed: bool,
    pub(crate) setup_telegram: bool,
    pub(crate) telegram_token: String,
    pub(crate) telegram_chat_id: String,
    pub(crate) binding_code: String,
    pub(crate) binding_status: String,
    pub(crate) binding_rx: Option<std::sync::mpsc::Receiver<BindingEvent>>,
    pub(crate) configured_env_vars: HashMap<String, String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Step {
    Welcome,
    ProviderSelect,
    ApiKeyInput,
    BaseUrlInput,
    ModelFetch,
    ModelList,
    ModelSelect,
    TelegramSetupPrompt,
    TelegramTokenInput,
    TelegramBindingWait,
    Review,
    Complete,
}

impl SetupWizard {
    pub fn new(config_path: PathBuf) -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let env_path = std::path::PathBuf::from(&home).join(".claw10").join(".env");
        let mut configured_env_vars = HashMap::new();
        if let Ok(content) = std::fs::read_to_string(&env_path) {
            for line in content.lines() {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let val = parts[1].trim();
                    if !val.is_empty() {
                        configured_env_vars.insert(key.to_string(), val.to_string());
                    }
                }
            }
        }

        let telegram_token = configured_env_vars.get("TELEGRAM_BOT_TOKEN").cloned().unwrap_or_default();
        let telegram_chat_id = configured_env_vars.get("TELEGRAM_CHAT_ID").cloned().unwrap_or_default();
        let setup_telegram = !telegram_token.is_empty();

        Self {
            step: Step::Welcome,
            providers: build_providers(),
            selected: 0,
            api_key: String::new(),
            custom_model: String::new(),
            custom_url: String::new(),
            config_path,
            error_msg: String::new(),
            scroll: 0,
            fetched_models: Vec::new(),
            static_models: Vec::new(),
            model_search: String::new(),
            model_list_selected: 0,
            fetch_failed: false,
            setup_telegram,
            telegram_token,
            telegram_chat_id,
            binding_code: String::new(),
            binding_status: String::new(),
            binding_rx: None,
            configured_env_vars,
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        use crossterm::ExecutableCommand;
        stdout.execute(crossterm::terminal::EnterAlternateScreen)?;

        let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
        terminal.clear()?;

        let result = self.run_loop(&mut terminal);

        crossterm::terminal::disable_raw_mode()?;
        let mut stdout = std::io::stdout();
        stdout.execute(crossterm::terminal::LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn run_loop(&mut self, terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            // Auto-fetch saat masuk ke ModelFetch step
            if matches!(self.step, Step::ModelFetch) {
                self.do_fetch_models();
                if self.fetch_failed {
                    self.step = Step::ModelSelect;
                } else {
                    self.step = Step::ModelList;
                }
                continue;
            }

            // Check async binding events
            if let Step::TelegramBindingWait = self.step {
                if let Some(ref rx) = self.binding_rx {
                    if let Ok(event) = rx.try_recv() {
                        match event {
                            BindingEvent::ChatDetected { username, chat_id } => {
                                self.telegram_chat_id = chat_id;
                                self.binding_status = format!(
                                    "Ditemukan chat dari @{username}! Kirim kode verifikasi ke bot: {}",
                                    self.binding_code
                                );
                            }
                            BindingEvent::CodeMatched { chat_id } => {
                                self.telegram_chat_id = chat_id;
                                self.binding_status = "Koneksi bot Telegram sukses terverifikasi!".to_string();
                                terminal.draw(|f| self.draw(f))?;
                                std::thread::sleep(std::time::Duration::from_millis(1500));
                                self.next_step();
                            }
                            BindingEvent::Error(err) => {
                                self.error_msg = err;
                            }
                        }
                    }
                }
            }

            // Non-blocking keyboard event poll
            if crossterm::event::poll(std::time::Duration::from_millis(100))? {
                let event = crossterm::event::read()?;
                if let crossterm::event::Event::Key(key) = event {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        match self.step {
                            Step::Welcome => self.handle_welcome(key),
                            Step::ProviderSelect => self.handle_provider_select(key),
                            Step::ApiKeyInput => self.handle_api_key_input(key),
                            Step::BaseUrlInput => self.handle_base_url_input(key),
                            Step::ModelList => self.handle_model_list(key),
                            Step::ModelSelect => self.handle_model_select(key),
                            Step::TelegramSetupPrompt => self.handle_telegram_setup_prompt(key),
                            Step::TelegramTokenInput => self.handle_telegram_token_input(key),
                            Step::TelegramBindingWait => {
                                if key.code == crossterm::event::KeyCode::Esc {
                                    self.binding_rx = None;
                                    self.prev_step();
                                } else if key.code == crossterm::event::KeyCode::Enter && !self.telegram_chat_id.is_empty() {
                                    self.next_step();
                                }
                            }
                            Step::Review => self.handle_review(key),
                            Step::Complete => return Ok(()),
                            Step::ModelFetch => {}
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn do_fetch_models(&mut self) {
        self.static_models = self.load_static_models();

        let provider = self.current_provider();
        let base_url = if provider.base_url.is_empty() {
            self.custom_url.clone()
        } else {
            provider.base_url.to_string()
        };
        let api_key = self.api_key.clone();

        if base_url.is_empty() || api_key.is_empty() {
            if !self.static_models.is_empty() {
                self.fetched_models = self.static_models.clone();
                self.error_msg = "Gunakan daftar model statis (API key tidak tersedia).".to_string();
            } else {
                self.error_msg = "Tidak ada daftar model. Input manual.".to_string();
                self.fetch_failed = true;
            }
            return;
        }

        let handle = std::thread::spawn(move || {
            crate::setup_service::fetch_provider_models(&base_url, &api_key)
        });

        match handle.join() {
            Ok(Ok(models)) => {
                self.fetched_models = models;
                self.model_search.clear();
                self.model_list_selected = 0;
            }
            Ok(Err(e)) => {
                self.error_msg = format!("Gagal fetch API: {e}.");
                self.fallback_to_static();
            }
            Err(_) => {
                self.error_msg = "Thread fetch panic.".to_string();
                self.fallback_to_static();
            }
        }
    }

    pub(crate) fn fallback_to_static(&mut self) {
        if !self.static_models.is_empty() {
            self.fetched_models = self.static_models.clone();
            self.error_msg.push_str(" Gunakan daftar statis.");
        } else {
            self.fetch_failed = true;
            self.error_msg.push_str(" Input manual.");
        }
    }

    pub(crate) fn filtered_models(&self) -> Vec<&str> {
        if self.model_search.is_empty() {
            return self.fetched_models.iter().map(|s| s.as_str()).collect();
        }
        let q = self.model_search.to_lowercase();
        self.fetched_models
            .iter()
            .filter(|m| m.to_lowercase().contains(&q))
            .map(|s| s.as_str())
            .collect()
    }

    pub(crate) fn next_step(&mut self) {
        let is_custom = self.current_provider().slot == "custom";
        self.step = match self.step {
            Step::Welcome => Step::ProviderSelect,
            Step::ProviderSelect => Step::ApiKeyInput,
            Step::ApiKeyInput => {
                if is_custom { Step::BaseUrlInput } else { Step::ModelFetch }
            }
            Step::BaseUrlInput => Step::ModelFetch,
            Step::ModelList | Step::ModelSelect => Step::TelegramSetupPrompt,
            Step::TelegramSetupPrompt => {
                if self.setup_telegram {
                    Step::TelegramTokenInput
                } else {
                    Step::Review
                }
            }
            Step::TelegramTokenInput => {
                self.start_telegram_binding_poll();
                Step::TelegramBindingWait
            }
            Step::TelegramBindingWait => Step::Review,
            Step::Review => Step::Complete,
            _ => Step::Complete,
        };
        self.error_msg.clear();
        self.scroll = 0;
    }

    pub(crate) fn prev_step(&mut self) {
        self.step = match self.step {
            Step::Welcome => Step::Welcome,
            Step::ProviderSelect => Step::Welcome,
            Step::ApiKeyInput => Step::ProviderSelect,
            Step::BaseUrlInput => Step::ApiKeyInput,
            Step::ModelList
            | Step::ModelFetch
            | Step::ModelSelect => {
                if self.current_provider().slot == "custom" { Step::BaseUrlInput } else { Step::ApiKeyInput }
            }
            Step::TelegramSetupPrompt => {
                if !self.fetched_models.is_empty() {
                    Step::ModelList
                } else {
                    Step::ModelSelect
                }
            }
            Step::TelegramTokenInput => Step::TelegramSetupPrompt,
            Step::TelegramBindingWait => Step::TelegramTokenInput,
            Step::Review => {
                if self.setup_telegram {
                    Step::TelegramBindingWait
                } else {
                    Step::TelegramSetupPrompt
                }
            }
            Step::Complete => Step::Complete,
        };
        self.error_msg.clear();
        self.scroll = 0;
    }

    pub(crate) fn current_provider(&self) -> &ProviderOption {
        &self.providers[self.selected]
    }

    pub(crate) fn load_static_models(&self) -> Vec<String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let cache_file = std::path::PathBuf::from(&home).join(".claw10").join("models.json");
        let slot = &self.current_provider().slot;

        if cache_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&cache_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(arr) = json.get(slot).and_then(|v| v.as_array()) {
                        let models: Vec<String> = arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        if !models.is_empty() {
                            return models;
                        }
                    }
                }
            }
        }

        Vec::new()
    }

    pub(crate) fn save_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = self.current_provider();
        
        let model_string;
        let model = if self.custom_model.is_empty() {
            let preferred = match provider.slot.as_str() {
                "nvidia" => "meta/llama-3.3-70b-instruct",
                "openai" => "gpt-4o",
                "anthropic" => "claude-3-5-sonnet-latest",
                "google-gemini" => "gemini-1.5-flash",
                "deepseek" => "deepseek-chat",
                "groq" => "llama-3.3-70b-versatile",
                "ollama" => "llama3",
                _ => "",
            };
            if !preferred.is_empty() && self.fetched_models.iter().any(|m| m == preferred) {
                preferred
            } else if let Some(m) = self.fetched_models.first() {
                m.as_str()
            } else {
                let static_list = self.load_static_models();
                if let Some(first_static) = static_list.first() {
                    model_string = first_static.clone();
                    &model_string
                } else {
                    ""
                }
            }
        } else {
            self.custom_model.as_str()
        };

        let telegram_token = if self.setup_telegram { self.telegram_token.as_str() } else { "" };
        let telegram_chat_id = if self.setup_telegram { self.telegram_chat_id.as_str() } else { "" };

        crate::setup_service::save_config_to_disk(
            &self.config_path,
            provider,
            model,
            &self.custom_url,
            &self.api_key,
            telegram_token,
            telegram_chat_id,
        )
    }
}
