use std::path::PathBuf;
use std::collections::HashMap;

use crossterm::event::{read, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, List, ListItem};
use ratatui::Frame;

use crate::setup_service::{ProviderOption, BindingEvent};

const PROVIDERS: &[ProviderOption] = &[
    ProviderOption { name: "OpenAI", slot: "openai", env_var: "OPENAI_API_KEY", base_url: "https://api.openai.com/v1" },
    ProviderOption { name: "Anthropic", slot: "anthropic", env_var: "ANTHROPIC_API_KEY", base_url: "https://api.anthropic.com/v1" },
    ProviderOption { name: "Google Gemini", slot: "google-gemini", env_var: "GEMINI_API_KEY", base_url: "https://generativelanguage.googleapis.com/v1beta/openai" },
    ProviderOption { name: "DeepSeek", slot: "deepseek", env_var: "DEEPSEEK_API_KEY", base_url: "https://api.deepseek.com" },
    ProviderOption { name: "Mistral AI", slot: "mistral", env_var: "MISTRAL_API_KEY", base_url: "https://api.mistral.ai/v1" },
    ProviderOption { name: "Cohere", slot: "cohere", env_var: "COHERE_API_KEY", base_url: "https://api.cohere.com/v1" },
    ProviderOption { name: "Groq", slot: "groq", env_var: "GROQ_API_KEY", base_url: "https://api.groq.com/openai/v1" },
    ProviderOption { name: "Perplexity", slot: "perplexity", env_var: "PERPLEXITY_API_KEY", base_url: "https://api.perplexity.ai" },
    ProviderOption { name: "xAI", slot: "xai", env_var: "XAI_API_KEY", base_url: "https://api.x.ai/v1" },
    ProviderOption { name: "NVIDIA NIM", slot: "nvidia", env_var: "NVIDIA_API_KEY", base_url: "https://integrate.api.nvidia.com/v1" },
    ProviderOption { name: "Together AI", slot: "together", env_var: "TOGETHER_API_KEY", base_url: "https://api.together.xyz/v1" },
    ProviderOption { name: "Fireworks AI", slot: "fireworks", env_var: "FIREWORKS_API_KEY", base_url: "https://api.fireworks.ai/inference/v1" },
    ProviderOption { name: "Ollama (Local)", slot: "ollama", env_var: "OLLAMA_API_KEY", base_url: "http://localhost:11434/v1" },
    ProviderOption { name: "OpenRouter", slot: "openrouter", env_var: "OPENROUTER_API_KEY", base_url: "https://openrouter.ai/api/v1" },
    ProviderOption { name: "Custom Provider", slot: "custom", env_var: "CUSTOM_API_KEY", base_url: "" },
];

pub struct SetupWizard {
    step: Step,
    providers: Vec<ProviderOption>,
    selected: usize,
    api_key: String,
    custom_model: String,
    custom_url: String,
    config_path: PathBuf,
    error_msg: String,
    scroll: usize,
    fetched_models: Vec<String>,
    static_models: Vec<String>,
    model_search: String,
    model_list_selected: usize,
    fetch_failed: bool,
    setup_telegram: bool,
    telegram_token: String,
    telegram_chat_id: String,
    binding_code: String,
    binding_status: String,
    binding_rx: Option<std::sync::mpsc::Receiver<BindingEvent>>,
    configured_env_vars: HashMap<String, String>,
}

enum Step {
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
            providers: PROVIDERS.to_vec(),
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
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        stdout.execute(EnterAlternateScreen)?;

        let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
        terminal.clear()?;

        let result = self.run_loop(&mut terminal);

