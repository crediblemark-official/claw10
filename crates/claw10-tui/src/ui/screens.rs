use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::TuiApp;

fn default_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(0), // Dinonaktifkan (diganti Top Bar global)
            Constraint::Min(0),    // content
            Constraint::Length(1), // footer
            Constraint::Length(1), // Spacer bawah agar tidak mepet ke batas bawah
        ])
        .split(area)
        .to_vec()
}


pub fn draw_tab_bar(_frame: &mut Frame, _area: Rect, _app: &TuiApp) {
    // No-op: dinonaktifkan karena sudah digantikan oleh Top Bar global di ui/mod.rs
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &TuiApp, title: &str) {
    let hint = format!(
        "{} | Tab: next | ↑↓: scroll | Esc/q: home | {}",
        title,
        app.status_message
    );
    let footer = Paragraph::new(hint)
        .style(Style::default().fg(Color::Rgb(140, 140, 140)))
        .alignment(Alignment::Center);
    frame.render_widget(footer, area);
}

fn empty_rows(message: &str) -> Vec<Row<'static>> {
    vec![Row::new(vec![Cell::from(format!("  {}", message)).style(Style::default().fg(Color::Rgb(140, 140, 140)))])]
}

fn draw_table_columns(frame: &mut Frame, area: Rect, percentages: &[u16]) {
    let y_start = area.y + 1;
    let height = area.height.saturating_sub(2);
    if height == 0 {
        return;
    }

    let mut current_x = area.x;
    for &pct in percentages.iter().take(percentages.len() - 1) {
        let col_width = (area.width as u32 * pct as u32 / 100) as u16;
        current_x += col_width;

        if current_x < area.x + area.width {
            let col_area = Rect {
                x: current_x,
                y: y_start,
                width: 1,
                height,
            };
            let separator = Paragraph::new("│".repeat(height as usize))
                .style(Style::default().fg(Color::Rgb(150, 120, 50)));
            frame.render_widget(separator, col_area);
        }
    }
}

