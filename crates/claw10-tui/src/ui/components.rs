use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{CommandMode, ModelSelectionStep, TuiApp};

pub fn get_fixed_centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}

pub fn draw_apikey_input(frame: &mut Frame, area: Rect, app: &TuiApp) {
    if let CommandMode::ApiKeyInput { key_input, error_message } = &app.command_mode {
        let modal_area = get_fixed_centered_rect(65, 8, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)))
            .title(" Set API Key ")
            .title_alignment(Alignment::Center);

        let inner = block.inner(modal_area);
        frame.render_widget(ratatui::widgets::Clear, modal_area);
        frame.render_widget(block, modal_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner);

        let provider_label = app.model_sel_pending_provider.as_deref().unwrap_or("provider");

        let input_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(provider_label.len() as u16 + 2), Constraint::Min(0)])
            .split(chunks[2]);

        let provider_tag = Paragraph::new(Line::from(vec![Span::styled(
            format!(" {}:", provider_label),
            Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD),
        )]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));
        frame.render_widget(provider_tag, input_chunks[0]);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(120, 120, 120)));
        let input_inner = input_block.inner(input_chunks[1]);
        frame.render_widget(input_block, input_chunks[1]);

        let display_text = if key_input.is_empty() {
            "sk-...".to_string()
        } else {
            key_input.clone()
        };
        let input_para = Paragraph::new(Line::from(vec![Span::styled(
            display_text,
            Style::default().fg(Color::Rgb(218, 165, 32)),
        )]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));
        frame.render_widget(input_para, input_inner);

        if !error_message.is_empty() {
            let err = Paragraph::new(Line::from(vec![Span::styled(
                format!("  {}", error_message),
                Style::default().fg(Color::Red),
            )]))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[3]);
        }

        let hint = Paragraph::new(Line::from(vec![
            Span::styled("Esc", Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD)),
            Span::raw(" back  "),
            Span::styled("Enter", Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD)),
            Span::raw(" confirm"),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(hint, chunks[0]);

        // Cursor
        let cursor_x = input_inner.x + key_input.len() as u16;
        frame.set_cursor_position((cursor_x, input_inner.y));
    }
}

pub fn draw_command_palette(frame: &mut Frame, area: Rect, app: &TuiApp) {
    if let CommandMode::CommandPalette {
        search_query,
        selected_index,
        filtered_items,
    } = &app.command_mode
    {
        let palette_area = get_fixed_centered_rect(65, 18, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(120, 120, 120)))
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
            .style(Style::default().fg(Color::Rgb(140, 140, 140)))
            .alignment(ratatui::layout::Alignment::Right);

        frame.render_widget(header_left, header_chunks[0]);
        frame.render_widget(header_right, header_chunks[1]);

        // --- 2. Render Search Box ---
        let search_text = if search_query.is_empty() {
            Span::styled("Search", Style::default().fg(Color::Rgb(120, 120, 120)))
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
        let mut current_category = "";

        for (flat_idx, (category, name, shortcut, _)) in filtered_items.iter().enumerate() {
            if category != current_category {
                current_category = category;
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
                        Style::default().fg(Color::Rgb(140, 140, 140)),
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

pub fn draw_model_selection(frame: &mut Frame, area: Rect, app: &TuiApp) {
    if !matches!(app.command_mode, CommandMode::ModelSelection) {
        return;
    }

    let title: String;
    let items: Vec<String>;

    match app.model_sel_step {
        ModelSelectionStep::SelectProvider => {
            title = "Select Provider".to_string();
            let search = app.model_sel_search.to_lowercase();
            items = app
                .all_catalog_providers()
                .iter()
                .filter(|(name, _)| search.is_empty() || name.to_lowercase().contains(&search))
                .map(|(name, configured)| {
                    if *configured {
                        format!("\u{2713} {name}")
                    } else {
                        format!("  {name}")
                    }
                })
                .collect();
        }
        ModelSelectionStep::SelectFamily => {
            title = format!("{} \u{2192} Select Model", app.model_sel_provider);
            let search = app.model_sel_search.to_lowercase();
            let mut list_items: Vec<String> = if search.is_empty() {
                app.model_sel_families.iter().map(|f| f.name.clone()).collect()
            } else {
                app.model_sel_families
                    .iter()
                    .filter(|f| f.name.to_lowercase().contains(&search))
                    .map(|f| f.name.clone())
                    .collect()
            };
            list_items.insert(0, "< Tambah Model Manual >".to_string());
            items = list_items;
        }
        ModelSelectionStep::SelectVariant => {
            title = format!("{} \u{2192} Select Type", app.model_sel_provider);
            let search = app.model_sel_search.to_lowercase();
            items = if search.is_empty() {
                app.model_sel_variants.iter().map(|v| v.id.clone()).collect()
            } else {
                app.model_sel_variants
                    .iter()
                    .filter(|v| {
                        v.id.to_lowercase().contains(&search)
                            || v.model_name.to_lowercase().contains(&search)
                            || v.suitable_for.iter().any(|t| t.to_lowercase().contains(&search))
                    })
                    .map(|v| v.id.clone())
                    .collect()
            };
        }
    };

    let modal_area = get_fixed_centered_rect(70, 20, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(120, 120, 120)))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)))
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Center);

    let inner_area = block.inner(modal_area);
    frame.render_widget(ratatui::widgets::Clear, modal_area);
    frame.render_widget(block, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner_area);

    // Search box
    let search_text = if app.model_sel_search.is_empty() {
        Span::styled("Search...", Style::default().fg(Color::Rgb(120, 120, 120)))
    } else {
        Span::styled(app.model_sel_search.as_str(), Style::default().fg(Color::White))
    };
    let search_para = Paragraph::new(Line::from(vec![
        Span::raw(" "),
        search_text,
    ]))
    .style(Style::default().bg(Color::Rgb(25, 25, 25)));
    frame.render_widget(search_para, chunks[2]);

    // Items list dengan windowed scrolling
    let visible_height = chunks[3].height as usize;
    let total_items = items.len();
    
    let mut start_idx = 0;
    if app.model_sel_index >= visible_height {
        start_idx = app.model_sel_index - visible_height + 1;
    }
    let end_idx = (start_idx + visible_height).min(total_items);

    let mut list_lines: Vec<Line> = Vec::new();
    for i in start_idx..end_idx {
        let item = &items[i];
        let is_selected = i == app.model_sel_index;
        let line = if is_selected {
            Line::from(vec![
                Span::styled(
                    format!("  {}", item),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(format!("  {}", item), Style::default().fg(Color::White)),
            ])
        };
        list_lines.push(line);
    }

    let p_list = Paragraph::new(list_lines)
        .wrap(ratatui::widgets::Wrap { trim: false })
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
    frame.render_widget(p_list, chunks[3]);

    // Esc hint
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Esc", Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD)),
        Span::raw(" to go back"),
    ]))
    .style(Style::default().bg(Color::Rgb(15, 15, 15)));
    frame.render_widget(hint, chunks[0]);
}

