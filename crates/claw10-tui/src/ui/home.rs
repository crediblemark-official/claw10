use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::TuiApp;

pub fn draw_home(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let banner_content = include_str!("../../../../assets/claw10.txt");
    let banner_lines_count = banner_content.lines().count();

    // Tinggi komponen
    let form_height: u16 = 4;
    let ws_list_height: u16 = (app.workspaces.len().min(8) as u16) + 2; // max 8 + border
    let tip_height: u16 = 1;
    let content_height = banner_lines_count as u16 + 2 + form_height + 1 + ws_list_height + 2 + tip_height;

    // Layout utama
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Area utama
            Constraint::Length(1), // Footer
            Constraint::Length(1), // Spacer bawah agar tidak mepet ke batas bawah
        ])
        .split(area);
    let main_area = main_chunks[0];
    let footer_area = main_chunks[1];

    // Vertikal centering
    let center_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(content_height),
            Constraint::Min(0),
        ])
        .split(main_area);
    let content_area = center_chunks[1];

    // Inner layout: logo | spacer | form | spacer | list | spacer | tip
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(banner_lines_count as u16), // 0: Logo
            Constraint::Length(2),                          // 1: Spacer
            Constraint::Length(form_height),                // 2: Create Workspace form
            Constraint::Length(1),                          // 3: Spacer
            Constraint::Length(ws_list_height),             // 4: Workspace list
            Constraint::Length(2),                          // 5: Spacer
            Constraint::Length(tip_height),                 // 6: Tip
        ])
        .split(content_area);

    // ── 0. Logo ─────────────────────────────────────────────────────────────
    let gradient_colors = [
        Color::Rgb(255, 225, 120),
        Color::Rgb(245, 205, 90),
        Color::Rgb(230, 190, 65),
        Color::Rgb(220, 180, 55),
        Color::Rgb(205, 165, 40),
        Color::Rgb(184, 134, 11),
    ];
    let logo_lines: Vec<Line> = banner_content
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let color = gradient_colors.get(i).copied().unwrap_or(Color::Rgb(184, 134, 11));
            Line::from(Span::styled(line, Style::default().fg(color)))
        })
        .collect();
    let logo = Paragraph::new(logo_lines).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(logo, inner_chunks[0]);

    // ── 2. Form Create Workspace (lebar 60%, tengah) ─────────────────────────
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(inner_chunks[2]);
    let form_area = horiz[1];

    let form_border_style = if app.workspace_input.is_empty() {
        Style::default().fg(Color::Rgb(218, 165, 32))
    } else {
        Style::default().fg(Color::Rgb(255, 200, 80)).add_modifier(Modifier::BOLD)
    };

    let form_block = Block::default()
        .title(" Create New Workspace ")
        .title_style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(form_border_style);

    let form_inner = form_block.inner(form_area);

    let input_display = if app.workspace_input.is_empty() {
        Span::styled("  Nama workspace baru...", Style::default().fg(Color::Rgb(120, 120, 120)))
    } else {
        Span::styled(format!("  {}", app.workspace_input), Style::default().fg(Color::White))
    };

    let form_widget = Paragraph::new(Line::from(input_display)).block(form_block);
    frame.render_widget(form_widget, form_area);

    // Posisi kursor di dalam form
    let cursor_x = form_inner.x + 2 + app.workspace_input.len() as u16;
    let cursor_y = form_inner.y;
    frame.set_cursor_position((cursor_x, cursor_y));

    // ── 4. Workspace List ─────────────────────────────────────────────────────
    let list_horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(inner_chunks[4]);
    let list_area = list_horiz[1];

    if app.workspaces.is_empty() {
        let empty_msg = Paragraph::new(Line::from(vec![
            Span::styled("  Belum ada workspace. Buat workspace pertama di atas.", Style::default().fg(Color::Rgb(150, 150, 150))),
        ]))
        .block(Block::default()
            .title(" Workspaces ")
            .title_style(Style::default().fg(Color::Rgb(100, 100, 100)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(50, 50, 50))));
        frame.render_widget(empty_msg, list_area);
    } else {
        let items: Vec<ListItem> = app
            .workspaces
            .iter()
            .enumerate()
            .map(|(i, ws)| {
                let is_selected = i == app.workspace_selected_index;
                let (prefix_style, bg) = if is_selected {
                    (
                        Style::default().fg(Color::Black).bg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD),
                        Color::Rgb(218, 165, 32),
                    )
                } else {
                    (Style::default().fg(Color::White), Color::Reset)
                };

                // Format tanggal terakhir digunakan
                let last_used = ws.last_used_at
                    .format("%Y-%m-%d %H:%M")
                    .to_string();

                let prefix = if is_selected { "▶ " } else { "  " };
                let name_span = Span::styled(
                    format!("{}{:<30}", prefix, ws.name.chars().take(28).collect::<String>()),
                    prefix_style,
                );
                let time_span = Span::styled(
                    format!("  {}", last_used),
                    if is_selected {
                        Style::default().fg(Color::Black).bg(bg)
                    } else {
                        Style::default().fg(Color::Rgb(140, 140, 140))
                    },
                );
                let id_span = Span::styled(
                    format!("  [{}]", ws.id),
                    if is_selected {
                        Style::default().fg(Color::Rgb(80, 60, 0)).bg(bg)
                    } else {
                        Style::default().fg(Color::Rgb(100, 100, 100))
                    },
                );

                ListItem::new(Line::from(vec![name_span, time_span, id_span]))
            })
            .collect();

        let ws_list = List::new(items)
            .block(Block::default()
                .title(format!(" Workspaces ({}) ", app.workspaces.len()))
                .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(80, 70, 30))));
        frame.render_widget(ws_list, list_area);
    }

    // ── 6. Tip ────────────────────────────────────────────────────────────────
    let tip_line = Line::from(vec![
        Span::styled("● ", Style::default().fg(Color::Yellow)),
        Span::styled("Tip  ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Ketik nama workspace baru dan "),
        Span::styled("Enter", Style::default().fg(Color::Rgb(218, 165, 32))),
        Span::raw(" untuk membuat, atau pilih dengan "),
        Span::styled("↑↓", Style::default().fg(Color::Rgb(218, 165, 32))),
        Span::raw(" + "),
        Span::styled("Enter / Tab", Style::default().fg(Color::Rgb(218, 165, 32))),
        Span::raw(" untuk membuka."),
    ]);
    let tip = Paragraph::new(tip_line).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(tip, inner_chunks[6]);

    // ── Footer ────────────────────────────────────────────────────────────────
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    frame.render_widget(
        Paragraph::new(version)
            .style(Style::default().fg(Color::Rgb(140, 140, 140)))
            .alignment(ratatui::layout::Alignment::Center),
        footer_area,
    );
}
