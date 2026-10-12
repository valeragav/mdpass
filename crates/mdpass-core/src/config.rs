//! Настройки приложения. Лежат отдельно от хранилища и секретов не содержат.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::fs_atomic::write_atomic;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Хранилище, которое открывали последним.
    pub last_vault: Option<PathBuf>,
    /// Через сколько минут бездействия хранилище блокируется. 0 — никогда.
    pub auto_lock_minutes: u32,
    /// Через сколько секунд после копирования очищается буфер обмена. 0 — не очищать.
    pub clipboard_clear_seconds: u32,
    /// Блокировать хранилище, когда окно сворачивают.
    pub lock_on_minimize: bool,
    /// Делать зашифрованную копию файла перед каждой записью.
    pub backups_enabled: bool,
    /// Сколько резервных копий хранить. Старые удаляются.
    pub backups_to_keep: u32,
    pub theme: Theme,
    pub text_size: TextSize,
    /// Цвет акцента интерфейса, `#RRGGBB`.
    pub accent: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextSize {
    S,
    M,
    L,
}

/// Значения по умолчанию взяты с экрана настроек в дизайне.
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_vault: None,
            auto_lock_minutes: 5,
            clipboard_clear_seconds: 30,
            lock_on_minimize: true,
            backups_enabled: true,
            backups_to_keep: 30,
            theme: Theme::Dark,
            text_size: TextSize::M,
            accent: "#E5A93D".into(),
        }
    }
}

impl AppConfig {
    /// Читает настройки. Если файла нет или он испорчен, возвращает настройки по умолчанию:
    /// из-за настроек приложение не должно переставать открываться.
    pub fn load(path: &Path) -> Self {
        let config: Self =
            fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        config.normalized()
    }

    /// Приводит значения из файла к допустимым: файл могли поправить вручную.
    pub fn normalized(mut self) -> Self {
        let defaults = Self::default();
        self.backups_to_keep = self.backups_to_keep.clamp(1, 1000);
        self.auto_lock_minutes = self.auto_lock_minutes.min(24 * 60);
        self.clipboard_clear_seconds = self.clipboard_clear_seconds.min(60 * 60);
        let valid_accent = self.accent.len() == 7
            && self.accent.starts_with('#')
            && self.accent[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !valid_accent {
            self.accent = defaults.accent;
        }
        self
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        write_atomic(path, &json)
    }
}
