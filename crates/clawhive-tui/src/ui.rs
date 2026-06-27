use clawhive_domain::AgentState;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::{Screen, Tab, TuiApp};

fn draw_home(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(10), // Spacer atas sedikit dinaikkan agar seimbang di tengah
            Constraint::Length(15),     // Logo setinggi 15 baris (9 ikon + 1 spacer + 5 teks)
            Constraint::Length(2),      // Spacer logo-input
            Constraint::Length(4),      // Input Box (height 4 untuk text + model info di dalam)
            Constraint::Length(1),      // Sub-input info
            Constraint::Length(2),      // Spacer input-tip
            Constraint::Length(1),      // Tip
            Constraint::Min(0),         // Spacer bawah
            Constraint::Length(1),      // Footer
        ])
        .split(area);

    // 1. Logo ASCII Baru dengan Warna Emas (Rgb(212, 175, 55)) & Gradient Teks Emas Kuning dari Aset
    let gold = Color::Rgb(212, 175, 55);

    let mut logo_lines = vec![
        Line::from(Span::styled(
            r"         ________   ____",
            Style::default().fg(gold),
        )),
        Line::from(Span::styled(
            r"     ___/   \_/   \_/____\",
            Style::default().fg(gold),
        )),
        Line::from(Span::styled(
            r"   _/   \_  :       /____\",
            Style::default().fg(gold),
        )),
        Line::from(Span::styled(
            r"  /|     :  :  .   /      \",
            Style::default().fg(gold),
        )),
        Line::from(Span::styled(
            r" | |    :|  ||    \ \____/",
            Style::default().fg(gold),
        )),
        Line::from(Span::styled(
            r" | |    ||  ||     |\ / |",
            Style::default().fg(gold),
        )),
        Line::from(Span::styled(
            r"  \|    ||  ||     | /_/_",
            Style::default().fg(gold),
        )),
        Line::from(Span::styled(
            r"   | __ || __ | __ |/__/ \",
            Style::default().fg(gold),
        )),
        Line::from(Span::styled(
            r"    \_-_/  \_-_/ \_-/  \_/",
            Style::default().fg(gold),
        )),
        Line::from(""),
    ];

    let gradient_colors = [
        Color::Rgb(255, 225, 120),
        Color::Rgb(245, 205, 90),
        Color::Rgb(230, 190, 65),
        Color::Rgb(205, 165, 40),
        Color::Rgb(184, 134, 11),
    ];

    let ansi_content = include_str!("../../../assets/clawhive-gold-gradient.ans");
    for (i, line) in ansi_content.lines().enumerate() {
        let color = gradient_colors.get(i).copied().unwrap_or(Color::Rgb(184, 134, 11));
        logo_lines.push(Line::from(Span::styled(
            line,
            Style::default().fg(color),
        )));
    }

    let logo = Paragraph::new(logo_lines).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(logo, chunks[1]);

    // Pembagian horizontal di tengah (lebar 60%) agar input box tidak full-width
    let horizontal_input_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(chunks[3]);
    let input_box_area = horizontal_input_layout[1];

    let horizontal_sub_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(chunks[4]);
    let sub_info_area = horizontal_sub_layout[1];

    // 2. Input Box (dengan border kiri Cyan/Blue dan background gelap)
    let input_block = Block::default().borders(Borders::LEFT).border_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let input_inner = input_block.inner(input_box_area);

    let lines = if app.input_buffer.is_empty() {
        vec![
            Line::from(""), // Padding atas
            Line::from(Span::styled(
                "  Ask anything... \"Spawn a new research agent\"",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Build",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" · Kimi K2.7 Code ", Style::default().fg(Color::White)),
                Span::styled("OpenCode Go", Style::default().fg(Color::DarkGray)),
            ]),
        ]
    } else {
        vec![
            Line::from(""), // Padding atas
            Line::from(Span::styled(
                format!("  {}", app.input_buffer),
                Style::default().fg(Color::White),
            )),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Build",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" · Kimi K2.7 Code ", Style::default().fg(Color::White)),
                Span::styled("OpenCode Go", Style::default().fg(Color::DarkGray)),
            ]),
        ]
    };

    let input_widget = Paragraph::new(lines)
        .style(Style::default().bg(Color::Rgb(30, 30, 30)))
        .block(input_block);
    frame.render_widget(input_widget, input_box_area);

    // Set cursor position di baris tengah
    frame.set_cursor_position((
        input_inner.x + 2 + app.input_buffer.len() as u16,
        input_inner.y + 1,
    ));

    // 3. Sub-input info (Shortcuts saja di bawah kanan di luar box)
    let shortcuts_spans = vec![
        Span::styled("tab", Style::default().fg(Color::White)),
        Span::styled(" agents  ", Style::default().fg(Color::DarkGray)),
        Span::styled("ctrl+p", Style::default().fg(Color::White)),
        Span::styled(" commands", Style::default().fg(Color::DarkGray)),
    ];
    let shortcuts =
        Paragraph::new(Line::from(shortcuts_spans)).alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(shortcuts, sub_info_area);

    // 4. Tip
    let tip_line = Line::from(vec![
        Span::styled("●", Style::default().fg(Color::Yellow)),
        Span::styled(
            " Tip",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Ketik prompt dan tekan Enter untuk menjalankan agen. Gunakan "),
        Span::styled(":help", Style::default().fg(Color::Cyan)),
        Span::raw(" untuk perintah terminal."),
    ]);
    let tip = Paragraph::new(tip_line).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(tip, chunks[6]);

    // 5. Footer
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[8]);

    let current_dir = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "~/PROJECT/clawhive".to_string());

    let repo_info = format!("{}:master", current_dir);
    frame.render_widget(
        Paragraph::new(repo_info).style(Style::default().fg(Color::DarkGray)),
        footer_chunks[0],
    );

    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    frame.render_widget(
        Paragraph::new(version)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Right),
        footer_chunks[1],
    );
}

