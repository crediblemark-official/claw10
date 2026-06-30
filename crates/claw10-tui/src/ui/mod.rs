use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
    Frame,
};

pub mod chat;
pub mod components;
pub mod home;
pub mod screens;

use crate::app::{CommandMode, Screen, Tab, TuiApp};
use crate::ui::chat::draw_chat;
use crate::ui::components::{draw_apikey_input, draw_command_palette, draw_model_selection};
use crate::ui::home::draw_home;
use crate::ui::screens::{
    draw_approvals, draw_artifacts, draw_costs, draw_incidents, draw_memory,
    draw_missions, draw_policies, draw_skills, draw_tasks,
};

pub fn draw(frame: &mut Frame, area: Rect, app: &TuiApp) {
    // Warnai background seluruh area terminal dengan hitam murni solid (#000000)
    let root_bg = Block::default().style(Style::default().bg(Color::Rgb(0, 0, 0)));
    frame.render_widget(root_bg, area);

    if app.active_screen == Screen::Home || app.active_screen == Screen::WorkspaceSelect {
        draw_home(frame, area, app);

        // Render modals di atas layar apa pun jika aktif
        if matches!(app.command_mode, CommandMode::CommandPalette { .. }) {
            draw_command_palette(frame, area, app);
        }
        if matches!(app.command_mode, CommandMode::ApiKeyInput { .. }) {
            draw_apikey_input(frame, area, app);
        }
        if matches!(app.command_mode, CommandMode::ModelSelection) {
            draw_model_selection(frame, area, app);
        }
        if matches!(app.command_mode, CommandMode::ManualModelInput { .. }) {
            crate::ui::components::draw_manual_model_input(frame, area, app);
        }
        return;
    }

    // Bagi area menjadi Top Bar (2 baris: 1 baris invisible border + 1 baris teks), Pembatas Bawah, dan Content Area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Top Bar dengan invisible top border
            Constraint::Length(1), // Pembatas Bawah
            Constraint::Min(0),    // Content Area
        ])
        .split(area);

    draw_top_bar(frame, chunks[0], app);

    // Garis horizontal pembatas bawah yang sleek & minimalis
    let bottom_border = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Rgb(40, 40, 40)).bg(Color::Rgb(0, 0, 0)));
    frame.render_widget(bottom_border, chunks[1]);

    let content_area = chunks[2];

    match app.active_screen {
        Screen::Home | Screen::WorkspaceSelect => unreachable!(),
        Screen::Chat => draw_chat(frame, content_area, app),
        Screen::Missions => draw_missions(frame, content_area, app),
        Screen::Tasks => draw_tasks(frame, content_area, app),
        Screen::Memory => draw_memory(frame, content_area, app),
        Screen::Approvals => draw_approvals(frame, content_area, app),
        Screen::Costs => draw_costs(frame, content_area, app),
        Screen::Policies => draw_policies(frame, content_area, app),
        Screen::Skills => draw_skills(frame, content_area, app),
        Screen::Artifacts => draw_artifacts(frame, content_area, app),
        Screen::Incidents => draw_incidents(frame, content_area, app),
    }

    // Render modals di atas layar apa pun jika aktif
    if matches!(app.command_mode, CommandMode::CommandPalette { .. }) {
        draw_command_palette(frame, area, app);
    }
    if matches!(app.command_mode, CommandMode::ApiKeyInput { .. }) {
        draw_apikey_input(frame, area, app);
    }
    if matches!(app.command_mode, CommandMode::ModelSelection) {
        draw_model_selection(frame, area, app);
    }
    if matches!(app.command_mode, CommandMode::ManualModelInput { .. }) {
        crate::ui::components::draw_manual_model_input(frame, area, app);
    }
}

fn draw_top_bar(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let active_idx = match app.selected_tab {
        Tab::Session | Tab::Agents | Tab::Workers | Tab::SpawnRequests => 0,
        Tab::Missions => 1,
        Tab::Tasks => 2,
        Tab::Memory => 3,
        Tab::Approvals => 4,
        Tab::Costs => 5,
        Tab::Policies => 6,
        Tab::Skills => 7,
        Tab::Artifacts => 8,
        Tab::Incidents => 9,
    };

    let titles = vec![
        "  Chat  ",
        "  Msn  ",
        "  Tasks  ",
        "  Mem  ",
        "  Aprv  ",
        "  Cost  ",
        "  Pol  ",
        "  Skl  ",
        "  Art  ",
        "  Inc  ",
    ];

    let tabs = ratatui::widgets::Tabs::new(titles)
        .select(active_idx)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Rgb(0, 0, 0)).bg(Color::Rgb(0, 0, 0)))
                .style(Style::default().bg(Color::Rgb(0, 0, 0)))
        )
        .style(Style::default().fg(Color::Gray).bg(Color::Rgb(0, 0, 0)))
        .highlight_style(
            Style::default()
                .fg(Color::Rgb(254, 192, 126))
                .bg(Color::Rgb(0, 0, 0))
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    frame.render_widget(tabs, area);
}

/// Melakukan word-wrap manual per baris teks ke lebar maksimal `max_width`.
/// Menjaga line breaks yang ditulis secara eksplisit dari input.
pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        for word in words {
            let space_needed = if current_line.is_empty() { 0 } else { 1 };
            if current_line.chars().count() + space_needed + word.chars().count() > max_width {
                // Baris penuh, flush ke lines
                lines.push(current_line);
                current_line = word.to_string();
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}
