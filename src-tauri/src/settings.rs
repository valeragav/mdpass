//! Настройки приложения для интерфейса.

use mdpass_core::config::{AppConfig, TextSize, Theme};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::session::AppState;

/// Настройки без пути к последнему хранилищу: им управляет само приложение.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub lock_on_minimize: bool,
    pub backups_enabled: bool,
    pub backups_to_keep: u32,
    pub theme: Theme,
    pub text_size: TextSize,
    pub accent: String,
    pub sidebar_width: u32,
}

impl From<&AppConfig> for Settings {
    fn from(config: &AppConfig) -> Self {
        Self {
            auto_lock_minutes: config.auto_lock_minutes,
            clipboard_clear_seconds: config.clipboard_clear_seconds,
            lock_on_minimize: config.lock_on_minimize,
            backups_enabled: config.backups_enabled,
            backups_to_keep: config.backups_to_keep,
            theme: config.theme,
            text_size: config.text_size,
            accent: config.accent.clone(),
            sidebar_width: config.sidebar_width,
        }
    }
}

impl Settings {
    fn apply(self, config: &mut AppConfig) {
        config.auto_lock_minutes = self.auto_lock_minutes;
        config.clipboard_clear_seconds = self.clipboard_clear_seconds;
        config.lock_on_minimize = self.lock_on_minimize;
        config.backups_enabled = self.backups_enabled;
        config.backups_to_keep = self.backups_to_keep;
        config.theme = self.theme;
        config.text_size = self.text_size;
        config.accent = self.accent;
        config.sidebar_width = self.sidebar_width;
    }
}

/// Сохраняет настройки и возвращает их в том виде, в каком они применились.
#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, settings: Settings) -> Settings {
    let mut session = state.lock();
    let mut config = session.config.clone();
    settings.apply(&mut config);
    session.config = config.normalized();
    session.save_config();
    Settings::from(&session.config)
}
