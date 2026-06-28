use ratatui::{layout::Rect, Frame};

pub mod chat;
pub mod components;
pub mod home;

use crate::app::{CommandMode, Screen, TuiApp};
use crate::ui::chat::draw_chat;
use crate::ui::components::{draw_apikey_input, draw_command_palette, draw_model_selection};
use crate::ui::home::draw_home;

pub fn draw(frame: &mut Frame, area: Rect, app: &TuiApp) {
    match app.active_screen {
        Screen::Home => draw_home(frame, area, app),
        Screen::Chat => draw_chat(frame, area, app),
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
