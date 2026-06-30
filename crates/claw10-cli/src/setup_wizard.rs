use std::path::PathBuf;

use crossterm::event::{read, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, List, ListItem};
use ratatui::Frame;

#[derive(Clone)]
struct ProviderOption {
    name: &'static str,
    slot: &'static str,
    env_var: &'static str,
    base_url: &'static str,
}

const PROVIDERS: &[ProviderOption] = &[
    ProviderOption { name: "OpenAI", slot: "openai", env_var: "OPENAI_API_KEY", base_url: "https://api.openai.com/v1" },
    ProviderOption { name: "Anthropic", slot: "anthropic", env_var: "ANTHROPIC_API_KEY", base_url: "https://api.anthropic.com/v1" },
    ProviderOption { name: "Groq", slot: "groq", env_var: "GROQ_API_KEY", base_url: "https://api.groq.com/openai/v1" },
    ProviderOption { name: "OpenRouter", slot: "openrouter", env_var: "OPENROUTER_API_KEY", base_url: "https://openrouter.ai/api/v1" },
    ProviderOption { name: "Custom", slot: "custom", env_var: "CUSTOM_API_KEY", base_url: "" },
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
}

enum Step {
    Welcome,
    ProviderSelect,
    ApiKeyInput,
    BaseUrlInput,
    ModelFetch,
    ModelList,
    ModelSelect,
    Review,
    Complete,
}

