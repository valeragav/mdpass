//! Состояние приложения между командами: открытое хранилище, пауза после неверного
//! пароля и настройки.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use mdpass_core::config::AppConfig;
use mdpass_core::vault::{AttemptLimiter, Vault};

use crate::backups::OpenBackup;

/// Пауза после неверного мастер-пароля.
pub const RETRY_DELAY: Duration = Duration::from_secs(3);

pub struct Session {
    /// Открытое хранилище. `None`, пока хранилище заблокировано.
    pub vault: Option<Vault>,
    pub limiter: AttemptLimiter,
    pub config: AppConfig,
    config_path: PathBuf,
    /// Папка резервных копий в папке данных приложения.
    pub backups_root: PathBuf,
    /// Последнее действие пользователя. От него считается автоблокировка.
    last_activity: Instant,
    /// Открытый файл, который импортировали последним: только его можно удалить.
    pub last_import: Option<PathBuf>,
    /// Копия, которую смотрят на экране резервных копий.
    pub open_backup: Option<OpenBackup>,
}

impl Session {
    pub fn new(config_path: PathBuf, backups_root: PathBuf) -> Self {
        Self {
            vault: None,
            limiter: AttemptLimiter::new(RETRY_DELAY),
            config: AppConfig::load(&config_path),
            config_path,
            backups_root,
            last_activity: Instant::now(),
            last_import: None,
            open_backup: None,
        }
    }

    pub fn touch(&mut self, now: Instant) {
        self.last_activity = now;
    }

    /// Пора ли заблокировать хранилище из-за бездействия.
    pub fn idle_expired(&self, now: Instant) -> bool {
        let minutes = self.config.auto_lock_minutes;
        self.vault.is_some()
            && minutes > 0
            && now.saturating_duration_since(self.last_activity) >= Duration::from_secs(u64::from(minutes) * 60)
    }

    /// Закрывает хранилище: расшифрованные данные стираются, буфер обмена очищается,
    /// если в нём наше значение.
    pub fn lock_vault(&mut self) {
        self.vault = None;
        self.open_backup = None;
        #[cfg(desktop)]
        crate::clipboard::clear_ours();
    }

    /// Через сколько очищать буфер после копирования. `None` — не очищать.
    pub fn clipboard_clear_after(&self) -> Option<Duration> {
        let seconds = self.config.clipboard_clear_seconds;
        (seconds > 0).then(|| Duration::from_secs(u64::from(seconds)))
    }

    pub fn save_config(&self) {
        // Без сохранённых настроек приложение работает, просто не вспомнит файл.
        if let Err(err) = self.config.save(&self.config_path) {
            log::warn!("не удалось сохранить настройки: {err}");
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

#[cfg(test)]
mod tests {
    use mdpass_core::vault::{Passphrase, VaultOptions};

    use super::*;

    fn unlocked_session(dir: &tempfile::TempDir) -> Session {
        let mut session = Session::new(dir.path().join("config.json"), dir.path().join("backups"));
        let vault = Vault::create(
            &dir.path().join("v.md.age"),
            Passphrase::from("p".to_string()),
            VaultOptions { work_factor: 4 },
        )
        .unwrap();
        session.vault = Some(vault);
        session
    }

    #[test]
    fn idle_lock_fires_after_the_configured_minutes() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut session = unlocked_session(&dir);
        let start = Instant::now();
        session.touch(start);
        session.config.auto_lock_minutes = 5;
        assert!(!session.idle_expired(start + Duration::from_secs(299)));
        assert!(session.idle_expired(start + Duration::from_secs(300)));

        session.touch(start + Duration::from_secs(200));
        assert!(!session.idle_expired(start + Duration::from_secs(300)));
    }

    #[test]
    fn idle_lock_is_off_when_disabled_or_already_locked() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut session = unlocked_session(&dir);
        let start = Instant::now();
        session.touch(start);
        session.config.auto_lock_minutes = 0;
        assert!(!session.idle_expired(start + Duration::from_secs(86_400)));

        session.config.auto_lock_minutes = 1;
        session.vault = None;
        assert!(!session.idle_expired(start + Duration::from_secs(86_400)));
    }
}
