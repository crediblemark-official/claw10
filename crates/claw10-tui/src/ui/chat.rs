use claw10_domain::{AgentState, SpawnState};
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

    // Bersihkan area chat utama (sisi kiri) dan isi dengan background hitam pekat solid
    frame.render_widget(ratatui::widgets::Clear, main_chunks[0]);
    let left_bg_block = Block::default().style(Style::default().bg(Color::Black));
    frame.render_widget(left_bg_block, main_chunks[0]);

    // Hitung tinggi input box secara dinamis berdasarkan wrap_text dari input_buffer
    let input_inner_width = (main_chunks[0].width as usize).saturating_sub(5).max(1);
    let raw_input_lines = if app.input_buffer.is_empty() {
        let placeholder = if let Some(ref ws) = app.active_workspace {
            format!("[{}] Ketik pesan di sini...", ws.name)
        } else {
            "Ketik pesan di sini...".to_string()
        };
        vec![placeholder]
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
            let total_items = app.active_suggestions.len();
            let max_visible = 10;
            let visible_count = total_items.min(max_visible);
            let mut height = 2 + visible_count + 1; // Header + visible suggestions + footer + spacer
            if total_items > max_visible {
                // start_idx > 0 akan menambah 1 baris info atas jika index berada di bawah
                if app.suggestion_index >= max_visible {
                    height += 1;
                }
                // remaining > 0 akan menambah 1 baris info bawah jika ada sisa di bawah viewport
                let start_idx = if app.suggestion_index >= max_visible {
                    app.suggestion_index - max_visible + 1
                } else {
                    0
                };
                let start_idx = start_idx.min(total_items.saturating_sub(max_visible));
                let end_idx = (start_idx + max_visible).min(total_items);
                if total_items.saturating_sub(end_idx) > 0 {
                    height += 1;
                }
            }
            suggestion_height = height;
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
        if sender.eq_ignore_ascii_case("tool") && !app.show_internal_process {
            return 1; // Hanya 1 baris ringkas
        }
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
    let mut total_needed_height: usize = app.chat_history.iter()
        .map(|(sender, _, msg)| bubble_height(sender, msg))
        .sum();

    if app.is_streaming && app.stream_status.is_some() {
        total_needed_height += 2; // 1 baris status + 1 baris spacer kosong
    }

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

            let paragraph_scroll_y = scroll_offset.saturating_sub(item_start) as u16;
            let p = Paragraph::new(lines)
                .scroll((paragraph_scroll_y, 0))
                .wrap(Wrap { trim: false });
            frame.render_widget(p, bubble_chunks[2]);
        } else if sender.to_lowercase() == "tool" {
            let mut lines = Vec::new();

            if app.show_internal_process {
                // Header: ikon kunci/tool + nama tool (Expanded)
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("🔧 ", Style::default().fg(Color::LightBlue)),
                    Span::styled(format!("Tool: {model} (F3/Ctrl+G to collapse)", model = model), Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)),
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
            } else {
                // Render ringkas (Collapsed)
                let status_icon = if msg.contains("selesai") { "✓" } else { "▶" };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("🔧 ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("Tool: {model} {status_icon} (F3/Ctrl+G to expand)", model = model), Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)),
                ]));
            }

            let paragraph_scroll_y = scroll_offset.saturating_sub(item_start) as u16;
            let p = Paragraph::new(lines)
                .scroll((paragraph_scroll_y, 0));
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
                Span::styled(model_display, Style::default().fg(Color::Gray)),
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

            let paragraph_scroll_y = scroll_offset.saturating_sub(item_start) as u16;
            let p = Paragraph::new(lines)
                .scroll((paragraph_scroll_y, 0));
            frame.render_widget(p, render_area);
        }
    }

    // Render streaming status real-time di paling bawah body chat jika sedang streaming
    if app.is_streaming {
        if let Some(ref status_text) = app.stream_status {
            let item_height = 2; // 1 baris status + 1 baris spacer kosong
            let item_start = current_y_offset;
            let item_end = item_start + item_height;

            if item_end > scroll_offset {
                let relative_y = item_start.saturating_sub(scroll_offset) as u16;
                if relative_y < chat_area.height {
                    let visible_height = (item_end - scroll_offset).min(chat_area.height as usize) - relative_y as usize;
                    if visible_height > 0 {
                        let render_area = Rect {
                            x: chat_area.x,
                            y: chat_area.y + relative_y,
                            width: chat_area.width,
                            height: visible_height as u16,
                        };

                        let mut lines = Vec::new();
                        const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                        let spinner = SPINNER_FRAMES[app.spinner_tick % SPINNER_FRAMES.len()];
                        
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(format!("{} ", spinner), Style::default().fg(Color::Yellow)),
                            Span::styled(status_text, Style::default().fg(Color::Rgb(254, 192, 126)).add_modifier(Modifier::ITALIC)),
                            Span::styled(" (F3/Ctrl+G to expand processes)", Style::default().fg(Color::Gray)),
                        ]));
                        lines.push(Line::from("")); // Spacer bawah

                        let paragraph_scroll_y = scroll_offset.saturating_sub(item_start) as u16;
                        let p = Paragraph::new(lines)
                            .scroll((paragraph_scroll_y, 0));
                        frame.render_widget(p, render_area);
                    }
                }
            }
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
        format!("TUI {} Merespons...", spinner)
    } else {
        "TUI".to_string()
    };
    let status_color = if app.is_streaming { Color::Yellow } else { Color::Rgb(218, 165, 32) };

    let _left_len = 2 + status_label.len() + 3 + active_model_name.len() + 1 + provider_name.len();
    let workspace_label = if let Some(ref ws) = app.active_workspace {
        format!("⬡ {} ", ws.name)
    } else {
        String::new()
    };

    // right_len: workspace label + shortcut hints + esc hint
    let _right_len = workspace_label.len() + 56;

    let mut chat_input_lines = Vec::new();
    if let Some(ref req) = app.pending_tool_approval {
        chat_input_lines.push(Line::from(vec![
            Span::styled("  ⚠️  PERSETUJUAN EKSEKUSI SHELL: ", Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
            Span::styled(&req.tool_name, Style::default().fg(Color::White).add_modifier(Modifier::ITALIC)),
        ]));
        chat_input_lines.push(Line::from(vec![
            Span::styled("     $ ", Style::default().fg(Color::Gray)),
            Span::styled(&req.command, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]));
        chat_input_lines.push(Line::from(""));
        chat_input_lines.push(Line::from(vec![
            Span::styled("     Ketik di form input: ", Style::default().fg(Color::White)),
            Span::styled(":approve", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" / ", Style::default()),
            Span::styled(":approve always", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
            Span::styled(" / ", Style::default()),
            Span::styled(":deny", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
    } else {
        // Tambahkan suggestions jika input diawali dengan '/'
        if app.input_buffer.starts_with('/') {
            if !app.input_buffer.starts_with("/model") {
                if !app.active_suggestions.is_empty() {
                    chat_input_lines.push(Line::from(Span::styled("   [Commands] ---------------------------------------------", Style::default().fg(Color::Gray))));
                    
                    let all_descs = vec![
                        ("/model <id>", "   - Ganti model aktif secara cepat"),
                        ("/help", "         - Tampilkan daftar perintah lengkap"),
                        ("/refresh", "      - Segarkan database agen dan task"),
                        ("/clear", "        - Bersihkan cache, history, dan context"),
                        ("/workspace", "    - Kembali ke Workspace Selector"),
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
                    
                    chat_input_lines.push(Line::from(Span::styled("   --------------------------------------------------------", Style::default().fg(Color::Gray))));
                    chat_input_lines.push(Line::from(""));
                }
            } else if app.input_buffer.starts_with("/model") {
                let search_query = if app.input_buffer.len() > 7 {
                    app.input_buffer[7..].trim()
                } else {
                    ""
                };
                let total_items = app.active_suggestions.len();
                let max_visible = 10;
                let mut start_idx = 0;
                if app.suggestion_index >= max_visible {
                    start_idx = app.suggestion_index - max_visible + 1;
                }
                start_idx = start_idx.min(total_items.saturating_sub(max_visible));
                let end_idx = (start_idx + max_visible).min(total_items);

                let search_indicator = if search_query.is_empty() {
                    String::new()
                } else {
                    format!(" Cari: '{}' |", search_query)
                };

                let pagination = if total_items > 0 {
                    format!(" {} {} - {} dari {} ", search_indicator, start_idx + 1, end_idx, total_items)
                } else {
                    String::new()
                };

                chat_input_lines.push(Line::from(Span::styled(
                    format!("   [Select Model]{}---------------------------------", pagination),
                    Style::default().fg(Color::Gray),
                )));

                if total_items == 0 {
                    chat_input_lines.push(Line::from(Span::styled("     (Tidak ada model terkonfigurasi cocok)", Style::default().fg(Color::Red))));
                } else {
                    if start_idx > 0 {
                        chat_input_lines.push(Line::from(Span::styled(
                            format!("     ▲ (Ada {} model sebelumnya...)", start_idx),
                            Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                        )));
                    }

                    for idx in start_idx..end_idx {
                        let (model_id, _) = &app.active_suggestions[idx];
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
                        
                        let provider = app.state.model_router.as_ref()
                            .and_then(|r| r.registry().list_profiles().iter().find(|p| &p.id == model_id).map(|p| p.provider.clone()))
                            .unwrap_or_else(|| "Unknown".to_string());
                        
                        chat_input_lines.push(Line::from(vec![
                            Span::styled(format!("{}{:<25}", prefix, model_id), style),
                            Span::styled(format!(" ({})", provider), Style::default().fg(Color::Gray)),
                        ]));
                    }

                    let remaining = total_items.saturating_sub(end_idx);
                    if remaining > 0 {
                        chat_input_lines.push(Line::from(Span::styled(
                            format!("     ▼ (Ada {} model lainnya...)", remaining),
                            Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
                        )));
                    }
                }
                chat_input_lines.push(Line::from(Span::styled("   --------------------------------------------------------", Style::default().fg(Color::Gray))));
                chat_input_lines.push(Line::from(""));
            }
        }

        let text_style = if app.input_buffer.is_empty() {
            Style::default().fg(Color::Gray)
        } else {
            Style::default()
        };
        for line in &input_lines {
            chat_input_lines.push(Line::from(Span::styled(line.clone(), text_style)));
        }
    }
    chat_input_lines.push(Line::from("")); // Spacer
    let f3_hint_label = if app.show_internal_process { " collapse  " } else { " expand  " };
    let left_len = 2 + status_label.len() + 3 + active_model_name.len() + 1 + provider_name.len();
    let right_len = 56 + 14 + f3_hint_label.len();
    let spacer_len = (input_inner.width as usize)
        .saturating_sub(left_len)
        .saturating_sub(right_len)
        .max(1);
    let middle_spacer = " ".repeat(spacer_len);

    chat_input_lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            status_label,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" · {} ", active_model_name), Style::default()),
        Span::styled(provider_name, Style::default().fg(Color::Gray)),
        Span::raw(middle_spacer),
        Span::styled("esc", Style::default()),
        Span::styled(" workspace  ", Style::default().fg(Color::Gray)),
        Span::styled("/", Style::default()),
        Span::styled(" commands  ", Style::default().fg(Color::Gray)),
        Span::styled(":", Style::default()),
        Span::styled(" terminal  ", Style::default().fg(Color::Gray)),
        Span::styled("ctrl+p", Style::default()),
        Span::styled(" palette  ", Style::default().fg(Color::Gray)),
        Span::styled("F3", Style::default()),
        Span::styled(f3_hint_label, Style::default().fg(Color::Gray)),
    ]));

    let input_widget = Paragraph::new(chat_input_lines)
        .block(input_block);
    frame.render_widget(input_widget, active_input_area);

    let cursor_pos = if app.input_buffer.is_empty() {
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
            .border_style(Style::default().fg(Color::Gray))
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

        // Gambar sidebar accordion secara penuh pada active_sidebar_area
        draw_sidebar_accordion(frame, active_sidebar_area, app);
    }
}

/// Render sidebar kanan sebagai accordion vertikal. Tab yang sedang aktif
/// di-expand untuk menampilkan konten; tab lain tetap terlihat sebagai
/// header collapsed sehingga memanfaatkan tinggi layar sempit.
fn draw_sidebar_accordion(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let bg = Style::default().bg(Color::Rgb(0, 0, 0));
    frame.render_widget(Block::default().style(bg), area);

    let tabs = [
        Tab::Session,
        Tab::Agents,
        Tab::Workers,
        Tab::SpawnRequests,
    ];

    // Bangun constraints: setiap tab collapsed = 2 baris (teks + border bottom),
    // tab aktif = Min(0) agar mengambil sisa ruang untuk konten.
    let mut constraints: Vec<Constraint> = tabs
        .iter()
        .map(|t| {
            if *t == app.selected_tab {
                Constraint::Min(0)
            } else {
                Constraint::Length(2)
            }
        })
        .collect();
    constraints.push(Constraint::Length(0));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (i, tab) in tabs.iter().enumerate() {
        let section_area = chunks[i];
        let is_expanded = *tab == app.selected_tab;

        // Header
        let count = accordion_item_count(app, *tab);
        let count_text = if count > 0 {
            format!(" [{count}]")
        } else {
            String::new()
        };
        let indicator = if is_expanded { "▼ " } else { "▶ " };
        let header_style = if is_expanded {
            Style::default()
                .fg(Color::Rgb(254, 192, 126))
                .bg(Color::Rgb(0, 0, 0))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0))
        };

        // Render header dengan border bottom jika collapsed
        let header_block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Rgb(40, 40, 40)));

        let header = Paragraph::new(Line::from(vec![
            Span::styled(indicator, header_style),
            Span::styled(format!("{}{}", accordion_tab_name(*tab), count_text), header_style),
        ]))
        .block(header_block);

        if is_expanded {
            // Jika expanded, render header tanpa border bottom di baris 1 (tinggi 1)
            let header_rect = Rect {
                x: section_area.x,
                y: section_area.y,
                width: section_area.width,
                height: 1,
            };
            let header_expanded = Paragraph::new(Line::from(vec![
                Span::styled(indicator, header_style),
                Span::styled(format!("{}{}", accordion_tab_name(*tab), count_text), header_style),
            ]));
            frame.render_widget(header_expanded, header_rect);

            // Render top spacer 1 baris kosong di bawah header
            let spacer_rect = Rect {
                x: section_area.x,
                y: section_area.y + 1,
                width: section_area.width,
                height: 1,
            };
            let spacer = Paragraph::new("").style(Style::default().bg(Color::Rgb(0, 0, 0)));
            frame.render_widget(spacer, spacer_rect);

            // Konten digambar di sisa area (disisakan 1 baris paling bawah untuk border bottom penutup, dan 1 spacer atas)
            let content_area = Rect {
                x: section_area.x,
                y: section_area.y + 2,
                width: section_area.width,
                height: section_area.height.saturating_sub(3),
            };
            if content_area.height > 0 {
                draw_accordion_content(frame, content_area, app, *tab);
            }

            // Border bottom penutup tab expanded di baris paling bawah
            let footer_rect = Rect {
                x: section_area.x,
                y: section_area.y + section_area.height.saturating_sub(1),
                width: section_area.width,
                height: 1,
            };
            let bottom_line = Paragraph::new(Line::from(""))
                .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::Rgb(40, 40, 40))));
            frame.render_widget(bottom_line, footer_rect);
        } else {
            // Jika collapsed, render block header (tinggi 2, memuat border bottom) di seluruh section_area
            frame.render_widget(header, section_area);
        }
    }
}

