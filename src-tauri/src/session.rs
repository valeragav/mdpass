//! Состояние приложения между командами: открытое хранилище, пауза после неверного
//! пароля и настройки.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

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
    /// Последнее действие пользователя. От него считается автоблокировка. Часы
    /// настенные: монотонные часы на macOS и Linux стоят, пока компьютер спит, и
    /// хранилище, открытое перед сном, осталось бы открытым утром.
    last_activity: SystemTime,
    /// Открытый файл, который импортировали последним: только его можно удалить.
    pub last_import: Option<PathBuf>,
    /// Копия, которую смотрят на экране резервных копий.
    pub open_backup: Option<OpenBackup>,
    /// Последняя запись слила изменения с другого устройства: интерфейс покажет
    /// это, когда перечитает записи.
    pub merge_notice: Option<MergeNotice>,
    /// Что знаем об облаке для открытого хранилища: путь хранилища и состояние.
    pub sync_state: Option<(PathBuf, mdpass_core::cloud::SyncState)>,
    pub cloud_status: crate::cloud::SyncStatus,
    /// Номера записей у интерфейса верные. Синхронизация, которая заменила документ
    /// версией из облака, их сбрасывает: пока интерфейс не перечитает записи, команды
    /// с номерами отказывают, иначе правка попала бы не в ту запись.
    pub ids_fresh: bool,
    /// Интерфейс просит пока не менять записи из облака: открыт редактор записи.
    pub cloud_hold: bool,
}

/// Что сообщить после слияния. Только названия записей: значений здесь нет.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeNotice {
    /// Записи, которые изменили с двух сторон: в файле остались обе версии.
    pub both_kept: Vec<String>,
}

impl Session {
    pub fn new(config_path: PathBuf, backups_root: PathBuf) -> Self {
        Self {
            vault: None,
            limiter: AttemptLimiter::new(RETRY_DELAY),
            config: AppConfig::load(&config_path),
            config_path,
            backups_root,
            last_activity: SystemTime::now(),
            last_import: None,
            open_backup: None,
            merge_notice: None,
            sync_state: None,
            cloud_status: crate::cloud::SyncStatus::Idle,
            ids_fresh: true,
            cloud_hold: false,
        }
    }

    pub fn touch(&mut self, now: SystemTime) {
        self.last_activity = now;
    }

    /// Пора ли заблокировать хранилище из-за бездействия.
    pub fn idle_expired(&self, now: SystemTime) -> bool {
        let minutes = self.config.auto_lock_minutes;
        self.vault.is_some()
            && minutes > 0
            && now.duration_since(self.last_activity).unwrap_or_default()
                >= Duration::from_secs(u64::from(minutes) * 60)
    }

    /// Открывает другое хранилище. Всё, что относилось к прежнему, забывается.
    pub fn replace_vault(&mut self, vault: Vault) {
        self.open_backup = None;
        self.last_import = None;
        self.merge_notice = None;
        self.sync_state = None;
        self.cloud_status = crate::cloud::SyncStatus::Idle;
        self.vault = Some(vault);
    }

    /// Закрывает хранилище: расшифрованные данные стираются, буфер обмена очищается,
    /// если в нём наше значение.
    pub fn lock_vault(&mut self) {
        self.vault = None;
        self.open_backup = None;
        self.merge_notice = None;
        self.sync_state = None;
        self.cloud_status = crate::cloud::SyncStatus::Idle;
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
        let start = SystemTime::now();
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
        let start = SystemTime::now();
        session.touch(start);
        session.config.auto_lock_minutes = 0;
        assert!(!session.idle_expired(start + Duration::from_secs(86_400)));

        session.config.auto_lock_minutes = 1;
        session.vault = None;
        assert!(!session.idle_expired(start + Duration::from_secs(86_400)));
    }

    #[test]
    fn opening_another_vault_forgets_the_previous_one() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut session = unlocked_session(&dir);
        session.last_import = Some(dir.path().join("old.md"));
        let other = Vault::create(
            &dir.path().join("other.md.age"),
            Passphrase::from("p".to_string()),
            VaultOptions { work_factor: 4 },
        )
        .unwrap();
        session.replace_vault(other);
        assert!(session.last_import.is_none());
        assert!(session.open_backup.is_none());
        assert!(session.vault.as_ref().unwrap().path().ends_with("other.md.age"));
    }
}