pub fn draw_manual_model_input(frame: &mut Frame, area: Rect, app: &TuiApp) {
    if let CommandMode::ManualModelInput { model_input, error_message } = &app.command_mode {
        let modal_area = get_fixed_centered_rect(65, 8, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)))
            .title(" Add Model Manually ")
            .title_alignment(Alignment::Center);

        let inner = block.inner(modal_area);
        frame.render_widget(ratatui::widgets::Clear, modal_area);
        frame.render_widget(block, modal_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner);

        let provider_label = &app.model_sel_provider;

        let input_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(provider_label.len() as u16 + 2), Constraint::Min(0)])
            .split(chunks[2]);

        let provider_tag = Paragraph::new(Line::from(vec![Span::styled(
            format!(" {}:", provider_label),
            Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD),
        )]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));
        frame.render_widget(provider_tag, input_chunks[0]);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(120, 120, 120)));
        let input_inner = input_block.inner(input_chunks[1]);
        frame.render_widget(input_block, input_chunks[1]);

        let display_text = if model_input.is_empty() {
            "Masukkan nama model (misal: nvidia/llama-3.1-nemotron-70b-instruct)".to_string()
        } else {
            model_input.clone()
        };
        let text_color = if model_input.is_empty() { Color::DarkGray } else { Color::Rgb(218, 165, 32) };
        let input_para = Paragraph::new(Line::from(vec![Span::styled(
            display_text,
            Style::default().fg(text_color),
        )]))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));
        frame.render_widget(input_para, input_inner);

        if !error_message.is_empty() {
            let err = Paragraph::new(Line::from(vec![Span::styled(
                format!("  {}", error_message),
                Style::default().fg(Color::Red),
            )]))
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
            frame.render_widget(err, chunks[3]);
        }

        let hint = Paragraph::new(Line::from(vec![
            Span::styled("Esc", Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD)),
            Span::raw(" back  "),
            Span::styled("Enter", Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD)),
            Span::raw(" add model"),
        ]))
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        frame.render_widget(hint, chunks[0]);

        // Cursor
        let cursor_x = input_inner.x + model_input.len() as u16;
        frame.set_cursor_position((cursor_x, input_inner.y));
    }
}