fn draw_chat(frame: &mut Frame, area: Rect, app: &TuiApp) {
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

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Chat history
            Constraint::Length(4), // Input Box (height 4 untuk text + model info di dalam)
            Constraint::Length(1), // Sub-input info
        ])
        .split(main_chunks[0]);

    // 1. Chat History (Render manual menggunakan sub-layout dinamis agar background solid dan rapi)
    // Tambahkan 1 baris spacer kosong di paling atas agar chat tidak mepet ke atas
    let chat_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Spacer kosong di paling atas
            Constraint::Min(0),    // Area utama chat history
        ])
        .split(left_chunks[0]);
    let chat_history_area = chat_layout[1];

    let max_height = chat_history_area.height as i16;
    let mut current_height = 0;
    let mut visible_chats = Vec::new();

    for (sender, model, msg) in app.chat_history.iter().rev() {
        let lines_count = msg.lines().count() as i16;
        let is_box = sender.to_lowercase() == "user" || sender.to_lowercase() == "system";
        let item_height = if is_box {
            lines_count + 2 + 1 // 2 untuk padding vertikal + 1 margin
        } else {
            1 + 1 + lines_count + 1 // 1 label + 1 blank line + lines_count + 1 margin
        };

        if current_height + item_height <= max_height {
            visible_chats.push((sender, model, msg, item_height));
            current_height += item_height;
        } else {
            break; // Tidak cukup tinggi layar
        }
    }
    visible_chats.reverse(); // Kembalikan ke urutan kronologis

    // Buat sub-layout constraints
    let mut constraints = Vec::new();
    for (_, _, _, h) in &visible_chats {
        constraints.push(Constraint::Length(*h as u16));
    }
    constraints.push(Constraint::Min(0)); // Spacer sisa di bawah

    let chat_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(chat_history_area);

    for (idx, (sender, model, msg, _)) in visible_chats.into_iter().enumerate() {
        let area = chat_areas[idx];
        // Kurangi tinggi 1 baris untuk pemisah (margin bawah)
        let bubble_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: (area.height as i16 - 1).max(0) as u16,
        };

        // Tambahkan gap horizontal 2 spasi dari tepi kiri layar dan tepi kanan (pembatas)
        let horizontal_bubble_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // Margin kiri 2 spasi
                Constraint::Min(0),    // Gelembung chat utama
                Constraint::Length(2), // Margin kanan 2 spasi
            ])
            .split(bubble_area);
        let active_bubble_area = horizontal_bubble_layout[1];

        let is_user = sender.to_lowercase() == "user";
        let is_system = sender.to_lowercase() == "system";

        if is_user || is_system {
            let border_color = if is_user { Color::Cyan } else { Color::Red };
            let input_block = Block::default()
                .borders(Borders::LEFT)
                .border_style(
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                )
                .style(Style::default().bg(Color::Indexed(236))); // Background abu-abu gelap solid

            let mut lines = Vec::new();
            lines.push(Line::from("")); // Padding vertikal atas
            for part in msg.lines() {
                lines.push(Line::from(format!("  {}", part))); // Padding horizontal kiri
            }
            lines.push(Line::from("")); // Padding vertikal bawah

            let p = Paragraph::new(lines).block(input_block);
            frame.render_widget(p, active_bubble_area);
        } else {
            // Agent / Assistant (Respon polos dengan padding kiri 2 spasi)
            let mut lines = Vec::new();
            // Solid blue box icon: ■
            lines.push(Line::from(vec![
                Span::raw("  "), // Padding kiri 2 spasi
                Span::styled("■ ", Style::default().fg(Color::Cyan)),
                Span::styled(model.as_str(), Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from("")); // Blank line
            for part in msg.lines() {
                lines.push(Line::from(vec![
                    Span::raw("  "), // Padding kiri 2 spasi
                    Span::styled(part, Style::default().fg(Color::White)),
                ]));
            }

            let p = Paragraph::new(lines);
            frame.render_widget(p, active_bubble_area);
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
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let input_inner = input_block.inner(active_input_area);

    let chat_input_lines = if app.input_buffer.is_empty() {
        vec![
            Line::from(""), // Padding atas
            Line::from(Span::styled(
                "  Ketik pesan di sini...",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Build",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" · Kimi K2.7 Code ", Style::default().fg(Color::White)),
                Span::styled("OpenCode Go", Style::default().fg(Color::DarkGray)),
            ]),
        ]
    } else {
        vec![
            Line::from(""), // Padding atas
            Line::from(Span::styled(
                format!("  {}", app.input_buffer),
                Style::default().fg(Color::White),
            )),
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Build",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" · Kimi K2.7 Code ", Style::default().fg(Color::White)),
                Span::styled("OpenCode Go", Style::default().fg(Color::DarkGray)),
            ]),
        ]
    };

    let input_widget = Paragraph::new(chat_input_lines)
        .style(Style::default().bg(Color::Rgb(30, 30, 30)))
        .block(input_block);
    frame.render_widget(input_widget, active_input_area);

    frame.set_cursor_position((
        input_inner.x + 2 + app.input_buffer.len() as u16,
        input_inner.y + 1,
    ));

    // 3. Sub-input info (Tambahkan margin horizontal 2 spasi kiri-kanan agar sejajar)
    let horizontal_info_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(2), // Margin kiri 2 spasi
            Constraint::Min(0),    // Sub-info utama
            Constraint::Length(2), // Margin kanan 2 spasi
        ])
        .split(left_chunks[2]);
    let active_info_area = horizontal_info_layout[1];

    let shortcuts_spans = vec![
        Span::styled("tab", Style::default().fg(Color::White)),
        Span::styled(" agents  ", Style::default().fg(Color::DarkGray)),
        Span::styled("ctrl+p", Style::default().fg(Color::White)),
        Span::styled(" commands", Style::default().fg(Color::DarkGray)),
    ];
    let shortcuts =
        Paragraph::new(Line::from(shortcuts_spans)).alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(shortcuts, active_info_area);

    if show_sidebar {
        // --- KOLOM KANAN (SIDEBAR) ---
        // Sidebar block dengan background hitam pekat solid untuk membedakannya dari chat area
        let sidebar_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::Black));
        let sidebar_inner = sidebar_block.inner(main_chunks[1]);
        frame.render_widget(sidebar_block, main_chunks[1]);

        // Berikan padding horizontal 2 spasi di dalam sidebar agar teks tidak mepet border
        let horizontal_sidebar_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // Padding kiri dari garis pembatas
                Constraint::Min(0),    // Area utama sidebar
                Constraint::Length(2), // Padding kanan dari tepi terminal
            ])
            .split(sidebar_inner);
        let active_sidebar_area = horizontal_sidebar_layout[1];

        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab header
                Constraint::Min(0),    // Tab content
                Constraint::Length(3), // Footer sidebar
            ])
            .split(active_sidebar_area);

        // Tab Header (Lebih minimalis dan modern tanpa bracket [])
        let tab_titles = vec!["Session", "Agents", "Workers", "Spawn"];
        let tab_spans: Vec<Span> = tab_titles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let is_selected = match app.selected_tab {
                    Tab::Session => i == 0,
                    Tab::Agents => i == 1,
                    Tab::Workers => i == 2,
                    Tab::SpawnRequests => i == 3,
                };
                if is_selected {
                    Span::styled(
                        format!(" {} ", t),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(format!(" {} ", t), Style::default().fg(Color::DarkGray))
                }
            })
            .collect();

        let tab_header = Paragraph::new(vec![
            Line::from(""),        // Baris ke-1 kosong
            Line::from(tab_spans), // Baris ke-2 berisi teks tab
        ])
        .style(Style::default().bg(Color::Black)) // Background hitam pekat
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray))
                .style(Style::default().bg(Color::Black)),
        );
        frame.render_widget(tab_header, sidebar_chunks[0]);

        // Tab Content
        match app.selected_tab {
            Tab::Session => {
                let now_str = chrono::Utc::now()
                    .format("%Y-%m-%dT%H:%M:%S.000Z")
                    .to_string();
                let tokens_str = format!("{} tokens", app.chat_history.len() * 12);

                // Susun lines dengan kontras hierarki warna (Putih tebal untuk judul seksi, abu-abu untuk detail)
                let lines = vec![
                    Line::from(Span::styled(
                        format!("New session - {}", now_str),
                        Style::default().fg(Color::White),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Context",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        tokens_str,
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(Span::styled(
                        "0% used",
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(Span::styled(
                        "$0.00 spent",
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "LSP",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        "LSPs are disabled",
                        Style::default().fg(Color::DarkGray),
                    )),
                ];

                let session_para = Paragraph::new(lines)
                    .style(Style::default().bg(Color::Black)) // Background hitam pekat
                    .wrap(Wrap { trim: false });
                frame.render_widget(session_para, sidebar_chunks[1]);
            }
            Tab::Agents => {
                let items: Vec<ListItem> = app
                    .agents
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        let prefix = if i == app.selected_index { "> " } else { "  " };
                        let state_color = match a.state {
                            AgentState::Active => Color::Green,
                            AgentState::Paused => Color::Blue,
                            _ => Color::DarkGray,
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(Color::Cyan)),
                            Span::raw(format!("{} ", a.name)),
                            Span::styled(
                                format!("{:?}", a.state),
                                Style::default().fg(state_color),
                            ),
                        ]))
                    })
                    .collect();
                let list = List::new(items)
                    .style(Style::default().bg(Color::Black)) // Background hitam pekat
                    .block(
                        Block::default()
                            .title(" Agents ")
                            .title_alignment(ratatui::layout::Alignment::Center)
                            .style(Style::default().bg(Color::Black)),
                    );
                frame.render_widget(list, sidebar_chunks[1]);
            }
            Tab::Workers => {
                let items: Vec<ListItem> = app
                    .workers
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        let prefix = if i == app.selected_index { "> " } else { "  " };
                        ListItem::new(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(Color::Cyan)),
                            Span::raw(format!("{} ", w.name)),
                            Span::styled(
                                format!("{:?}", w.state),
                                Style::default().fg(Color::Green),
                            ),
                        ]))
                    })
                    .collect();
                let list = List::new(items)
                    .style(Style::default().bg(Color::Black)) // Background hitam pekat
                    .block(
                        Block::default()
                            .title(" Workers ")
                            .title_alignment(ratatui::layout::Alignment::Center)
                            .style(Style::default().bg(Color::Black)),
                    );
                frame.render_widget(list, sidebar_chunks[1]);
            }
            Tab::SpawnRequests => {
                let items: Vec<ListItem> = app
                    .spawn_requests
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let prefix = if i == app.selected_index { "> " } else { "  " };
                        let display_id = if r.id.0.to_string().len() > 8 {
                            r.id.0.to_string()[..8].to_string()
                        } else {
                            r.id.0.to_string()
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(Color::Cyan)),
                            Span::raw(format!("{} ", display_id)),
                            Span::styled(
                                format!("{:?}", r.state),
                                Style::default().fg(Color::Yellow),
                            ),
                        ]))
                    })
                    .collect();
                let list = List::new(items)
                    .style(Style::default().bg(Color::Black)) // Background hitam pekat
                    .block(
                        Block::default()
                            .title(" Spawn Requests ")
                            .title_alignment(ratatui::layout::Alignment::Center)
                            .style(Style::default().bg(Color::Black)),
                    );
                frame.render_widget(list, sidebar_chunks[1]);
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
                Span::styled(base_path, Style::default().fg(Color::DarkGray)),
                Span::styled(":master", Style::default().fg(Color::White)),
            ])
        } else {
            Line::from(vec![
                Span::styled(base_path, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    folder_name,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(":master", Style::default().fg(Color::DarkGray)),
            ])
        };

        let footer_text = Paragraph::new(vec![
            repo_line,
            Line::from(vec![
                Span::styled("● ", Style::default().fg(Color::Green)),
                Span::styled(
                    "ClawHive ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("0.1.0", Style::default().fg(Color::DarkGray)),
            ]),
        ])
        .style(Style::default().bg(Color::Black)); // Background hitam pekat
        frame.render_widget(footer_text, sidebar_chunks[2]);
    }

    // Render Slash Autocomplete Pop-up tepat di atas input box jika aktif
    if let crate::app::CommandMode::SlashAutocomplete {
        selected_index,
        filtered_commands,
    } = &app.command_mode
    {
        if !filtered_commands.is_empty() {
            let popup_height = (filtered_commands.len() + 2).min(8) as u16;
            let popup_area = Rect {
                x: active_input_area.x,
                y: active_input_area.y.saturating_sub(popup_height),
                width: active_input_area.width,
                height: popup_height,
            };

            let block = Block::default()
                .borders(Borders::LEFT)
                .border_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .style(Style::default().bg(Color::Rgb(15, 15, 15))); // Background super gelap

            let mut lines = Vec::new();
            for (i, (cmd, desc)) in filtered_commands.iter().enumerate() {
                let is_selected = i == *selected_index;
                let line = if is_selected {
                    Line::from(vec![
                        Span::styled(
                            format!("  {:<12}", cmd),
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Rgb(254, 192, 126))
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("  {}", desc),
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Rgb(254, 192, 126)),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::styled(format!("  {:<12}", cmd), Style::default().fg(Color::White)),
                        Span::styled(format!("  {}", desc), Style::default().fg(Color::DarkGray)),
                    ])
                };
                lines.push(line);
            }

            let p = Paragraph::new(lines)
                .block(block)
                .style(Style::default().bg(Color::Rgb(15, 15, 15)));

            frame.render_widget(ratatui::widgets::Clear, popup_area);
            frame.render_widget(p, popup_area);
        }
    }
}

