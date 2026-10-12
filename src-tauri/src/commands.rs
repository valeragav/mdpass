//! Команды, которые вызывает интерфейс.

use std::path::{Path, PathBuf};
use std::time::Instant;

use age::secrecy::ExposeSecret;
use mdpass_core::backup::BackupKind;
use mdpass_core::vault::{Passphrase, Vault, VaultError, VaultOptions};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::session::{AppState, RETRY_DELAY};
use crate::settings::Settings;

const DEFAULT_FILE_NAME: &str = "vault.md.age";

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CommandError {
    WrongPassphrase {
        retry_in_ms: u64,
    },
    TooSoon {
        retry_in_ms: u64,
    },
    Damaged {
        detail: String,
    },
    NotFound,
    AlreadyExists,
    EmptyPassphrase,
    /// Файл зашифрован, а пароль не введён.
    PassphraseRequired,
    ChangedOnDisk,
    /// Хранилище заблокировано: команде нужны открытые записи.
    Locked,
    EntryNotFound,
    Clipboard {
        message: String,
    },
    /// Резервная копия зашифрована другим мастер-паролем: сделана до его смены.
    OtherPassphrase,
    /// Данные записи не прошли проверку. В `message` — текст для пользователя.
    Invalid {
        message: String,
    },
    Io {
        message: String,
    },
}

impl From<VaultError> for CommandError {
    fn from(err: VaultError) -> Self {
        match err {
            VaultError::EmptyPassphrase => Self::EmptyPassphrase,
            VaultError::AlreadyExists => Self::AlreadyExists,
            VaultError::WrongPassphrase => Self::WrongPassphrase { retry_in_ms: millis(RETRY_DELAY) },
            VaultError::Damaged(detail) => Self::Damaged { detail },
            VaultError::ChangedOnDisk => Self::ChangedOnDisk,
            VaultError::PassphraseRequired => Self::PassphraseRequired,
            VaultError::Io(err) if err.kind() == std::io::ErrorKind::NotFound => Self::NotFound,
            VaultError::Io(err) => Self::Io { message: err.to_string() },
        }
    }
}

pub(crate) fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Файл хранилища, как его показывает интерфейс.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultFile {
    path: String,
    /// Путь с `~` вместо домашней папки.
    display: String,
    name: String,
    exists: bool,
    /// Зашифрован ли файл. `None`, если файла нет или его не прочитать.
    encrypted: Option<bool>,
}

/// Как защищено хранилище.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protection {
    /// Надёжное шифрование: scrypt 2^18.
    Strong,
    /// Слабое шифрование: scrypt 2^12, открывается мгновенно.
    Weak,
    /// Без шифрования: обычный Markdown-файл.
    None,
}

impl Protection {
    pub fn of(vault: &Vault) -> Self {
        match vault.options() {
            Some(options) if options.is_strong() => Self::Strong,
            Some(_) => Self::Weak,
            None => Self::None,
        }
    }

    fn options(self) -> Option<VaultOptions> {
        match self {
            Self::Strong => Some(VaultOptions::STRONG),
            Self::Weak => Some(VaultOptions::WEAK),
            Self::None => None,
        }
    }
}

