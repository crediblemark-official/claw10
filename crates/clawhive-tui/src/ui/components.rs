use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{CommandMode, TuiApp};

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

pub fn draw_slash_autocomplete(frame: &mut Frame, input_area: Rect, app: &TuiApp) {
    if let CommandMode::SlashAutocomplete {
        selected_index,
        filtered_commands,
    } = &app.command_mode
    {
        if !filtered_commands.is_empty() {
            let popup_height = (filtered_commands.len() + 2).min(8) as u16;
            let popup_area = Rect {
                x: input_area.x,
                y: input_area.y.saturating_sub(popup_height),
                width: input_area.width,
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
                        Span::styled(format!("  {:<12}", cmd), Style::default()),
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
            Span::styled(search_query.as_str(), Style::default())
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
                    Span::styled(format!("  {}", name), Style::default()),
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