fn accordion_tab_name(tab: Tab) -> &'static str {
    match tab {
        Tab::Session => "Session",
        Tab::Agents => "Agents",
        Tab::Workers => "Workers",
        Tab::SpawnRequests => "Broker",
        Tab::Missions => "Missions",
        Tab::Tasks => "Tasks",
        Tab::Memory => "Memory",
        Tab::Approvals => "Approvals",
        Tab::Costs => "Costs",
        Tab::Policies => "Policies",
        Tab::Skills => "Skills",
        Tab::Artifacts => "Artifacts",
        Tab::Incidents => "Incidents",
    }
}

fn accordion_item_count(app: &TuiApp, tab: Tab) -> usize {
    match tab {
        Tab::Session => 1,
        Tab::Agents => app.agents.len(),
        Tab::Workers => app.workers.len(),
        Tab::SpawnRequests => app.spawn_requests.len(),
        Tab::Missions => app.missions.len(),
        Tab::Tasks => app.tasks.len(),
        Tab::Memory => app.memories.len(),
        Tab::Approvals => app.approvals.len(),
        Tab::Costs => app.agents.len(),
        Tab::Policies => app.policies.len(),
        Tab::Skills => app.skills.len(),
        Tab::Artifacts => app.artifacts.len(),
        Tab::Incidents => app.incidents.len(),
    }
}