pub fn draw(frame: &mut Frame, area: Rect, app: &TuiApp) {
    match app.active_screen {
        Screen::Home => draw_home(frame, area, app),
        Screen::Chat => draw_chat(frame, area, app),
    }

    // Render Command Palette Modal (Ctrl+P) di atas layar apa pun jika aktif
    if let crate::app::CommandMode::CommandPalette {
        search_query,
        selected_index,
        filtered_items,
    } = &app.command_mode
    {
        let palette_area = get_fixed_centered_rect(65, 18, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::Rgb(15, 15, 15))); // Background hitam pekat modal

        let inner_area = block.inner(palette_area);

        frame.render_widget(ratatui::widgets::Clear, palette_area);
        frame.render_widget(block, palette_area);

        let palette_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Search
                Constraint::Length(1), // Spacer
                Constraint::Min(0),    // List
            ])
            .split(inner_area);

        // --- 0. Render Header ---
        let header_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(palette_chunks[0]);

        let header_left = Paragraph::new("Commands").style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        let header_right = Paragraph::new("esc")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Right);

        frame.render_widget(header_left, header_chunks[0]);
        frame.render_widget(header_right, header_chunks[1]);

        // --- 2. Render Search Box ---
        let search_text = if search_query.is_empty() {
            Span::styled("Search", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(search_query.as_str(), Style::default().fg(Color::White))
        };
        let search_para = Paragraph::new(Line::from(vec![
            Span::raw(" "), // Padding kiri 1 spasi
            search_text,
        ]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));
        frame.render_widget(search_para, palette_chunks[2]);

        // --- 4. Render List ---
        let mut list_lines = Vec::new();
        let mut current_category = String::new();

        for (flat_idx, (category, name, shortcut, _)) in filtered_items.iter().enumerate() {
            if category != &current_category {
                current_category = category.clone();
                list_lines.push(Line::from("")); // Spacer kategori
                list_lines.push(Line::from(vec![Span::styled(
                    format!(" {}", current_category),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )]));
            }

            let is_selected = flat_idx == *selected_index;
            let item_line = if is_selected {
                let spaces_needed =
                    (inner_area.width as usize).saturating_sub(name.len() + shortcut.len() + 6);
                let padding = " ".repeat(spaces_needed);
                Line::from(vec![
                    Span::styled(
                        format!("  {}", name),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(254, 192, 126))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        padding,
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(254, 192, 126)),
                    ),
                    Span::styled(
                        format!("{}  ", shortcut),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(254, 192, 126)),
                    ),
                ])
            } else {
                let spaces_needed =
                    (inner_area.width as usize).saturating_sub(name.len() + shortcut.len() + 6);
                let padding = " ".repeat(spaces_needed);
                Line::from(vec![
                    Span::styled(format!("  {}", name), Style::default().fg(Color::White)),
                    Span::raw(padding),
                    Span::styled(
                        format!("{}  ", shortcut),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            };
            list_lines.push(item_line);
        }

        let p_list = Paragraph::new(list_lines)
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(p_list, palette_chunks[4]);
    }
}

fn get_fixed_centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}
