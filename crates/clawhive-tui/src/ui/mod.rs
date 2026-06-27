use ratatui::{layout::Rect, Frame};

pub mod chat;
pub mod components;
pub mod home;

use crate::app::{Screen, TuiApp};
use crate::ui::chat::draw_chat;
use crate::ui::components::draw_command_palette;
use crate::ui::home::draw_home;

pub fn draw(frame: &mut Frame, area: Rect, app: &TuiApp) {
    match app.active_screen {
        Screen::Home => draw_home(frame, area, app),
        Screen::Chat => draw_chat(frame, area, app),
    }

    // Render Command Palette Modal (Ctrl+P) di atas layar apa pun jika aktif
    draw_command_palette(frame, area, app);
}
