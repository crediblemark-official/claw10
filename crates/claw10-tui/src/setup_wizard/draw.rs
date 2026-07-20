use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, List, ListItem};
use ratatui::Frame;

use super::{SetupWizard, Step};

impl SetupWizard {
    pub(crate) fn draw(&self, frame: &mut Frame) {
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
        let banner_content = include_str!("../../assets/claw10.txt");
        let mut lines = vec![Line::from("")];

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
        
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(welcome_height),
                Constraint::Min(0),
            ])
            .split(area);

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
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(area);

        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(0),
                Constraint::Length(4),
            ])
            .split(chunks[1]);

        let list_area = horizontal_chunks[1];

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
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
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
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
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

        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(review_height),
                Constraint::Min(0),
            ])
            .split(chunks[1]);

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

        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(complete_height),
                Constraint::Min(0),
            ])
            .split(area);

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
