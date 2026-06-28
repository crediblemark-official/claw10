use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::TuiApp;


pub fn draw_home(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let banner_content = include_str!("../../../../assets/clawhive.txt");
    let banner_lines_count = banner_content.lines().count();
    
    // Hitung tinggi input box secara dinamis berdasarkan wrap_text dari input_buffer
    let input_inner_width = (((area.width as usize * 60) / 100).saturating_sub(1)).max(1);
    let raw_input_lines = if app.input_buffer.is_empty() {
        vec!["Ask anything... \"Spawn a new research agent\"".to_string()]
    } else {
        crate::ui::wrap_text(&app.input_buffer, input_inner_width.saturating_sub(2).max(1))
    };
    let input_lines: Vec<String> = raw_input_lines
        .into_iter()
        .map(|line| format!("  {}", line))
        .collect();
    let input_height = (input_lines.len() + 2) as u16; // input lines + 1 spacer + 1 status bar

    // Hitung tinggi konten utama (logo + spacer + input + spacer + tip) secara dinamis
    let content_height = banner_lines_count as u16 + 5 + input_height;

    // 1. Pisahkan Area Footer terlebih dahulu di bagian paling bawah
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Area utama
            Constraint::Length(1), // Footer
        ])
        .split(area);
    let main_area = main_chunks[0];
    let footer_area = main_chunks[1];

    // 2. Bagi Area Utama secara vertikal: Spacer Atas (elastis), Konten Tengah, Spacer Bawah (elastis)
    let center_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),                  // Spacer atas
            Constraint::Length(content_height),  // Konten tengah
            Constraint::Min(0),                  // Spacer bawah
        ])
        .split(main_area);
    let content_area = center_chunks[1];

    // 3. Bagi Area Konten Tengah menjadi komponen-komponennya
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(banner_lines_count as u16), // Logo
            Constraint::Length(2),                         // Spacer logo-input
            Constraint::Length(input_height),              // Input Box dinamis
            Constraint::Length(2),                         // Spacer input-tip
            Constraint::Length(1),                         // Tip
        ])
        .split(content_area);

    let mut logo_lines = Vec::new();

    let gradient_colors = [
        Color::Rgb(255, 225, 120),
        Color::Rgb(245, 205, 90),
        Color::Rgb(230, 190, 65),
        Color::Rgb(220, 180, 55),
        Color::Rgb(205, 165, 40),
        Color::Rgb(184, 134, 11),
    ];

    for (i, line) in banner_content.lines().enumerate() {
        let color = gradient_colors.get(i).copied().unwrap_or(Color::Rgb(184, 134, 11));
        logo_lines.push(Line::from(Span::styled(
            line,
            Style::default().fg(color),
        )));
    }

    let logo = Paragraph::new(logo_lines).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(logo, inner_chunks[0]);

    // Pembagian horizontal di tengah (lebar 60%) agar input box tidak full-width
    let horizontal_input_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(inner_chunks[2]);
    let input_box_area = horizontal_input_layout[1];

    // 2. Input Box (dengan border kiri Cyan/Blue dan background gelap)
    let input_block = Block::default().borders(Borders::LEFT).border_style(
        Style::default()
            .fg(Color::Rgb(218, 165, 32))
            .add_modifier(Modifier::BOLD),
    );

    let input_inner = input_block.inner(input_box_area);

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

    let left_len = 2 + 3 + 3 + active_model_name.len() + 1 + provider_name.len();
    let right_len = 40; // panjang visual dari: "/ commands  : terminal  ctrl+p palette  "
    
    let spacer_len = (input_inner.width as usize)
        .saturating_sub(left_len)
        .saturating_sub(right_len)
        .max(1);
    let middle_spacer = " ".repeat(spacer_len);

    let mut input_widget_lines = Vec::new();
    let text_style = if app.input_buffer.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };
    for line in &input_lines {
        input_widget_lines.push(Line::from(Span::styled(line.clone(), text_style)));
    }
    input_widget_lines.push(Line::from("")); // Spacer
    input_widget_lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "TUI",
            Style::default()
                .fg(Color::Rgb(218, 165, 32))
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

    let input_widget = Paragraph::new(input_widget_lines)
        .block(input_block);
    frame.render_widget(input_widget, input_box_area);

    let cursor_pos = if app.input_buffer.is_empty() {
        (input_inner.x + 2, input_inner.y)
    } else {
        let last_line = input_lines.last().cloned().unwrap_or_default();
        (
            input_inner.x + last_line.len() as u16,
            input_inner.y + (input_lines.len() - 1) as u16,
        )
    };
    frame.set_cursor_position(cursor_pos);

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
        Span::styled(":help", Style::default().fg(Color::Rgb(218, 165, 32))),
        Span::raw(" untuk perintah terminal."),
    ]);
    let tip = Paragraph::new(tip_line).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(tip, inner_chunks[4]);

    // 5. Footer
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(footer_area);

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
