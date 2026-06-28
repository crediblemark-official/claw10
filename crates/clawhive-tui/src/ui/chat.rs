use clawhive_domain::{AgentState, SpawnState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{Tab, TuiApp};


pub fn draw_chat(frame: &mut Frame, area: Rect, app: &TuiApp) {
    // Tampilkan sidebar hanya jika lebar layar >= 90
    let show_sidebar = area.width >= 90;

    let main_chunks = if show_sidebar {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(area)
    };

    // Hitung tinggi input box secara dinamis berdasarkan wrap_text dari input_buffer
    let input_inner_width = (main_chunks[0].width as usize).saturating_sub(5).max(1);
    let raw_input_lines = if app.pending_tool_approval.is_some() {
        // Sediakan ruang tinggi 4 baris untuk dialog approval
        vec![String::new(), String::new(), String::new(), String::new()]
    } else if app.input_buffer.is_empty() {
        vec!["Ketik pesan di sini...".to_string()]
    } else {
        crate::ui::wrap_text(&app.input_buffer, input_inner_width.saturating_sub(2).max(1))
    };
    let input_lines: Vec<String> = raw_input_lines
        .into_iter()
        .map(|line| format!("  {}", line))
        .collect();

    // Hitung tinggi tambahan untuk slash command suggestions
    let mut suggestion_height = 0;
    if app.input_buffer.starts_with('/') {
        if app.input_buffer.starts_with("/model") {
            suggestion_height = 2 + app.active_suggestions.len().max(1) + 1; // Header + suggestions + footer + spacer
        } else {
            if !app.active_suggestions.is_empty() {
                suggestion_height = 2 + app.active_suggestions.len() + 1; // Header + suggestions + footer + spacer
            }
        }
    }

    let input_height = (input_lines.len() + 2 + suggestion_height) as u16; // input lines + spacer + status + suggestions

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Chat history
            Constraint::Length(input_height), // Input Box dinamis
            Constraint::Length(1), // Spacer bawah agar tidak mepet ke footer/batas bawah
        ])
        .split(main_chunks[0]);

    // 1. Chat History (Render manual menggunakan sub-layout dinamis agar background solid dan rapi)
    // Tambahkan 1 baris spacer kosong di paling atas agar chat tidak mepet ke atas
    let chat_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Spacer kosong di paling atas
            Constraint::Min(0),    // Area utama chat history
            Constraint::Length(1), // Spacer kosong di paling bawah (padding atas form input)
        ])
        .split(left_chunks[0]);

    // Tambahkan margin horizontal 2 spasi kiri-kanan agar gelembung chat tidak mepet tepi
    let horizontal_chat_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2), // Padding kiri
            Constraint::Min(0),    // Chat history container
            Constraint::Length(2), // Padding kanan
        ])
        .split(chat_layout[1]);

    let chat_area = horizontal_chat_layout[1];

    // Lebar wrap: user bubble pakai penuh, assistant pakai -4 (2 padding + 2 safety)
    let user_wrap_w = (chat_area.width as usize).saturating_sub(6).max(1);
    let asst_wrap_w = (chat_area.width as usize).saturating_sub(10).max(1);

    // Helper: hitung visual line count sebuah pesan dengan wrap_width tertentu menggunakan wrap_text helper
    let count_lines = |msg: &str, wrap_w: usize| -> usize {
        crate::ui::wrap_text(msg, wrap_w).len().max(1)
    };

    // Helper: hitung tinggi bubble (visual lines + chrome)
    let bubble_height = |sender: &str, msg: &str| -> usize {
        let is_user = sender.eq_ignore_ascii_case("user") || sender.eq_ignore_ascii_case("system");
        let wrap_w = if is_user { user_wrap_w } else { asst_wrap_w };
        let vlines = count_lines(msg, wrap_w);
        if is_user {
            vlines + 2 // padding atas + bawah
        } else {
            1 + 1 + vlines // header + blank + lines
        }
    };

    // Hitung tinggi total semua bubble
    let total_needed_height: usize = app.chat_history.iter()
        .map(|(sender, _, msg)| bubble_height(sender, msg))
        .sum();

    // Scroll offset dari state app (clamp ke max secara dinamis via Cell)
    let max_scroll = total_needed_height.saturating_sub(chat_area.height as usize);
    let clamped_scroll_offset = app.chat_scroll_offset.get().min(max_scroll);
    app.chat_scroll_offset.set(clamped_scroll_offset);

    let scroll_offset = if app.chat_at_bottom || max_scroll == 0 {
        max_scroll
    } else {
        max_scroll.saturating_sub(clamped_scroll_offset)
    };

    let mut current_y_offset = 0;

    for (idx, (sender, model, msg)) in app.chat_history.iter().enumerate() {
        // Hitung tinggi asli bubble
        let item_height = bubble_height(sender, msg);
        let item_start = current_y_offset;
        let item_end = item_start + item_height;

        current_y_offset += item_height;

        // Skip rendering jika gelembung pesan berada di luar viewport (karena scroll)
        if item_end <= scroll_offset {
            continue;
        }

        // Hitung pergeseran posisi y gelembung
        let relative_y = item_start.saturating_sub(scroll_offset) as u16;
        if relative_y >= chat_area.height {
            break; // Berada di bawah viewport
        }

        let visible_height = (item_end - scroll_offset).min(chat_area.height as usize) - relative_y as usize;
        if visible_height == 0 {
            continue;
        }

        let render_area = Rect {
            x: chat_area.x,
            y: chat_area.y + relative_y,
            width: chat_area.width,
            height: visible_height as u16,
        };

        // Render gelembung pesan chat
        if sender.to_lowercase() == "user" || sender.to_lowercase() == "system" {
            let is_user = sender.to_lowercase() == "user";
            let border_color = if is_user { Color::Rgb(218, 165, 32) } else { Color::Red };

            // Bagi area gelembung secara horizontal: border kiri (1 kolom), spacer (2 kolom), teks (sisa area)
            let bubble_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(1), // Border kiri
                    Constraint::Length(2), // Jarak spasi kosong dari border kiri
                    Constraint::Min(0),    // Konten teks pesan
                ])
                .split(render_area);

            let input_block = Block::default()
                .borders(Borders::LEFT)
                .border_style(
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                );
            frame.render_widget(input_block, bubble_chunks[0]);

            let mut lines = Vec::new();
            lines.push(Line::from("")); // Padding vertikal atas
            for part in msg.lines() {
                lines.push(parse_markdown_line(part, Style::default()));
            }
            lines.push(Line::from("")); // Padding vertikal bawah

            let p = Paragraph::new(lines)
                .wrap(Wrap { trim: false });
            frame.render_widget(p, bubble_chunks[2]);
        } else if sender.to_lowercase() == "tool" {
            // Render pesan Tool secara khusus dengan warna/ikon tersendiri
            let mut lines = Vec::new();

            // Header: ikon kunci/tool + nama tool
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("🔧 ", Style::default().fg(Color::LightBlue)),
                Span::styled(format!("Tool: {model}"), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
            ]));
            lines.push(Line::from(""));

            // Teks konten tool (bisa multiline, kita wrap_text)
            for line_str in crate::ui::wrap_text(msg, asst_wrap_w) {
                if line_str.is_empty() {
                    lines.push(Line::from(Span::raw("  ")));
                } else {
                    let mut markdown_line = parse_markdown_line(&line_str, Style::default().fg(Color::Rgb(170, 180, 190)));
                    markdown_line.spans.insert(0, Span::raw("  "));
                    lines.push(markdown_line);
                }
            }

            let p = Paragraph::new(lines);
            frame.render_widget(p, render_area);
        } else {
            // Agent / Assistant: padding kiri 2 spasi, pre-wrap manual agar indentasi konsisten
            let mut lines = Vec::new();

            // Cek apakah item asisten ini adalah item terakhir dan sedang streaming
            let is_last_item = idx == app.chat_history.len().saturating_sub(1);
            let model_display = if is_last_item && app.is_streaming {
                format!("{} (Merespons...)", model)
            } else {
                model.clone()
            };

            // Header: ikon + model
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("■ ", Style::default().fg(Color::Rgb(218, 165, 32))),
                Span::styled(model_display, Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(""));

            // Pre-wrap menggunakan word-wrap helper dan parse markdown bold
            for line_str in crate::ui::wrap_text(msg, asst_wrap_w) {
                if line_str.is_empty() {
                    lines.push(Line::from(Span::raw("  ")));
                } else {
                    let mut markdown_line = parse_markdown_line(&line_str, Style::default());
                    markdown_line.spans.insert(0, Span::raw("  "));
                    lines.push(markdown_line);
                }
            }

            let p = Paragraph::new(lines);
            frame.render_widget(p, render_area);
        }
    }

    // 2. Input Box (Tambahkan margin horizontal 2 spasi kiri-kanan agar simetris)
    let horizontal_input_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2), // Margin kiri 2 spasi
            Constraint::Min(0),    // Input box utama
            Constraint::Length(2), // Margin kanan 2 spasi
        ])
        .split(left_chunks[1]);
    let active_input_area = horizontal_input_layout[1];

    let input_block = Block::default().borders(Borders::LEFT).border_style(
        Style::default()
            .fg(Color::Rgb(218, 165, 32))
            .add_modifier(Modifier::BOLD),
    );

    let input_inner = input_block.inner(active_input_area);

    let (active_model_name, provider_name) = if let Some(router) = &app.state.model_router {
        let registry = router.registry();
        let profiles = registry.list_profiles();
        if profiles.is_empty() {
            ("Belum Dikonfigurasi".to_string(), "None".to_string())
        } else {
            let matched = profiles.iter().find(|p| p.model_name == app.active_model || p.id == app.active_model);
            if let Some(profile) = matched {
                (profile.model_name.clone(), profile.provider.clone())
            } else {
                (profiles[0].model_name.clone(), profiles[0].provider.clone())
            }
        }
    } else {
        ("Belum Dikonfigurasi".to_string(), "None".to_string())
    };

    let status_label = if app.is_streaming {
        const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spinner = SPINNER_FRAMES[app.spinner_tick % SPINNER_FRAMES.len()];
        let status_text = app.stream_status.as_deref().unwrap_or("Berpikir...");
        format!("TUI {} {}", spinner, status_text)
    } else {
        "TUI".to_string()
    };
    let status_color = if app.is_streaming { Color::Yellow } else { Color::Rgb(218, 165, 32) };

    let left_len = 2 + status_label.len() + 3 + active_model_name.len() + 1 + provider_name.len();
    let right_len = 40; // panjang visual dari: "/ commands  : terminal  ctrl+p palette  "
    
    let spacer_len = (input_inner.width as usize)
        .saturating_sub(left_len)
        .saturating_sub(right_len)
        .max(1);
    let middle_spacer = " ".repeat(spacer_len);

    let mut chat_input_lines = Vec::new();
    if let Some(ref req) = app.pending_tool_approval {
        chat_input_lines.push(Line::from(vec![
            Span::styled("  ⚠️  PERSETUJUAN EKSEKUSI SHELL: ", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
            Span::styled(&req.tool_name, Style::default().fg(Color::White).add_modifier(Modifier::ITALIC)),
        ]));
        chat_input_lines.push(Line::from(vec![
            Span::styled("     $ ", Style::default().fg(Color::DarkGray)),
            Span::styled(&req.command, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]));
        chat_input_lines.push(Line::from(""));
        chat_input_lines.push(Line::from(vec![
            Span::styled("     Pilihan: ", Style::default().fg(Color::White)),
            Span::styled("[a] Allow (Sekali)", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("   ", Style::default()),
            Span::styled("[w] Always Allow", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
            Span::styled("   ", Style::default()),
            Span::styled("[d] Deny (Tolak)", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
    } else {
        // Tambahkan suggestions jika input diawali dengan '/'
        if app.input_buffer.starts_with('/') {
            if !app.input_buffer.starts_with("/model") {
                if !app.active_suggestions.is_empty() {
                    chat_input_lines.push(Line::from(Span::styled("   [Commands] ---------------------------------------------", Style::default().fg(Color::DarkGray))));
                    
                    let all_descs = vec![
                        ("/model <id>", "   - Ganti model aktif secara cepat"),
                        ("/help", "         - Tampilkan daftar perintah lengkap"),
                        ("/refresh", "      - Segarkan database agen dan task"),
                        ("/q", "            - Keluar dari aplikasi TUI"),
                    ];
                    
                    for (idx, (cmd_name, _)) in app.active_suggestions.iter().enumerate() {
                        let is_selected = idx == app.suggestion_index;
                        let (prefix, style) = if is_selected {
                            (" >  ", Style::default().fg(Color::Rgb(0, 0, 0)).bg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD))
                        } else {
                            ("    ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                        };
                        
                        let display_name = if cmd_name == "/model " { "/model <id>" } else { cmd_name.as_str() };
                        let desc = all_descs.iter()
                            .find(|(name, _)| *name == display_name || name.starts_with(cmd_name))
                            .map(|(_, d)| *d)
                            .unwrap_or("");
                            
                        chat_input_lines.push(Line::from(vec![
                            Span::styled(format!("{}{:<15}", prefix, display_name), style),
                            Span::styled(desc, Style::default().fg(Color::Gray)),
                        ]));
                    }
                    
                    chat_input_lines.push(Line::from(Span::styled("   --------------------------------------------------------", Style::default().fg(Color::DarkGray))));
                    chat_input_lines.push(Line::from(""));
                }
            } else if app.input_buffer.starts_with("/model") {
                chat_input_lines.push(Line::from(Span::styled("   [Select Model] -----------------------------------------", Style::default().fg(Color::DarkGray))));
                if app.active_suggestions.is_empty() {
                    chat_input_lines.push(Line::from(Span::styled("     (Tidak ada model terkonfigurasi cocok)", Style::default().fg(Color::Red))));
                } else {
                    for (idx, (model_id, _)) in app.active_suggestions.iter().enumerate() {
                        let is_selected = idx == app.suggestion_index;
                        let is_active = model_id == &app.active_model;
                        
                        let mut prefix = if is_active { " ✓  ".to_string() } else { "    ".to_string() };
                        if is_selected {
                            prefix = " >  ".to_string();
                        }
                        
                        let style = if is_selected {
                            Style::default().fg(Color::Rgb(0, 0, 0)).bg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::BOLD)
                        } else if is_active {
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::Cyan)
                        };
                        
                        // Dapatkan nama provider dari router registry (jika ada)
                        let provider = app.state.model_router.as_ref()
                            .and_then(|r| r.registry().list_profiles().iter().find(|p| &p.id == model_id).map(|p| p.provider.clone()))
                            .unwrap_or_else(|| "Unknown".to_string());
                        
                        chat_input_lines.push(Line::from(vec![
                            Span::styled(format!("{}{:<25}", prefix, model_id), style),
                            Span::styled(format!(" ({})", provider), Style::default().fg(Color::DarkGray)),
                        ]));
                    }
                }
                chat_input_lines.push(Line::from(Span::styled("   --------------------------------------------------------", Style::default().fg(Color::DarkGray))));
                chat_input_lines.push(Line::from(""));
            }
        }

        let text_style = if app.input_buffer.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };
        for line in &input_lines {
            chat_input_lines.push(Line::from(Span::styled(line.clone(), text_style)));
        }
    }
    chat_input_lines.push(Line::from("")); // Spacer
    chat_input_lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            status_label,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" · {} ", active_model_name), Style::default()),
        Span::styled(provider_name, Style::default().fg(Color::DarkGray)),
        Span::raw(middle_spacer),
        Span::styled("/", Style::default()),
        Span::styled(" commands  ", Style::default().fg(Color::DarkGray)),
        Span::styled(":", Style::default()),
        Span::styled(" terminal  ", Style::default().fg(Color::DarkGray)),
        Span::styled("ctrl+p", Style::default()),
        Span::styled(" palette  ", Style::default().fg(Color::DarkGray)),
    ]));

    let input_widget = Paragraph::new(chat_input_lines)
        .block(input_block);
    frame.render_widget(input_widget, active_input_area);

    let cursor_pos = if app.pending_tool_approval.is_some() {
        // Sembunyikan cursor dengan menaruhnya di pojok kanan bawah terminal
        (frame.area().width.saturating_sub(1), frame.area().height.saturating_sub(1))
    } else if app.input_buffer.is_empty() {
        (input_inner.x + 2, input_inner.y + suggestion_height as u16)
    } else {
        let last_line = input_lines.last().cloned().unwrap_or_default();
        (
            input_inner.x + last_line.len() as u16,
            input_inner.y + (input_lines.len() - 1 + suggestion_height) as u16,
        )
    };
    frame.set_cursor_position(cursor_pos);

    if show_sidebar {
        // --- KOLOM KANAN (SIDEBAR) ---
        // Bersihkan area sidebar dan isi dengan background hitam pekat solid
        frame.render_widget(ratatui::widgets::Clear, main_chunks[1]);
        let bg_block = Block::default().style(Style::default().bg(Color::Rgb(0, 0, 0)));
        frame.render_widget(bg_block, main_chunks[1]);

        // Sidebar block dengan background hitam pekat solid untuk membedakannya dari chat area
        let sidebar_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::Rgb(0, 0, 0)));
        let sidebar_inner = sidebar_block.inner(main_chunks[1]);
        frame.render_widget(sidebar_block, main_chunks[1]);

        // Berikan padding horizontal 2 spasi di dalam sidebar agar teks tidak mepet border
        let horizontal_sidebar_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // Padding kiri dari garis pembatas
                Constraint::Min(0),    // Area utama sidebar
                Constraint::Length(2), // Padding kanan dari tepi layar
            ])
            .split(sidebar_inner);
        let active_sidebar_area = horizontal_sidebar_layout[1];

        // Isi padding horizontal dengan background hitam pekat agar tidak transparan
        let black = Style::default().bg(Color::Rgb(0, 0, 0));
        let black_block = Block::default().style(black);
        frame.render_widget(black_block.clone(), horizontal_sidebar_layout[0]);
        frame.render_widget(black_block.clone(), horizontal_sidebar_layout[2]);

        // Bagi sidebar menjadi: Spacer Atas, Header (Titel tab), Spacer Tengah, List, dan Footer info
        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Spacer atas agar tidak mepet ke layar atas
                Constraint::Length(2), // Header (Tabs)
                Constraint::Length(1), // Spacer tengah agar bawah tab tidak mepet
                Constraint::Min(0),    // Content list
                Constraint::Length(2), // Footer
            ])
            .split(active_sidebar_area);

        // Isi spacer atas dan tengah dengan background hitam pekat
        frame.render_widget(black_block.clone(), sidebar_chunks[0]);
        frame.render_widget(black_block.clone(), sidebar_chunks[2]);

        // Sidebar Header (Titel tab aktif dengan indikator navigasi)
        let tab_titles = vec!["Sess", "Agents", "Pool", "Broker"];
        let tabs = ratatui::widgets::Tabs::new(tab_titles)
            .select(match app.selected_tab {
                Tab::Session => 0,
                Tab::Agents => 1,
                Tab::Workers => 2,
                Tab::SpawnRequests => 3,
            })
            .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0))))
            .style(Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0)))
            .highlight_style(
                Style::default()
                    .fg(Color::Rgb(218, 165, 32))
                    .bg(Color::Rgb(0, 0, 0))
                    .add_modifier(Modifier::BOLD),
            )
            .divider(" ");
        frame.render_widget(tabs, sidebar_chunks[1]);

        // Render data list sidebar sesuai tab aktif
        match app.selected_tab {
            Tab::Session => {
                let items = vec![ListItem::new(Span::styled(
                    "  Sesi default (Aktif)",
                    Style::default().fg(Color::Rgb(218, 165, 32)).bg(Color::Rgb(0, 0, 0)),
                ))];
                let list = List::new(items)
                    .block(Block::default())
                    .style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, sidebar_chunks[3]);
            }
            Tab::Agents => {
                if app.agents.is_empty() {
                    let p = Paragraph::new(Span::styled(
                        "  No active agents.",
                        Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                    ))
                    .style(Style::default().fg(Color::Rgb(255, 255, 255)).bg(Color::Rgb(0, 0, 0)));
                    frame.render_widget(p, sidebar_chunks[3]);
                } else {
                    let items: Vec<ListItem> = app
                        .agents
                        .iter()
                        .enumerate()
                        .map(|(i, a)| {
                            let is_selected = i == app.selected_index;
                            let style = if is_selected {
                                Style::default()
                                    .fg(Color::Rgb(0, 0, 0))
                                    .bg(Color::Rgb(254, 192, 126))
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0))
                            };

                            let status_indicator = match a.state {
                                AgentState::Active => Span::styled("● ", Style::default().fg(Color::Green).bg(Color::Rgb(0, 0, 0))),
                                AgentState::Paused => {
                                    Span::styled("● ", Style::default().fg(Color::Yellow).bg(Color::Rgb(0, 0, 0)))
                                }
                                AgentState::Terminated => {
                                    Span::styled("● ", Style::default().fg(Color::Red).bg(Color::Rgb(0, 0, 0)))
                                }
                                _ => Span::styled("● ", Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0))),
                            };

                            ListItem::new(Line::from(vec![
                                Span::styled("  ", Style::default().fg(Color::Rgb(255, 255, 255)).bg(Color::Rgb(0, 0, 0))),
                                status_indicator,
                                Span::styled(format!("{:<15}", a.name), style),
                            ]))
                        })
                        .collect();
                    let list = List::new(items)
                        .block(Block::default())
                        .style(Style::default().bg(Color::Rgb(0, 0, 0)));
                    frame.render_widget(list, sidebar_chunks[3]);
                }
            }
            Tab::Workers => {
                if app.workers.is_empty() {
                    let p = Paragraph::new(Span::styled(
                        "  No registered workers.",
                        Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                    ))
                    .style(Style::default().fg(Color::Rgb(255, 255, 255)).bg(Color::Rgb(0, 0, 0)));
                    frame.render_widget(p, sidebar_chunks[3]);
                } else {
                    let items: Vec<ListItem> = app
                        .workers
                        .iter()
                        .map(|w| {
                            let name = format!("  {:<15}", w.id.0.to_string().chars().take(8).collect::<String>());
                            ListItem::new(Line::from(vec![
                                Span::styled(name, Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0))),
                                Span::styled(format!(" {:?}", w.state), Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0))),
                            ]))
                        })
                        .collect();
                    let list = List::new(items)
                        .block(Block::default())
                        .style(Style::default().bg(Color::Rgb(0, 0, 0)));
                    frame.render_widget(list, sidebar_chunks[3]);
                }
            }
            Tab::SpawnRequests => {
                if app.spawn_requests.is_empty() {
                    let p = Paragraph::new(Span::styled(
                        "  No spawn requests.",
                        Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                    ))
                    .style(Style::default().fg(Color::Rgb(255, 255, 255)).bg(Color::Rgb(0, 0, 0)));
                    frame.render_widget(p, sidebar_chunks[3]);
                } else {
                    let items: Vec<ListItem> = app
                        .spawn_requests
                        .iter()
                        .map(|r| {
                            let req_id_prefix = &r.id.0.to_string()[..8];
                            let display_name = r.team.name.replace("-team", "");
                            let name = format!("  {:<12} ({})", display_name.chars().take(10).collect::<String>(), req_id_prefix);
                            let (status_str, status_color) = match r.state {
                                SpawnState::Pending => (" Pending", Color::Yellow),
                                SpawnState::Approved => (" Approved", Color::Green),
                                SpawnState::Denied => (" Denied", Color::Red),
                                SpawnState::Validating => (" Validating", Color::Cyan),
                                SpawnState::Provisioning => (" Provisioning", Color::Blue),
                                SpawnState::Completed => (" Completed", Color::LightGreen),
                                SpawnState::Failed => (" Failed", Color::LightRed),
                            };
                            ListItem::new(Line::from(vec![
                                Span::styled(name, Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0))),
                                Span::styled(status_str, Style::default().fg(status_color).bg(Color::Rgb(0, 0, 0))),
                            ]))
                        })
                        .collect();
                    let list = List::new(items)
                        .block(Block::default())
                        .style(Style::default().bg(Color::Rgb(0, 0, 0)));
                    frame.render_widget(list, sidebar_chunks[3]);
                }
            }
        }

        // Sidebar Footer
        let current_dir = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/home/rasyiqi/PROJECT/clawhive".to_string());

        // Pecah path agar kata "clawhive" dicetak putih tebal
        let (base_path, folder_name) = if current_dir.ends_with("/clawhive") {
            (
                current_dir[..current_dir.len() - 8].to_string(),
                "clawhive".to_string(),
            )
        } else {
            (current_dir.to_string(), "".to_string())
        };

        let repo_line = if folder_name.is_empty() {
            Line::from(vec![
                Span::styled(base_path, Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0))),
                Span::styled(":master", Style::default().fg(Color::Rgb(255, 255, 255)).bg(Color::Rgb(0, 0, 0))),
            ])
        } else {
            Line::from(vec![
                Span::styled(base_path, Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0))),
                Span::styled(
                    folder_name,
                    Style::default()
                        .fg(Color::Rgb(255, 255, 255))
                        .bg(Color::Rgb(0, 0, 0))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(":master", Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0))),
            ])
        };

        let footer_text = Paragraph::new(vec![
            repo_line,
            Line::from(vec![
                Span::styled("● ", Style::default().fg(Color::Green).bg(Color::Rgb(0, 0, 0))),
                Span::styled(
                    "ClawHive ",
                    Style::default()
                        .fg(Color::Rgb(255, 255, 255))
                        .bg(Color::Rgb(0, 0, 0))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("0.1.0", Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0))),
            ]),
        ])
        .style(Style::default().fg(Color::Rgb(255, 255, 255)).bg(Color::Rgb(0, 0, 0))); // Background hitam pekat
        frame.render_widget(footer_text, sidebar_chunks[4]);
    }
}