pub fn draw_missions(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let header = Row::new(vec!["  Objective", "  State", "  Budget"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.missions.is_empty() {
        empty_rows("No missions found.")
    } else {
        app.missions
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let objective = m.objective.chars().take(40).collect::<String>();
                let budget = format!(
                    "${:.2} / ${:.2}",
                    m.budget.spent_usd, m.budget.allocated_usd
                );
                Row::new(vec![
                    Cell::from(format!("  {}", objective)),
                    Cell::from(format!("  {:?}", m.state)),
                    Cell::from(format!("  {}", budget)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(55),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .column_spacing(0)
    .block(
        Block::default()
            .title(" Missions ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );
    frame.render_widget(table, chunks[1]);
    draw_table_columns(frame, chunks[1], &[55, 20, 25]);
    draw_footer(frame, chunks[2], app, "Missions");
}

pub fn draw_tasks(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let header = Row::new(vec!["  Objective", "  State", "  Mission ID"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.tasks.is_empty() {
        empty_rows("No tasks found.")
    } else {
        app.tasks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let objective = t.objective.chars().take(45).collect::<String>();
                let mission_id = t.mission_id.0.to_string();
                Row::new(vec![
                    Cell::from(format!("  {}", objective)),
                    Cell::from(format!("  {:?}", t.state)),
                    Cell::from(format!("  {}", mission_id)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(55),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .column_spacing(0)
    .block(
        Block::default()
            .title(" Tasks ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );
    frame.render_widget(table, chunks[1]);
    draw_table_columns(frame, chunks[1], &[55, 20, 25]);
    draw_footer(frame, chunks[2], app, "Tasks");
}

pub fn draw_memory(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let header = Row::new(vec!["  Content Preview", "  Status", "  Scope"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.memories.is_empty() {
        empty_rows("No memories found.")
    } else {
        app.memories
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let preview = m.content.chars().take(50).collect::<String>();
                Row::new(vec![
                    Cell::from(format!("  {}", preview)),
                    Cell::from(format!("  {:?}", m.status)),
                    Cell::from(format!("  {}", m.scope)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(60),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .column_spacing(0)
    .block(
        Block::default()
            .title(" Memory ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );
    frame.render_widget(table, chunks[1]);
    draw_table_columns(frame, chunks[1], &[60, 20, 20]);
    draw_footer(frame, chunks[2], app, "Memory");
}

pub fn draw_approvals(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let header = Row::new(vec!["  Tool Name", "  Status", "  Created"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.approvals.is_empty() {
        empty_rows("No tool approvals found.")
    } else {
        app.approvals
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let created = a.created_at.format("%Y-%m-%d %H:%M").to_string();
                Row::new(vec![
                    Cell::from(format!("  {}", a.tool_name)),
                    Cell::from(format!("  {:?}", a.state)),
                    Cell::from(format!("  {}", created)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(45),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .column_spacing(0)
    .block(
        Block::default()
            .title(" Tool Approvals ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );
    frame.render_widget(table, chunks[1]);
    draw_table_columns(frame, chunks[1], &[45, 25, 30]);
    draw_footer(frame, chunks[2], app, "Approvals");
}

pub fn draw_costs(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let total_spent: f64 = app.agents.iter().map(|a| a.total_cost_usd).sum();

    let summary = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Total Spent: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("${:.4}", total_spent), Style::default().fg(Color::Rgb(218, 165, 32))),
        ]),
        Line::from(vec![
            Span::styled("Agents: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(app.agents.len().to_string()),
        ]),
    ])
    .block(
        Block::default()
            .title(" Costs Summary ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );

    let header = Row::new(vec!["  Agent Name", "  State", "  Total Cost (USD)"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.agents.is_empty() {
        empty_rows("No agents found.")
    } else {
        app.agents
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Row::new(vec![
                    Cell::from(format!("  {}", a.name)),
                    Cell::from(format!("  {:?}", a.state)),
                    Cell::from(format!("  ${:.4}", a.total_cost_usd)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(45),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .column_spacing(0)
    .block(
        Block::default()
            .title(" Agent Costs ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );

    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(chunks[1]);

    frame.render_widget(summary, content_chunks[0]);
    frame.render_widget(table, content_chunks[1]);
    draw_table_columns(frame, content_chunks[1], &[45, 25, 30]);
    draw_footer(frame, chunks[2], app, "Costs");
}

pub fn draw_policies(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let header = Row::new(vec!["  Name", "│  Active", "│  Rules"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.policies.is_empty() {
        empty_rows("No policies found.")
    } else {
        app.policies
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Row::new(vec![
                    Cell::from(format!("  {}", p.name)),
                    Cell::from(format!("│  {}", if p.is_active { "Yes" } else { "No" })),
                    Cell::from(format!("│  {}", p.rules.len())),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(55),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" Policies ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );
    frame.render_widget(table, chunks[1]);
    draw_footer(frame, chunks[2], app, "Policies");
}

pub fn draw_skills(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let header = Row::new(vec!["  Name", "│  Version", "│  State", "│  Tools"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.skills.is_empty() {
        empty_rows("No skills found.")
    } else {
        app.skills
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let tools = s.required_tools.join(", ");
                Row::new(vec![
                    Cell::from(format!("  {}", s.name)),
                    Cell::from(format!("│  {}", s.version)),
                    Cell::from(format!("│  {:?}", s.state)),
                    Cell::from(format!("│  {}", tools)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" Skills ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );
    frame.render_widget(table, chunks[1]);
    draw_footer(frame, chunks[2], app, "Skills");
}

pub fn draw_artifacts(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let header = Row::new(vec!["  Name", "│  MIME Type", "│  Size", "│  Agent / Task"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.artifacts.is_empty() {
        empty_rows("No artifacts found.")
    } else {
        app.artifacts
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let owner = format!(
                    "{} / {}",
                    a.agent_id.0.to_string().chars().take(8).collect::<String>(),
                    a.task_id.0.to_string().chars().take(8).collect::<String>()
                );
                let size = format_size(a.size_bytes);
                Row::new(vec![
                    Cell::from(format!("  {}", a.name)),
                    Cell::from(format!("│  {}", a.mime_type)),
                    Cell::from(format!("│  {}", size)),
                    Cell::from(format!("│  {}", owner)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(25),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" Artifacts ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );
    frame.render_widget(table, chunks[1]);
    draw_footer(frame, chunks[2], app, "Artifacts");
}

pub fn draw_incidents(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let chunks = default_layout(area);
    draw_tab_bar(frame, chunks[0], app);

    let header = Row::new(vec!["  Severity", "│  State", "│  Description"])
        .style(Style::default().fg(Color::Rgb(218, 165, 32)).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = if app.incidents.is_empty() {
        empty_rows("No incidents found.")
    } else {
        app.incidents
            .iter()
            .enumerate()
            .map(|(i, inc)| {
                let style = if i == app.selected_index {
                    Style::default()
                        .fg(Color::Rgb(0, 0, 0))
                        .bg(Color::Rgb(254, 192, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let desc = inc.description.chars().take(50).collect::<String>();
                Row::new(vec![
                    Cell::from(format!("  {}", inc.severity)),
                    Cell::from(format!("│  {:?}", inc.state)),
                    Cell::from(format!("│  {}", desc)),
                ])
                .style(style)
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(25),
            Constraint::Percentage(55),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" Incidents ")
            .title_style(Style::default().fg(Color::Rgb(218, 165, 32)))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(150, 120, 50))),
    );
    frame.render_widget(table, chunks[1]);
    draw_footer(frame, chunks[2], app, "Incidents");
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let exp = (bytes as f64).log(1024.0).min(UNITS.len() as f64 - 1.0) as usize;
    let value = bytes as f64 / 1024_f64.powi(exp as i32);
    if exp == 0 {
        format!("{} {}", bytes, UNITS[exp])
    } else {
        format!("{:.2} {}", value, UNITS[exp])
    }
}
