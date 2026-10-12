//! Настройки приложения. Лежат отдельно от хранилища и секретов не содержат.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::fs_atomic::write_atomic;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Хранилище, которое открывали последним.
    pub last_vault: Option<PathBuf>,
}

impl AppConfig {
    /// Читает настройки. Если файла нет или он испорчен, возвращает настройки по умолчанию:
    /// из-за настроек приложение не должно переставать открываться.
    pub fn load(path: &Path) -> Self {
        fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        write_atomic(path, &json)
    }
}
