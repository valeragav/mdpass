//! Состояние приложения между командами: открытое хранилище, пауза после неверного
//! пароля и настройки.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use mdpass_core::config::AppConfig;
use mdpass_core::vault::{AttemptLimiter, Vault};

/// Пауза после неверного мастер-пароля.
pub const RETRY_DELAY: Duration = Duration::from_secs(3);

pub struct Session {
    /// Открытое хранилище. `None`, пока хранилище заблокировано.
    pub vault: Option<Vault>,
    pub limiter: AttemptLimiter,
    pub config: AppConfig,
    config_path: PathBuf,
}

impl Session {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            vault: None,
            limiter: AttemptLimiter::new(RETRY_DELAY),
            config: AppConfig::load(&config_path),
            config_path,
        }
    }

    pub fn save_config(&self) {
        // Без сохранённых настроек приложение работает, просто не вспомнит файл.
        if let Err(err) = self.config.save(&self.config_path) {
            eprintln!("не удалось сохранить настройки: {err}");
        }
    }
}

/// Сессия, общая для всех команд. `Arc`, чтобы долгие операции шифрования
/// можно было увести в отдельный поток.
#[derive(Clone)]
pub struct AppState(Arc<Mutex<Session>>);

impl AppState {
    pub fn new(session: Session) -> Self {
        Self(Arc::new(Mutex::new(session)))
    }

    pub fn lock(&self) -> MutexGuard<'_, Session> {
        // Паника в другой команде не должна навсегда закрыть доступ к сессии.
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