        disable_raw_mode()?;
        terminal.backend_mut().execute(LeaveAlternateScreen)?;
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
                let event = read()?;
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
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
                                if key.code == KeyCode::Esc {
                                    self.binding_rx = None;
                                    self.prev_step();
                                } else if key.code == KeyCode::Enter && !self.telegram_chat_id.is_empty() {
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

    fn do_fetch_models(&mut self) {
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

    fn fallback_to_static(&mut self) {
        if !self.static_models.is_empty() {
            self.fetched_models = self.static_models.clone();
            self.error_msg.push_str(" Gunakan daftar statis.");
        } else {
            self.fetch_failed = true;
            self.error_msg.push_str(" Input manual.");
        }
    }

    fn filtered_models(&self) -> Vec<&str> {
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

    fn next_step(&mut self) {
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

    fn prev_step(&mut self) {
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

    fn current_provider(&self) -> &ProviderOption {
        &self.providers[self.selected]
    }

    fn load_static_models(&self) -> Vec<String> {
        use claw10_model_router::models;
        let slot = self.current_provider().slot;
        match slot {
            "openai" => models::openai::MODELS.iter().map(|s| s.to_string()).collect(),
            "anthropic" => models::anthropic::MODELS.iter().map(|s| s.to_string()).collect(),
            "groq" => models::groq::MODELS.iter().map(|s| s.to_string()).collect(),
            "openrouter" => models::openrouter::MODELS.iter().map(|s| s.to_string()).collect(),
            "nvidia" => models::nvidia::MODELS.iter().map(|s| s.to_string()).collect(),
            "deepseek" => models::deepseek::MODELS.iter().map(|s| s.to_string()).collect(),
            "gemini" | "google-gemini" => models::gemini::MODELS.iter().map(|s| s.to_string()).collect(),
            "mistral" => models::mistral::MODELS.iter().map(|s| s.to_string()).collect(),
            "together" => models::together::MODELS.iter().map(|s| s.to_string()).collect(),
            "fireworks" => models::fireworks::MODELS.iter().map(|s| s.to_string()).collect(),
            "perplexity" => models::perplexity::MODELS.iter().map(|s| s.to_string()).collect(),
            "xai" => models::xai::MODELS.iter().map(|s| s.to_string()).collect(),
            "cohere" => models::cohere::MODELS.iter().map(|s| s.to_string()).collect(),
            "ollama" => models::ollama::MODELS.iter().map(|s| s.to_string()).collect(),
            _ => Vec::new(),
        }
    }

    fn handle_welcome(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.next_step(),
            KeyCode::Esc | KeyCode::Char('q') => self.step = Step::Complete,
            _ => {}
        }
    }

    fn handle_provider_select(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.providers.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected = self.selected.saturating_sub(5);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let next = self.selected + 5;
                if next < self.providers.len() {
                    self.selected = next;
                }
            }
            KeyCode::Enter => {
                let provider = self.current_provider();
                if let Some(val) = self.configured_env_vars.get(provider.env_var) {
                    self.api_key = val.clone();
                } else {
                    self.api_key.clear();
                }
                self.next_step();
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Tab | KeyCode::Char('s') | KeyCode::Char('S') => {
                self.step = Step::Review;
            }
            _ => {}
        }
    }

    fn handle_api_key_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.api_key.is_empty() {
                    self.error_msg = "API key kosong. Lewati? Tekan Esc untuk kembali.".to_string();
                }
                self.next_step();
            }
            KeyCode::Esc => {
                self.prev_step();
            }
            KeyCode::Backspace => {
                self.api_key.pop();
                self.error_msg.clear();
            }
            KeyCode::Char(c) => {
                self.api_key.push(c);
                self.error_msg.clear();
            }
            _ => {}
        }
    }