/// Melakukan parsing markdown bold sederhana (**text**) menjadi kumpulan Span.
/// Karakter asterisk (**) tidak ikut ditampilkan, melainkan diganti dengan Modifier::BOLD.
fn parse_markdown_line(text: &str, base_style: Style) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current_pos = 0;
    let chars: Vec<char> = text.chars().collect();

    while current_pos < chars.len() {
        // Cek token bold "**"
        if current_pos + 1 < chars.len() && chars[current_pos] == '*' && chars[current_pos + 1] == '*' {
            current_pos += 2; // Lewati "**" pembuka
            let mut accum = String::new();
            let mut found_end = false;

            while current_pos < chars.len() {
                if current_pos + 1 < chars.len() && chars[current_pos] == '*' && chars[current_pos + 1] == '*' {
                    current_pos += 2; // Lewati "**" penutup
                    found_end = true;
                    break;
                }
                accum.push(chars[current_pos]);
                current_pos += 1;
            }

            if found_end {
                spans.push(Span::styled(accum, base_style.add_modifier(Modifier::BOLD)));
            } else {
                // Jika tidak ada token penutup, render literal "**"
                let mut literal = String::from("**");
                literal.push_str(&accum);
                spans.push(Span::styled(literal, base_style));
            }
        } else {
            // Teks biasa
            let mut accum = String::new();
            while current_pos < chars.len() {
                if current_pos + 1 < chars.len() && chars[current_pos] == '*' && chars[current_pos + 1] == '*' {
                    break;
                }
                accum.push(chars[current_pos]);
                current_pos += 1;
            }
            spans.push(Span::styled(accum, base_style));
        }
    }

    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base_style));
    }

    Line::from(spans)
}