impl SetupWizard {
    pub fn new(config_path: PathBuf) -> Self {
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

            // Auto-fetch when entering ModelFetch step, fallback to static list or manual
            if matches!(self.step, Step::ModelFetch) {
                self.do_fetch_models();
                if self.fetch_failed {
                    self.step = Step::ModelSelect;
                } else {
                    self.step = Step::ModelList;
                }
                continue;
            }

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
                        Step::Review => self.handle_review(key),
                        Step::Complete => return Ok(()),
                        Step::ModelFetch => {}
                    }
                }
            }
        }
    }

    fn do_fetch_models(&mut self) {
        // Always load static models as fallback
        self.static_models = self.load_static_models();

        let provider = self.current_provider();
        let base_url = if provider.base_url.is_empty() {
            &self.custom_url
        } else {
            provider.base_url
        };

        // If no API key and no base URL (custom), skip fetch entirely
        if base_url.is_empty() || self.api_key.is_empty() {
            if !self.static_models.is_empty() {
                self.fetched_models = self.static_models.clone();
                self.error_msg = "Gunakan daftar model statis (API key tidak tersedia).".to_string();
            } else {
                self.error_msg = "Tidak ada daftar model. Input manual.".to_string();
                self.fetch_failed = true;
            }
            return;
        }

        let client = reqwest::blocking::Client::new();
        let url = format!("{}/models", base_url.trim_end_matches('/'));

        let resp = match client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(10))
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                self.error_msg = format!("Gagal fetch API: {e}.");
                self.fallback_to_static();
                return;
            }
        };

        if !resp.status().is_success() {
            self.error_msg = format!("Gagal fetch API (HTTP {}).", resp.status().as_u16());
            self.fallback_to_static();
            return;
        }

        let body: serde_json::Value = match resp.json() {
            Ok(v) => v,
            Err(e) => {
                self.error_msg = format!("Gagal parse response: {e}.");
                self.fallback_to_static();
                return;
            }
        };

        let models = body["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v["id"].as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if models.is_empty() {
            self.error_msg = "Tidak ada model dari API.".to_string();
            self.fallback_to_static();
            return;
        }

        self.fetched_models = models;
        self.model_search.clear();
        self.model_list_selected = 0;
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
            Step::ModelSelect => Step::Review,
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
            Step::Review => {
                if !self.fetched_models.is_empty() {
                    Step::ModelList
                } else {
                    Step::ModelSelect
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

    // ── Input Handlers ──

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
            KeyCode::Enter => self.next_step(),
            KeyCode::Esc => self.prev_step(),
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
                    // Typed a custom name not in list → use it directly
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
                // Switch to manual input
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

    fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let provider = self.current_provider();
        let model = self.custom_model.as_str();

        let config = if provider.slot == "custom" {
            format!(
                "# Claw10 OS configuration\n\
                 # Generated by `claw10 setup`\n\n\
                 [custom.my-provider]\n\
                 base_url = \"{url}\"\n\
                 api_key = \"${env_var}\"\n\
                 models = [\"{model}\"]\n\n\
                 [alias.default]\n\
                 slot = \"my-provider\"\n\
                 model = \"{model}\"\n\
                 api_key = \"${env_var}\"\n",
                url = self.custom_url,
                env_var = provider.env_var,
                model = model,
            )
        } else {
            format!(
                "# Claw10 OS configuration\n\
                 # Generated by `claw10 setup`\n\n\
                 [alias.default]\n\
                 slot = \"{slot}\"\n\
                 model = \"{model}\"\n\
                 api_key = \"${env_var}\"\n",
                slot = provider.slot,
                model = model,
                env_var = provider.env_var,
            )
        };

        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.config_path, config)?;

        // Save API key to .env
        if !self.api_key.is_empty() {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            let env_dir = std::path::PathBuf::from(&home).join(".claw10");
            std::fs::create_dir_all(&env_dir)?;
            std::fs::write(env_dir.join(".env"), format!("{}={}\n", provider.env_var, self.api_key))?;
        }

        Ok(())
    }

    // ── Draw ──

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(9),
                Constraint::Min(1),
                Constraint::Length(7),
            ])
            .split(area);

        self.draw_logo(frame, chunks[0]);
        self.draw_content(frame, chunks[1]);
        self.draw_footer(frame, chunks[2]);
    }

    fn draw_logo(&self, frame: &mut Frame, area: Rect) {
        let logo = Block::default()
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        let inner = logo.inner(area);
        frame.render_widget(logo, area);

        let banner_content = include_str!("../../../assets/claw10.txt");
        let mut lines: Vec<Line> = banner_content
            .lines()
            .map(|line| {
                Line::from(vec![
                    Span::styled(format!("  {}", line), Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
                ])
            })
            .collect();

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Recursive Agent Swarm Operating System", Style::default().fg(Color::Rgb(150, 150, 150))),
        ]));

        let para = Paragraph::new(lines)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, inner);
    }

    fn draw_content(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 60)))
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
            Step::Review => self.draw_review(frame, inner),
            Step::Complete => self.draw_complete(frame, inner),
        }
    }

    fn draw_welcome(&self, frame: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Selamat datang di Claw10 OS!", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Wizard ini akan membantu Anda mengatur:", Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("    \u{2022} Provider model LLM", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("    \u{2022} API key", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(vec![
                Span::styled("    \u{2022} Model default (auto-fetch dari API)", Style::default().fg(Color::Rgb(180, 180, 180))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Tekan Enter untuk memulai...", Style::default().fg(Color::Rgb(120, 120, 120))),
            ]),
        ];
        let para = Paragraph::new(lines).style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, area);
    }

    fn draw_provider_select(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(area);

        let title = Paragraph::new(Line::from(vec![
            Span::styled(" Pilih Provider LLM", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, chunks[0]);

        let cols = 2u16;
        let _rows = (self.providers.len() as u16 + cols - 1) / cols;
        let card_h = 5u16;
        let list_area = chunks[1];

        for (i, provider) in self.providers.iter().enumerate() {
            let col = i as u16 % cols;
            let row = i as u16 / cols;
            let card_width = list_area.width / cols;
            let card_x = list_area.x + col * card_width;
            let card_y = list_area.y + row * card_h;

            if card_y + card_h > list_area.y + list_area.height {
                break;
            }

            let card_rect = Rect { x: card_x, y: card_y, width: card_width, height: card_h };
            let is_selected = i == self.selected;

            let border_style = if is_selected {
                Style::default().fg(Color::Rgb(254, 192, 126))
            } else {
                Style::default().fg(Color::Rgb(60, 60, 60))
            };
            let card = Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .style(if is_selected {
                    Style::default().bg(Color::Rgb(25, 25, 25))
                } else {
                    Style::default().bg(Color::Rgb(15, 15, 15))
                });

            let inner = card.inner(card_rect);
            frame.render_widget(ratatui::widgets::Clear, card_rect);
            frame.render_widget(card, card_rect);

            let text = vec![
                Line::from(vec![
                    Span::styled(
                        format!(" {}", provider.name),
                        if is_selected {
                            Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Rgb(200, 200, 200))
                        },
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "  OpenAI-compatible",
                        Style::default().fg(Color::Rgb(80, 80, 80)),
                    ),
                ]),
            ];

            let para = Paragraph::new(text)
                .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(para, inner);
        }
    }

    fn draw_api_key_input(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Length(3), Constraint::Length(1), Constraint::Min(0)])
            .split(area);

        let provider = self.current_provider();
        let title = Paragraph::new(Line::from(vec![
            Span::styled(format!(" API Key untuk {}", provider.name), Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, chunks[0]);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(format!(" {} ", provider.env_var));
        let input_inner = input_block.inner(chunks[1]);

        let display = if self.api_key.is_empty() {
            "  (input hidden — ketik API key Anda)"
        } else {
            &"*".repeat(self.api_key.len().min(40))
        };
        let input_para = Paragraph::new(Line::from(vec![
            Span::styled(display, Style::default().fg(Color::Rgb(218, 165, 32))),
        ]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[1]);
        frame.render_widget(input_block, chunks[1]);
        frame.render_widget(input_para, input_inner);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!("  {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[2]);
        }

        if !self.api_key.is_empty() {
            let cursor_x = input_inner.x + self.api_key.len() as u16;
            frame.set_cursor_position((cursor_x.min(input_inner.x + input_inner.width.saturating_sub(1)), input_inner.y));
        }
    }

    fn draw_base_url_input(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Length(3), Constraint::Length(1), Constraint::Min(0)])
            .split(area);

        let title = Paragraph::new(Line::from(vec![
            Span::styled(" Base URL untuk Custom Provider", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, chunks[0]);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(" Base URL (OpenAI-compatible) ");
        let input_inner = input_block.inner(chunks[1]);

        let display = if self.custom_url.is_empty() {
            "  https://api.example.com/v1"
        } else {
            self.custom_url.as_str()
        };
        let input_para = Paragraph::new(Line::from(vec![
            Span::styled(display, Style::default().fg(if self.custom_url.is_empty() { Color::DarkGray } else { Color::Rgb(218, 165, 32) })),
        ]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[1]);
        frame.render_widget(input_block, chunks[1]);
        frame.render_widget(input_para, input_inner);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!("  {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[2]);
        }

        if !self.custom_url.is_empty() {
            let cursor_x = input_inner.x + self.custom_url.len() as u16;
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let provider = self.current_provider();
        let title = Paragraph::new(Line::from(vec![
            Span::styled(" Pilih Model — ", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            Span::styled(provider.name, Style::default().fg(Color::Rgb(200, 200, 200))),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, chunks[0]);

        let search_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(" Cari Model ");
        let search_inner = search_block.inner(chunks[1]);

        let search_display = if self.model_search.is_empty() {
            "  ketik untuk filter..."
        } else {
            self.model_search.as_str()
        };
        let search_para = Paragraph::new(Line::from(vec![
            Span::styled(search_display, Style::default().fg(if self.model_search.is_empty() { Color::DarkGray } else { Color::Rgb(218, 165, 32) })),
        ]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[1]);
        frame.render_widget(search_block, chunks[1]);
        frame.render_widget(search_para, search_inner);

        if !self.model_search.is_empty() {
            let cursor_x = search_inner.x + self.model_search.len() as u16;
            frame.set_cursor_position((cursor_x.min(search_inner.x + search_inner.width.saturating_sub(1)), search_inner.y));
        }

        // Model list
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
                    .title(format!(" {} model ", filtered.len())),
            );

        let list_area = chunks[2];
        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(Some(self.model_list_selected));

        frame.render_stateful_widget(list, list_area, &mut list_state);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!("  {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[0]);
        }
    }

    fn draw_model_select(&self, frame: &mut Frame, area: Rect) {
        let provider = self.current_provider();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(1), Constraint::Min(0)])
            .split(area);

        let title = Paragraph::new(Line::from(vec![
            Span::styled(" Masukkan nama model", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, chunks[0]);

        let hint = Paragraph::new(Line::from(vec![
            Span::styled(format!("  Provider: {} — ", provider.name), Style::default().fg(Color::Rgb(120, 120, 120))),
            Span::styled("ketik bebas nama model", Style::default().fg(Color::Rgb(80, 80, 80))),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(hint, chunks[0]);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title(" Nama Model ");
        let input_inner = input_block.inner(chunks[1]);

        let display = if self.custom_model.is_empty() {
            "  misal: gpt-4o, claude-3.5-haiku, llama-3.3-70b, dll."
        } else {
            self.custom_model.as_str()
        };
        let input_para = Paragraph::new(Line::from(vec![
            Span::styled(display, Style::default().fg(if self.custom_model.is_empty() { Color::DarkGray } else { Color::Rgb(218, 165, 32) })),
        ]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));

        frame.render_widget(ratatui::widgets::Clear, chunks[1]);
        frame.render_widget(input_block, chunks[1]);
        frame.render_widget(input_para, input_inner);

        if !self.error_msg.is_empty() {
            let err = Paragraph::new(Line::from(vec![
                Span::styled(format!("  {}", self.error_msg), Style::default().fg(Color::Red)),
            ]))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[2]);
        }

        if !self.custom_model.is_empty() {
            let cursor_x = input_inner.x + self.custom_model.len() as u16;
            frame.set_cursor_position((cursor_x.min(input_inner.x + input_inner.width.saturating_sub(1)), input_inner.y));
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
            Span::styled(" Konfirmasi Konfigurasi", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(title, chunks[0]);

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  Provider:  ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(provider.name, Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("  Model:     ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(model, Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(vec![
                Span::styled("  API Key:   ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(if self.api_key.is_empty() { "(env var)" } else { "********" }, Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(vec![
                Span::styled("  Base URL:  ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(if provider.base_url.is_empty() { &self.custom_url } else { provider.base_url }, Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  File config: ", Style::default().fg(Color::Rgb(150, 150, 150))),
                Span::styled(self.config_path.display().to_string(), Style::default().fg(Color::Rgb(120, 120, 120))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Tekan Enter untuk menyimpan, Esc untuk kembali", Style::default().fg(Color::Rgb(100, 100, 100))),
            ]),
        ];

        let para = Paragraph::new(lines)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, chunks[1]);
    }

    fn draw_complete(&self, frame: &mut Frame, area: Rect) {
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  \u{2713} Konfigurasi berhasil disimpan!", Style::default().fg(Color::Rgb(0, 200, 100)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Anda bisa menjalankan:", Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("    claw10 tui", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("    claw10 serve", Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Tekan tombol apa pun untuk keluar.", Style::default().fg(Color::Rgb(120, 120, 120))),
            ]),
        ];
        let para = Paragraph::new(lines)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, area);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let (hint, color) = match self.step {
            Step::Welcome => (
                "  Enter/Space: mulai  |  q/Esc: keluar",
                Color::Rgb(100, 100, 100),
            ),
            Step::ProviderSelect => (
                "  j/k/\u{2191}/\u{2193}: pilih  |  Enter: lanjut  |  Esc: kembali",
                Color::Rgb(100, 100, 100),
            ),
            Step::ApiKeyInput => (
                "  Ketik API key  |  Enter: lanjut  |  Esc: kembali",
                Color::Rgb(100, 100, 100),
            ),
            Step::BaseUrlInput => (
                "  Ketik Base URL  |  Enter: lanjut  |  Esc: kembali",
                Color::Rgb(100, 100, 100),
            ),
            Step::ModelFetch => (
                "  Mengambil daftar model...",
                Color::Rgb(100, 100, 100),
            ),
            Step::ModelList => (
                "  j/k/\u{2191}/\u{2193}: pilih  |  ketik: cari/filter  |  Enter: pilih/ketik manual  |  Esc: kembali",
                Color::Rgb(100, 100, 100),
            ),
            Step::ModelSelect => (
                "  Ketik nama model  |  Enter: lanjut  |  Esc: kembali",
                Color::Rgb(100, 100, 100),
            ),
            Step::Review => (
                "  Enter: simpan  |  Esc: kembali",
                Color::Rgb(100, 100, 100),
            ),
            Step::Complete => (
                "  Tekan tombol apa pun untuk keluar",
                Color::Rgb(100, 100, 100),
            ),
        };

        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  \u{2501}".repeat(area.width.saturating_sub(4) as usize), Style::default().fg(Color::Rgb(40, 40, 40))),
            ]),
            Line::from(vec![
                Span::styled(format!("  {}", hint), Style::default().fg(color)),
            ]),
        ];

        let para = Paragraph::new(lines)
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(para, area);
    }
}