    fn handle_base_url_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.custom_url.is_empty() {
                    self.error_msg = "Base URL tidak boleh kosong.".to_string();
                } else {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Backspace => {
                self.custom_url.pop();
                self.error_msg.clear();
            }
            KeyCode::Char(c) => {
                self.custom_url.push(c);
                self.error_msg.clear();
            }
            _ => {}
        }
    }

    fn handle_model_list(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.model_list_selected > 0 {
                    self.model_list_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let filtered = self.filtered_models();
                if self.model_list_selected + 1 < filtered.len() {
                    self.model_list_selected += 1;
                }
            }
            KeyCode::Enter => {
                let filtered = self.filtered_models();
                if !self.model_search.is_empty() && filtered.is_empty() {
                    self.custom_model = self.model_search.clone();
                } else if !filtered.is_empty() {
                    self.custom_model = filtered[self.model_list_selected].to_string();
                }
                if !self.custom_model.is_empty() {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Backspace => {
                self.model_search.pop();
                self.model_list_selected = 0;
            }
            KeyCode::Char(c) => {
                self.model_search.push(c);
                self.model_list_selected = 0;
            }
            KeyCode::Tab | KeyCode::F(2) => {
                self.error_msg = "Beralih ke input manual.".to_string();
                self.step = Step::ModelSelect;
            }
            _ => {}
        }
    }

    fn handle_model_select(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.custom_model.is_empty() {
                    self.error_msg = "Masukkan nama model.".to_string();
                } else {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Backspace => {
                self.custom_model.pop();
                self.error_msg.clear();
            }
            KeyCode::Char(c) => {
                self.custom_model.push(c);
                self.error_msg.clear();
            }
            _ => {}
        }
    }

    fn handle_telegram_setup_prompt(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.setup_telegram = true;
                self.next_step();
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.setup_telegram = false;
                self.next_step();
            }
            KeyCode::Enter => {
                self.next_step();
            }
            KeyCode::Esc => self.prev_step(),
            _ => {}
        }
    }

    fn handle_telegram_token_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.telegram_token.is_empty() {
                    self.error_msg = "Masukkan token bot Telegram atau tekan Esc.".to_string();
                } else {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Backspace => {
                self.telegram_token.pop();
                self.error_msg.clear();
            }
            KeyCode::Char(c) => {
                self.telegram_token.push(c);
                self.error_msg.clear();
            }
            _ => {}
        }
    }

    fn start_telegram_binding_poll(&mut self) {
        unsafe {
            std::env::set_var("TELEGRAM_CHAT_ID", "");
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.binding_rx = Some(rx);
        if !self.telegram_chat_id.is_empty() {
            self.binding_status = format!(
                "Sudah terhubung (Chat ID: {}). Tekan Enter untuk lanjut, atau kirim /start untuk pairing ulang.",
                self.telegram_chat_id
            );
        } else {
            self.binding_status = "Buka Telegram Anda, cari bot Anda, lalu kirim pesan /start...".to_string();
        }

        let token = self.telegram_token.clone();
        // Generate pseudo-random 6-digit code menggunakan timestamp nanos
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(123456);
        let code = format!("{:06}", (now % 1_000_000).abs());
        self.binding_code = code.clone();

        crate::setup_service::spawn_telegram_polling_thread(token, code, tx);
    }

    fn handle_review(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if let Err(e) = self.save_config() {
                    self.error_msg = format!("Gagal menyimpan: {e}");
                } else {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            _ => {}
        }
    }

    fn save_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = self.current_provider();
        
        let model_string;
        let model = if self.custom_model.is_empty() {
            if let Some(m) = self.fetched_models.first() {
                m.as_str()
            } else {
                let static_list = self.load_static_models();
                if let Some(first_static) = static_list.first() {
                    model_string = claw10_model_router::models::resolve_static_model(first_static, provider.slot);
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

        unsafe {
            std::env::set_var("TELEGRAM_BOT_TOKEN", telegram_token);
            std::env::set_var("TELEGRAM_CHAT_ID", telegram_chat_id);
            std::env::set_var(provider.env_var, &self.api_key);
        }

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

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        let wizard_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
            .title(" CLAW10 OS - Setup Wizard ")
            .title_style(Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));

        let inner_area = wizard_block.inner(area);
        frame.render_widget(wizard_block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(9),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(inner_area);

        self.draw_logo(frame, chunks[0]);
        self.draw_content(frame, chunks[1]);
        self.draw_footer(frame, chunks[2]);
    }

    fn draw_logo(&self, frame: &mut Frame, area: Rect) {
        let banner_content = include_str!("../../../assets/claw10.txt");
        let mut lines = vec![Line::from("")]; // Baris kosong pertama sebagai padding atas

        let mut banner_lines: Vec<Line> = banner_content
            .lines()
            .map(|line| {
                Line::from(vec![
                    Span::styled(line.to_string(), Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
                ])
            })
            .collect();
        lines.append(&mut banner_lines);

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Recursive Agent Swarm Operating System", Style::default().fg(Color::Rgb(150, 150, 150))),
        ]));

        let para = Paragraph::new(lines)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, area);
    }

    fn draw_content(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Rgb(50, 50, 50)))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        match self.step {
            Step::Welcome => self.draw_welcome(frame, inner),
            Step::ProviderSelect => self.draw_provider_select(frame, inner),
            Step::ApiKeyInput => self.draw_api_key_input(frame, inner),
            Step::BaseUrlInput => self.draw_base_url_input(frame, inner),
            Step::ModelFetch => self.draw_model_fetch(frame, inner),
            Step::ModelList => self.draw_model_list(frame, inner),
            Step::ModelSelect => self.draw_model_select(frame, inner),
            Step::TelegramSetupPrompt => self.draw_telegram_setup_prompt(frame, inner),
            Step::TelegramTokenInput => self.draw_telegram_token_input(frame, inner),
            Step::TelegramBindingWait => self.draw_telegram_binding_wait(frame, inner),
            Step::Review => self.draw_review(frame, inner),
            Step::Complete => self.draw_complete(frame, inner),
        }
    }

    fn draw_welcome(&self, frame: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from(vec![
                Span::styled("Selamat datang di Claw10 OS!", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Wizard ini akan membantu Anda mengatur:", Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  \u{2022} Provider model LLM", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("  \u{2022} API key", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("  \u{2022} Model default (auto-fetch dari API)", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Tekan Enter untuk memulai...", Style::default().fg(Color::Rgb(120, 120, 120))),
            ]),
        ];

        let welcome_height = lines.len() as u16;
        
        // Menengahkan secara vertikal
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(welcome_height),
                Constraint::Min(0),
            ])
            .split(area);

        // Menengahkan secara horizontal menggunakan card dengan lebar tetap 46
        let card_width = 46u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;
        
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical_chunks[1]);

        let para = Paragraph::new(lines)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, horizontal_chunks[1]);
    }

    fn draw_provider_select(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Spacer atas
                Constraint::Min(0),    // List area
            ])
            .split(area);

        // Padding horizontal (4 kolom di kiri dan kanan)
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(0),
                Constraint::Length(4),
            ])
            .split(chunks[1]);

        let list_area = horizontal_chunks[1];

        // 3 kolom, dengan 15 provider, maka rows_per_col adalah (15 + 2) / 3 = 5 baris.
        let cols = 3u16;
        let rows_per_col = 5u16;
        let col_width = list_area.width / cols;

        for (i, provider) in self.providers.iter().enumerate() {
            let col = i as u16 / rows_per_col;
            let row = i as u16 % rows_per_col;

            let item_x = list_area.x + col * col_width;
            let item_y = list_area.y + row;
            let is_selected = i == self.selected;

            let item_rect = Rect {
                x: item_x,
                y: item_y,
                width: col_width.saturating_sub(2),
                height: 1,
            };

            let is_configured = self.configured_env_vars.contains_key(provider.env_var);
            let text = if is_selected {
                let mut spans = vec![
                    Span::styled("\u{25B6} ", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
                    Span::styled(provider.name, Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
                ];
                if is_configured {
                    spans.push(Span::styled(" [config]", Style::default().fg(Color::Rgb(46, 139, 87)).add_modifier(Modifier::BOLD)));
                }
                Line::from(spans)
            } else {
                let mut spans = vec![
                    Span::styled(provider.name, Style::default().fg(Color::Rgb(160, 160, 160))),
                ];
                if is_configured {
                    spans.push(Span::styled(" [config]", Style::default().fg(Color::Rgb(46, 139, 87))));
                }
                Line::from(spans)
            };

            let para = Paragraph::new(text)
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().bg(if is_selected { Color::Rgb(25, 25, 25) } else { Color::Rgb(15, 15, 15) }));

            frame.render_widget(ratatui::widgets::Clear, item_rect);
            frame.render_widget(para, item_rect);
        }
    }

    fn draw_api_key_input(&self, frame: &mut Frame, area: Rect) {
        let provider = self.current_provider();

        // Menengahkan input box secara horizontal dengan lebar tetap 60
        let card_width = 60u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(area);

        let input_area = horizontal_chunks[1];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Spacer atas
                Constraint::Length(3), // Input box
                Constraint::Length(1), // Error msg
                Constraint::Min(0),
            ])
            .split(input_area);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(format!(" {} ", provider.env_var))
            .title_alignment(ratatui::layout::Alignment::Center);
        let input_inner = input_block.inner(chunks[1]);

        let display = if self.api_key.is_empty() {
            " (input hidden — ketik API key Anda)"
        } else {
            &"*".repeat(self.api_key.len().min(40))
        };
        let input_para = Paragraph::new(Line::from(vec![
            Span::styled(display, Style::default().fg(Color::Rgb(218, 165, 32))),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[1]);
        frame.render_widget(input_block, chunks[1]);
        frame.render_widget(input_para, input_inner);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[2]);
        }

        if !self.api_key.is_empty() {
            let text_len = self.api_key.len().min(40) as u16;
            let cursor_x = input_inner.x + (input_inner.width.saturating_sub(text_len) / 2) + text_len;
            frame.set_cursor_position((cursor_x.min(input_inner.x + input_inner.width.saturating_sub(1)), input_inner.y));
        }
    }

    fn draw_base_url_input(&self, frame: &mut Frame, area: Rect) {
        // Menengahkan input box secara horizontal dengan lebar tetap 60
        let card_width = 60u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(area);

        let input_area = horizontal_chunks[1];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Spacer atas
                Constraint::Length(3), // Input box
                Constraint::Length(1), // Error msg
                Constraint::Min(0),
            ])
            .split(input_area);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(" Base URL (OpenAI-compatible) ")
            .title_alignment(ratatui::layout::Alignment::Center);
        let input_inner = input_block.inner(chunks[1]);

        let display = if self.custom_url.is_empty() {
            " https://api.example.com/v1"
        } else {
            self.custom_url.as_str()
        };
        let input_para = Paragraph::new(Line::from(vec![
            Span::styled(display, Style::default().fg(if self.custom_url.is_empty() { Color::Gray } else { Color::Rgb(218, 165, 32) })),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[1]);
        frame.render_widget(input_block, chunks[1]);
        frame.render_widget(input_para, input_inner);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[2]);
        }

        if !self.custom_url.is_empty() {
            let text_len = self.custom_url.len() as u16;
            let cursor_x = input_inner.x + (input_inner.width.saturating_sub(text_len) / 2) + text_len;
            frame.set_cursor_position((cursor_x.min(input_inner.x + input_inner.width.saturating_sub(1)), input_inner.y));
        }
    }

    fn draw_model_fetch(&self, frame: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  \u{25F6}  Mengambil daftar model dari API...", Style::default().fg(Color::Rgb(218, 165, 32))),
            ]),
        ];
        let para = Paragraph::new(lines).style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, area);
    }

    fn draw_model_list(&self, frame: &mut Frame, area: Rect) {
        let vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let provider = self.current_provider();
        let title = Paragraph::new(Line::from(vec![
            Span::styled("Pilih Model — ", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            Span::styled(provider.name, Style::default().fg(Color::Rgb(200, 200, 200))),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, vertical_layout[0]);

        // Menengahkan list & search box secara horizontal dengan lebar tetap 60
        let card_width = 60u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical_layout[1]);

        let list_area = horizontal_chunks[1];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(list_area);

        let search_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(" Cari Model ")
            .title_alignment(ratatui::layout::Alignment::Center);
        let search_inner = search_block.inner(chunks[0]);

        let search_display = if self.model_search.is_empty() {
            " ketik untuk filter..."
        } else {
            self.model_search.as_str()
        };
        let search_para = Paragraph::new(Line::from(vec![
            Span::styled(search_display, Style::default().fg(if self.model_search.is_empty() { Color::Gray } else { Color::Rgb(218, 165, 32) })),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[0]);
        frame.render_widget(search_block, chunks[0]);
        frame.render_widget(search_para, search_inner);

        if !self.model_search.is_empty() {
            let text_len = self.model_search.len() as u16;
            let cursor_x = search_inner.x + (search_inner.width.saturating_sub(text_len) / 2) + text_len;
            frame.set_cursor_position((cursor_x.min(search_inner.x + search_inner.width.saturating_sub(1)), search_inner.y));
        }

        let filtered = self.filtered_models();

        let items: Vec<ListItem> = filtered
            .iter()
            .map(|name| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("  {}", name), Style::default().fg(Color::Rgb(180, 180, 180))),
                ]))
            })
            .collect();

        let list = List::new(items)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)))
            .highlight_symbol("\u{25B6} ")
            .highlight_style(Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(60, 60, 60)))
                    .title(format!(" {} model ", filtered.len()))
                    .title_alignment(ratatui::layout::Alignment::Center),
            );

        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(self.model_list_selected));

        frame.render_stateful_widget(list, chunks[1], &mut list_state);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, vertical_layout[0]);
        }
    }

    fn draw_model_select(&self, frame: &mut Frame, area: Rect) {
        let vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let provider = self.current_provider();
        let title_lines = vec![
            Line::from(vec![
                Span::styled("Masukkan nama model", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled(format!("Provider: {} — ", provider.name), Style::default().fg(Color::Rgb(120, 120, 120))),
                Span::styled("ketik bebas nama model", Style::default().fg(Color::Rgb(80, 80, 80))),
            ]),
        ];

        let title = Paragraph::new(title_lines)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, vertical_layout[0]);

        // Menengahkan input box secara horizontal dengan lebar tetap 60
        let card_width = 60u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical_layout[1]);

        let input_area = horizontal_chunks[1];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(1), Constraint::Min(0)])
            .split(input_area);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(" Nama Model ")
            .title_alignment(ratatui::layout::Alignment::Center);
        let input_inner = input_block.inner(chunks[0]);

        let display = if self.custom_model.is_empty() {
            " misal: gpt-4o, claude-3.5-haiku, llama-3.3-70b, dll."
        } else {
            self.custom_model.as_str()
        };
        let input_para = Paragraph::new(Line::from(vec![
            Span::styled(display, Style::default().fg(if self.custom_model.is_empty() { Color::Gray } else { Color::Rgb(218, 165, 32) })),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[0]);
        frame.render_widget(input_block, chunks[0]);
        frame.render_widget(input_para, input_inner);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[1]);
        }

        if !self.custom_model.is_empty() {
            let text_len = self.custom_model.len() as u16;
            let cursor_x = input_inner.x + (input_inner.width.saturating_sub(text_len) / 2) + text_len;
            frame.set_cursor_position((cursor_x.min(input_inner.x + input_inner.width.saturating_sub(1)), input_inner.y));
        }
    }

    fn draw_telegram_setup_prompt(&self, frame: &mut Frame, area: Rect) {
        let vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let title_lines = vec![
            Line::from(vec![
                Span::styled("Setup Telegram Bot", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Apakah Anda ingin mengaktifkan Telegram Bot?", Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
        ];

        let title = Paragraph::new(title_lines)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, vertical_layout[0]);

        let card_width = 40u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical_layout[1]);

        let opt_area = horizontal_chunks[1];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(opt_area);

        let prompt_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(" Setup Telegram? ")
            .title_alignment(ratatui::layout::Alignment::Center);
        let prompt_inner = prompt_block.inner(chunks[0]);

        let display = if self.setup_telegram { " [Y] Ya  /  N  Tidak " } else { "  Y  Ya  / [N] Tidak " };
        let prompt_para = Paragraph::new(Line::from(vec![
            Span::styled(display, Style::default().fg(Color::Rgb(218, 165, 32))),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[0]);
        frame.render_widget(prompt_block, chunks[0]);
        frame.render_widget(prompt_para, prompt_inner);
    }

    fn draw_telegram_token_input(&self, frame: &mut Frame, area: Rect) {
        let vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let title_lines = vec![
            Line::from(vec![
                Span::styled("Masukkan Token Bot Telegram", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Dapatkan token dari @BotFather di Telegram", Style::default().fg(Color::Rgb(120, 120, 120))),
            ]),
        ];

        let title = Paragraph::new(title_lines)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, vertical_layout[0]);

        let card_width = 60u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical_layout[1]);

        let input_area = horizontal_chunks[1];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(1), Constraint::Min(0)])
            .split(input_area);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(" Token Bot Telegram ")
            .title_alignment(ratatui::layout::Alignment::Center);
        let input_inner = input_block.inner(chunks[0]);

        let display = if self.telegram_token.is_empty() {
            " misal: 123456789:ABC-DEF1234ghIkl..."
        } else {
            self.telegram_token.as_str()
        };
        let input_para = Paragraph::new(Line::from(vec![
            Span::styled(display, Style::default().fg(if self.telegram_token.is_empty() { Color::Gray } else { Color::Rgb(218, 165, 32) })),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[0]);
        frame.render_widget(input_block, chunks[0]);
        frame.render_widget(input_para, input_inner);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[1]);
        }

        if !self.telegram_token.is_empty() {
            let text_len = self.telegram_token.len() as u16;
            let cursor_x = input_inner.x + (input_inner.width.saturating_sub(text_len) / 2) + text_len;
            frame.set_cursor_position((cursor_x.min(input_inner.x + input_inner.width.saturating_sub(1)), input_inner.y));
        }
    }

    fn draw_telegram_binding_wait(&self, frame: &mut Frame, area: Rect) {
        let vertical_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(0),
            ])
            .split(area);

        let title_lines = vec![
            Line::from(vec![
                Span::styled("Telegram Bot Pairing / Binding", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Langkah 1: Cari bot Anda di Telegram dan kirim pesan ", Style::default().fg(Color::Rgb(180, 180, 180))),
                Span::styled("/start", Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD)),
            ]),
        ];

        let title = Paragraph::new(title_lines)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, vertical_layout[0]);

        let card_width = 64u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical_layout[1]);

        let opt_area = horizontal_chunks[1];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(opt_area);

        // Render status
        let status_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
            .title(" Status Polling ")
            .title_alignment(ratatui::layout::Alignment::Center);
        let status_inner = status_block.inner(chunks[0]);
        let status_para = Paragraph::new(Line::from(vec![
            Span::styled(&self.binding_status, Style::default().fg(Color::Rgb(200, 200, 200))),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(20, 20, 20)));

        frame.render_widget(status_block, chunks[0]);
        frame.render_widget(status_para, status_inner);

        // Jika chat sudah dideteksi, tunjukkan kode verifikasi 6 digit yang harus diketik user ke bot telegramnya
        if !self.telegram_chat_id.is_empty() {
            let code_block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
                .title(" Kirim Kode ini ke Telegram Bot ")
                .title_alignment(ratatui::layout::Alignment::Center);
            let code_inner = code_block.inner(chunks[1]);
            let code_para = Paragraph::new(Line::from(vec![
                Span::styled(&self.binding_code, Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]))
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().bg(Color::Rgb(25, 25, 25)));

            frame.render_widget(code_block, chunks[1]);
            frame.render_widget(code_para, code_inner);
        }
    }

    fn draw_review(&self, frame: &mut Frame, area: Rect) {
        let provider = self.current_provider();
        let model = &self.custom_model;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(area);

        let title = Paragraph::new(Line::from(vec![
            Span::styled("Konfirmasi Konfigurasi", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
        ]))
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, chunks[0]);

        let mut lines = vec![
            Line::from(vec![
                Span::styled("  Provider:    ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(provider.name, Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("  Model:       ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(model, Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(vec![
                Span::styled("  API Key:     ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(if self.api_key.is_empty() { "(env var)" } else { "********" }, Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(vec![
                Span::styled("  Base URL:    ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(if provider.base_url.is_empty() { &self.custom_url } else { provider.base_url }, Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Telegram:    ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(if self.setup_telegram { "Aktif" } else { "Non-aktif" }, Style::default().fg(if self.setup_telegram { Color::Green } else { Color::Red })),
            ]),
        ];
        if self.setup_telegram && !self.telegram_token.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  Tele Token:  ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled("********", Style::default().fg(Color::Rgb(200, 200, 200))),
            ]));
            if !self.telegram_chat_id.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("  Chat ID:     ", Style::default().fg(Color::Rgb(150, 150, 150))),
                    Span::styled(&self.telegram_chat_id, Style::default().fg(Color::Rgb(200, 200, 200))),
                ]));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  File config: ", Style::default().fg(Color::Rgb(150, 150, 150))),
            Span::styled(self.config_path.display().to_string(), Style::default().fg(Color::Rgb(120, 120, 120))),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Tekan Enter untuk menyimpan, Esc untuk kembali", Style::default().fg(Color::Rgb(100, 100, 100))),
        ]));


        let review_height = lines.len() as u16;

        // Menengahkan secara vertikal
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(review_height),
                Constraint::Min(0),
            ])
            .split(chunks[1]);

        // Menengahkan secara horizontal dengan card lebar tetap 56
        let card_width = 56u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;

        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical_chunks[1]);

        let para = Paragraph::new(lines)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, horizontal_chunks[1]);
    }

    fn draw_complete(&self, frame: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from(vec![
                Span::styled("\u{2713} Konfigurasi berhasil disimpan!", Style::default().fg(Color::Rgb(0, 200, 100)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Anda bisa menjalankan:", Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  \u{2022} claw10 tui", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("  \u{2022} claw10 serve", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Tekan tombol apa pun untuk keluar.", Style::default().fg(Color::Rgb(120, 120, 120))),
            ]),
        ];

        let complete_height = lines.len() as u16;

        // Menengahkan secara vertikal
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(complete_height),
                Constraint::Min(0),
            ])
            .split(area);

        // Menengahkan secara horizontal dengan card lebar tetap 42
        let card_width = 42u16;
        let left_padding = area.width.saturating_sub(card_width) / 2;

        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(left_padding),
                Constraint::Length(card_width),
                Constraint::Min(0),
            ])
            .split(vertical_chunks[1]);

        let para = Paragraph::new(lines)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, horizontal_chunks[1]);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let (hint, color) = match self.step {
            Step::Welcome => (
                "Enter/Space: mulai  |  q/Esc: keluar",
                Color::Rgb(150, 150, 150),
            ),
            Step::ProviderSelect => (
                "j/k/h/l: pilih  |  Enter: lanjut  |  s/Tab: skip ke review  |  Esc: kembali",
                Color::Rgb(150, 150, 150),
            ),
            Step::ApiKeyInput => (
                "Ketik API key  |  Enter: lanjut  |  Esc: kembali",
                Color::Rgb(150, 150, 150),
            ),
            Step::BaseUrlInput => (
                "Ketik Base URL  |  Enter: lanjut  |  Esc: kembali",
                Color::Rgb(150, 150, 150),
            ),
            Step::ModelFetch => (
                "Mengambil daftar model...",
                Color::Rgb(150, 150, 150),
            ),
            Step::ModelList => (
                "j/k/\u{2191}/\u{2193}: pilih  |  ketik: cari/filter  |  Enter: pilih/ketik manual  |  Esc: kembali",
                Color::Rgb(150, 150, 150),
            ),
            Step::ModelSelect => (
                "Ketik nama model  |  Enter: lanjut  |  Esc: kembali",
                Color::Rgb(150, 150, 150),
            ),
            Step::TelegramSetupPrompt => (
                "y: Ya  |  n: Tidak  |  Esc: kembali",
                Color::Rgb(150, 150, 150),
            ),
            Step::TelegramTokenInput => (
                "Ketik Token Bot Telegram  |  Enter: lanjut  |  Esc: kembali",
                Color::Rgb(150, 150, 150),
            ),
            Step::TelegramBindingWait => (
                if self.telegram_chat_id.is_empty() {
                    "Kirim /start ke bot lalu ketik kode verifikasi  |  Esc: kembali"
                } else {
                    "Kirim /start untuk pairing baru  |  Enter: lanjut  |  Esc: kembali"
                },
                Color::Rgb(150, 150, 150),
            ),
            Step::Review => (
                "Enter: simpan  |  Esc: kembali",
                Color::Rgb(150, 150, 150),
            ),
            Step::Complete => (
                "Tekan tombol apa pun untuk keluar",
                Color::Rgb(150, 150, 150),
            ),
        };

        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Rgb(50, 50, 50)))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let para = Paragraph::new(Line::from(vec![
            Span::styled(hint, Style::default().fg(color)),
        ]))
        .alignment(ratatui::layout::Alignment::Center);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        frame.render_widget(para, layout[1]);
    }
}