fn draw_accordion_content(frame: &mut Frame, area: Rect, app: &TuiApp, tab: Tab) {
    let bg = Style::default().bg(Color::Rgb(0, 0, 0));
    frame.render_widget(Block::default().style(bg), area);

    match tab {
        Tab::Session => {
            let p = Paragraph::new(Span::styled(
                "  Default session active",
                Style::default().fg(Color::Rgb(218, 165, 32)).bg(Color::Rgb(0, 0, 0)),
            ));
            frame.render_widget(p, area);
        }
        Tab::Agents => {
            if app.agents.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No active agents.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
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
                            AgentState::Paused => Span::styled("● ", Style::default().fg(Color::Yellow).bg(Color::Rgb(0, 0, 0))),
                            AgentState::Terminated => Span::styled("● ", Style::default().fg(Color::Red).bg(Color::Rgb(0, 0, 0))),
                            _ => Span::styled("● ", Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0))),
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled("  ", Style::default().bg(Color::Rgb(0, 0, 0))),
                            status_indicator,
                            Span::styled(format!("{}", a.name), style),
                        ]))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Workers => {
            if app.workers.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No registered workers.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .workers
                    .iter()
                    .map(|w| {
                        let id_prefix = w.id.0.to_string().chars().take(8).collect::<String>();
                        ListItem::new(Line::from(vec![
                            Span::styled(format!("  {}", id_prefix), Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0))),
                            Span::styled(format!(" {:?}", w.state), Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0))),
                        ]))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::SpawnRequests => {
            if app.spawn_requests.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No spawn requests.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .spawn_requests
                    .iter()
                    .map(|r| {
                        let req_id_prefix = &r.id.0.to_string()[..8];
                        let display_name = r.team.name.replace("-team", "");
                        let name = format!("  {} ({})", display_name.chars().take(10).collect::<String>(), req_id_prefix);
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
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Missions => {
            if app.missions.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No missions.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .missions
                    .iter()
                    .map(|m| {
                        let objective = m.objective.chars().take(20).collect::<String>();
                        ListItem::new(Span::styled(
                            format!("  {} [{:?}]", objective, m.state),
                            Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)),
                        ))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Tasks => {
            if app.tasks.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No tasks.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .tasks
                    .iter()
                    .map(|t| {
                        let objective = t.objective.chars().take(20).collect::<String>();
                        ListItem::new(Span::styled(
                            format!("  {} [{:?}]", objective, t.state),
                            Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)),
                        ))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Memory => {
            if app.memories.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No memories.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .memories
                    .iter()
                    .map(|m| {
                        let scope = m.scope.chars().take(20).collect::<String>();
                        ListItem::new(Span::styled(
                            format!("  {}", scope),
                            Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)),
                        ))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Approvals => {
            if app.approvals.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No tool approvals.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .approvals
                    .iter()
                    .map(|a| {
                        let (status_str, status_color) = match a.state {
                            claw10_domain::approval::ToolApprovalState::Pending => (" Pending", Color::Yellow),
                            claw10_domain::approval::ToolApprovalState::Approved => (" Approved", Color::Green),
                            claw10_domain::approval::ToolApprovalState::Denied => (" Denied", Color::Red),
                            claw10_domain::approval::ToolApprovalState::AlwaysApproved => (" Always", Color::LightGreen),
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(format!("  {}", a.tool_name), Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0))),
                            Span::styled(status_str, Style::default().fg(status_color).bg(Color::Rgb(0, 0, 0))),
                        ]))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Costs => {
            let total: f64 = app.agents.iter().map(|a| a.total_cost_usd).sum();
            let p = Paragraph::new(vec![
                Line::from(Span::styled(
                    format!("  Total spent: ${total:.4}"),
                    Style::default().fg(Color::Rgb(218, 165, 32)).bg(Color::Rgb(0, 0, 0)),
                )),
                Line::from(Span::styled(
                    format!("  Agents: {}", app.agents.len()),
                    Style::default().fg(Color::Rgb(200, 200, 200)).bg(Color::Rgb(0, 0, 0)),
                )),
            ]);
            frame.render_widget(p, area);
        }
        Tab::Policies => {
            if app.policies.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No policies.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .policies
                    .iter()
                    .map(|p| {
                        ListItem::new(Span::styled(
                            format!("  {} ({} rules)", p.name, p.rules.len()),
                            Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)),
                        ))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Skills => {
            if app.skills.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No skills.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .skills
                    .iter()
                    .map(|s| {
                        ListItem::new(Span::styled(
                            format!("  {} [{:?}]", s.name, s.state),
                            Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)),
                        ))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Artifacts => {
            if app.artifacts.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No artifacts.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .artifacts
                    .iter()
                    .map(|a| {
                        let id_prefix = a.id.0.to_string().chars().take(8).collect::<String>();
                        ListItem::new(Span::styled(
                            format!("  {} ({})", a.name, id_prefix),
                            Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)),
                        ))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
        Tab::Incidents => {
            if app.incidents.is_empty() {
                let p = Paragraph::new(Span::styled(
                    "  No incidents.",
                    Style::default().fg(Color::Rgb(150, 150, 150)).bg(Color::Rgb(0, 0, 0)),
                ));
                frame.render_widget(p, area);
            } else {
                let items: Vec<ListItem> = app
                    .incidents
                    .iter()
                    .map(|inc| {
                        let desc = inc.description.chars().take(20).collect::<String>();
                        ListItem::new(Span::styled(
                            format!("  {} [{:?}]", desc, inc.state),
                            Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)),
                        ))
                    })
                    .collect();
                let list = List::new(items).style(Style::default().bg(Color::Rgb(0, 0, 0)));
                frame.render_widget(list, area);
            }
        }
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