fn describe<R: Runtime>(app: &AppHandle<R>, path: &Path) -> VaultFile {
    let display = match app.path().home_dir().ok().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf))
    {
        Some(rest) => format!("~/{}", rest.to_string_lossy().replace('\\', "/")),
        None => path.to_string_lossy().into_owned(),
    };
    VaultFile {
        path: path.to_string_lossy().into_owned(),
        display,
        name: path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
        exists: path.is_file(),
        encrypted: path.is_file().then(|| mdpass_core::vault::file_is_encrypted(path).ok()).flatten(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    unlocked: bool,
    /// Последнее открытое хранилище.
    last_vault: Option<VaultFile>,
    /// Куда предложить положить новое хранилище.
    default_vault: Option<VaultFile>,
    /// Защита открытого хранилища. `None`, пока хранилище заблокировано.
    protection: Option<Protection>,
    settings: Settings,
}

#[tauri::command]
pub fn status<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Status {
    let session = state.lock();
    let default_vault = app.path().document_dir().ok().map(|dir| describe(&app, &dir.join(DEFAULT_FILE_NAME)));
    Status {
        unlocked: session.vault.is_some(),
        last_vault: session.config.last_vault.as_deref().map(|path| describe(&app, path)),
        default_vault,
        protection: session.vault.as_ref().map(Protection::of),
        settings: Settings::from(&session.config),
    }
}

#[tauri::command]
pub fn describe_path<R: Runtime>(app: AppHandle<R>, path: String) -> VaultFile {
    describe(&app, Path::new(&path))
}

#[tauri::command]
pub async fn create_vault(
    state: State<'_, AppState>,
    path: String,
    passphrase: Option<String>,
    protection: Protection,
) -> Result<Protection, CommandError> {
    let state = state.inner().clone();
    let passphrase = passphrase.map(Passphrase::from);
    run_blocking(move || {
        let path = PathBuf::from(path);
        let vault = match (protection.options(), passphrase) {
            (None, _) => Vault::create_plain(&path)?,
            (Some(options), Some(passphrase)) => {
                check_length(&passphrase)?;
                Vault::create(&path, passphrase, options)?
            }
            (Some(_), None) => return Err(CommandError::EmptyPassphrase),
        };
        let protection = Protection::of(&vault);
        let mut session = state.lock();
        session.vault = Some(vault);
        session.touch(Instant::now());
        session.config.last_vault = Some(path);
        session.save_config();
        Ok(protection)
    })
    .await
}

fn check_length(passphrase: &Passphrase) -> Result<(), CommandError> {
    if passphrase.expose_secret().chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(CommandError::Invalid {
            message: format!("Мастер-пароль должен быть не короче {MIN_PASSPHRASE_CHARS} символов"),
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn unlock(
    state: State<'_, AppState>,
    path: String,
    passphrase: Option<String>,
) -> Result<Protection, CommandError> {
    let state = state.inner().clone();
    let passphrase = passphrase.filter(|p| !p.is_empty()).map(Passphrase::from);
    run_blocking(move || {
        // Сессия занята всё время проверки: параллельные попытки не обойдут паузу.
        let mut session = state.lock();
        if let Some(wait) = session.limiter.wait(Instant::now()) {
            return Err(CommandError::TooSoon { retry_in_ms: millis(wait) });
        }
        let path = PathBuf::from(path);
        let opened = match passphrase {
            Some(passphrase) => Vault::open(&path, passphrase),
            None => Vault::open_plain(&path),
        };
        match opened {
            Ok(vault) => {
                let protection = Protection::of(&vault);
                session.limiter.succeeded();
                session.vault = Some(vault);
                session.touch(Instant::now());
                session.config.last_vault = Some(path);
                session.save_config();
                Ok(protection)
            }
            Err(err) => {
                if matches!(err, VaultError::WrongPassphrase) {
                    session.limiter.failed(Instant::now());
                }
                Err(err.into())
            }
        }
    })
    .await
}

#[tauri::command]
pub fn lock(state: State<'_, AppState>) {
    state.lock().lock_vault();
}

/// Интерфейс сообщает, что пользователь что-то делает: откладывает автоблокировку.
#[tauri::command]
pub fn touch(state: State<'_, AppState>) {
    state.lock().touch(Instant::now());
}

/// Минимальная длина мастер-пароля, как на экране первого запуска.
const MIN_PASSPHRASE_CHARS: usize = 8;

/// Меняет защиту открытого хранилища: пароль, силу шифрования или снимает шифрование.
/// У зашифрованного хранилища сначала проверяется текущий пароль.
#[tauri::command]
pub async fn change_protection(
    state: State<'_, AppState>,
    current: Option<String>,
    new: Option<String>,
    protection: Protection,
) -> Result<Protection, CommandError> {
    let state = state.inner().clone();
    let current = current.map(Passphrase::from);
    let new = new.map(Passphrase::from);
    run_blocking(move || {
        let mut session = state.lock();
        if let Some(wait) = session.limiter.wait(Instant::now()) {
            return Err(CommandError::TooSoon { retry_in_ms: millis(wait) });
        }
        let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
        if vault.is_encrypted() && !current.as_ref().is_some_and(|c| vault.passphrase_matches(c)) {
            session.limiter.failed(Instant::now());
            return Err(CommandError::WrongPassphrase { retry_in_ms: millis(RETRY_DELAY) });
        }
        let key = match (protection.options(), new) {
            (None, _) => None,
            (Some(options), Some(new)) => {
                check_length(&new)?;
                Some((new, options))
            }
            (Some(_), None) => return Err(CommandError::EmptyPassphrase),
        };
        session.limiter.succeeded();
        let policy = session.backup_policy();
        let vault = session.vault.as_mut().ok_or(CommandError::Locked)?;
        match &policy {
            Some(policy) => vault.set_protection(key, policy.hook(BackupKind::Passphrase))?,
            None => vault.set_protection(key, |_| Ok(()))?,
        }
        Ok(Protection::of(vault))
    })
    .await
}

/// Шифрование занимает около секунды: уводим его из потока, который обслуживает окно.
pub(crate) async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|err| Err(CommandError::Io { message: err.to_string() }))
}
