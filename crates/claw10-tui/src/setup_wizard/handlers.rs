use crossterm::event::{KeyCode, KeyEvent};

use super::{SetupWizard, Step};

impl SetupWizard {
    pub(crate) fn handle_welcome(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.next_step(),
            KeyCode::Esc | KeyCode::Char('q') => self.step = Step::Complete,
            _ => {}
        }
    }

    pub(crate) fn handle_provider_select(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let filtered = self.filtered_providers();
                if self.selected + 1 < filtered.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected = self.selected.saturating_sub(5);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let filtered = self.filtered_providers();
                let next = self.selected + 5;
                if next < filtered.len() {
                    self.selected = next;
                }
            }
            KeyCode::Enter => {
                let provider = self.current_provider();
                if let Some(val) = self.configured_env_vars.get(&provider.env_var) {
                    self.api_key = val.clone();
                } else {
                    self.api_key.clear();
                }
                self.next_step();
            }
            KeyCode::Esc => {
                if self.provider_search.is_empty() {
                    self.prev_step();
                } else {
                    self.provider_search.clear();
                    self.selected = 0;
                }
            }
            KeyCode::Backspace => {
                self.provider_search.pop();
                self.selected = 0;
            }
            KeyCode::Tab | KeyCode::Char('s') | KeyCode::Char('S') => {
                self.step = Step::Review;
            }
            KeyCode::Char(c) => {
                self.provider_search.push(c);
                self.selected = 0;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_api_key_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.api_key.is_empty() {
                    self.error_msg = "API key kosong. Lewati? Tekan Esc untuk kembali.".to_string();
                }
                self.next_step();
            }
            KeyCode::Esc => {
                self.prev_step();
            }
            KeyCode::Backspace => {
                self.api_key.pop();
                self.error_msg.clear();
            }
            KeyCode::Char(c) => {
                self.api_key.push(c);
                self.error_msg.clear();
            }
            _ => {}
        }
    }

    pub(crate) fn handle_base_url_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.custom_url.is_empty() {
                    self.error_msg = "Base URL tidak boleh kosong.".to_string();
                } else {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Backspace => {
                self.custom_url.pop();
                self.error_msg.clear();
            }
            KeyCode::Char(c) => {
                self.custom_url.push(c);
                self.error_msg.clear();
            }
            _ => {}
        }
    }

    pub(crate) fn handle_model_list(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.model_list_selected > 0 {
                    self.model_list_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let filtered = self.filtered_models();
                if self.model_list_selected + 1 < filtered.len() {
                    self.model_list_selected += 1;
                }
            }
            KeyCode::Enter => {
                let filtered = self.filtered_models();
                if !self.model_search.is_empty() && filtered.is_empty() {
                    self.custom_model = self.model_search.clone();
                } else if !filtered.is_empty() {
                    self.custom_model = filtered[self.model_list_selected].to_string();
                }
                if !self.custom_model.is_empty() {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Backspace => {
                self.model_search.pop();
                self.model_list_selected = 0;
            }
            KeyCode::Char(c) => {
                self.model_search.push(c);
                self.model_list_selected = 0;
            }
            KeyCode::Tab | KeyCode::F(2) => {
                self.error_msg = "Beralih ke input manual.".to_string();
                self.step = Step::ModelSelect;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_model_select(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.custom_model.is_empty() {
                    self.error_msg = "Masukkan nama model.".to_string();
                } else {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Backspace => {
                self.custom_model.pop();
                self.error_msg.clear();
            }
            KeyCode::Char(c) => {
                self.custom_model.push(c);
                self.error_msg.clear();
            }
            _ => {}
        }
    }

    pub(crate) fn handle_telegram_setup_prompt(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.setup_telegram = true;
                self.next_step();
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.setup_telegram = false;
                self.next_step();
            }
            KeyCode::Enter => {
                self.next_step();
            }
            KeyCode::Esc => self.prev_step(),
            _ => {}
        }
    }

    pub(crate) fn handle_telegram_token_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if self.telegram_token.is_empty() {
                    self.error_msg = "Masukkan token bot Telegram atau tekan Esc.".to_string();
                } else {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            KeyCode::Backspace => {
                self.telegram_token.pop();
                self.error_msg.clear();
            }
            KeyCode::Char(c) => {
                self.telegram_token.push(c);
                self.error_msg.clear();
            }
            _ => {}
        }
    }

    pub(crate) fn start_telegram_binding_poll(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.binding_rx = Some(rx);
        if !self.telegram_chat_id.is_empty() {
            self.binding_status = format!(
                "Sudah terhubung (Chat ID: {}). Tekan Enter untuk lanjut, atau kirim /start untuk pairing ulang.",
                self.telegram_chat_id
            );
        } else {
            self.binding_status = "Buka Telegram Anda, cari bot Anda, lalu kirim pesan /start...".to_string();
        }

        let token = self.telegram_token.clone();
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(123456);
        let code = format!("{:06}", (now % 1_000_000).abs());
        self.binding_code = code.clone();

        crate::setup_service::spawn_telegram_polling_thread(token, code, tx);
    }

    pub(crate) fn handle_review(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if let Err(e) = self.save_config() {
                    self.error_msg = format!("Gagal menyimpan: {e}");
                } else {
                    self.next_step();
                }
            }
            KeyCode::Esc => self.prev_step(),
            _ => {}
        }
    }
}
