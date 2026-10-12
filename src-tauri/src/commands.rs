//! Команды, которые вызывает интерфейс.

use std::path::{Path, PathBuf};
use std::time::Instant;

use mdpass_core::vault::{Passphrase, Vault, VaultError, VaultOptions};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::session::{AppState, RETRY_DELAY};

const DEFAULT_FILE_NAME: &str = "vault.md.age";

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CommandError {
    WrongPassphrase { retry_in_ms: u64 },
    TooSoon { retry_in_ms: u64 },
    Damaged { detail: String },
    NotFound,
    AlreadyExists,
    EmptyPassphrase,
    ChangedOnDisk,
    Io { message: String },
}

impl From<VaultError> for CommandError {
    fn from(err: VaultError) -> Self {
        match err {
            VaultError::EmptyPassphrase => Self::EmptyPassphrase,
            VaultError::AlreadyExists => Self::AlreadyExists,
            VaultError::WrongPassphrase => Self::WrongPassphrase { retry_in_ms: millis(RETRY_DELAY) },
            VaultError::Damaged(detail) => Self::Damaged { detail },
            VaultError::ChangedOnDisk => Self::ChangedOnDisk,
            VaultError::Io(err) if err.kind() == std::io::ErrorKind::NotFound => Self::NotFound,
            VaultError::Io(err) => Self::Io { message: err.to_string() },
        }
    }
}

fn millis(duration: std::time::Duration) -> u64 {
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
}

#[tauri::command]
pub fn status<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Status {
    let session = state.lock();
    let default_vault = app.path().document_dir().ok().map(|dir| describe(&app, &dir.join(DEFAULT_FILE_NAME)));
    Status {
        unlocked: session.vault.is_some(),
        last_vault: session.config.last_vault.as_deref().map(|path| describe(&app, path)),
        default_vault,
    }
}

#[tauri::command]
pub fn describe_path<R: Runtime>(app: AppHandle<R>, path: String) -> VaultFile {
    describe(&app, Path::new(&path))
}

#[tauri::command]
pub async fn create_vault(state: State<'_, AppState>, path: String, passphrase: String) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let passphrase = Passphrase::from(passphrase);
    run_blocking(move || {
        let path = PathBuf::from(path);
        let vault = Vault::create(&path, passphrase, VaultOptions::default())?;
        let mut session = state.lock();
        session.vault = Some(vault);
        session.config.last_vault = Some(path);
        session.save_config();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn unlock(state: State<'_, AppState>, path: String, passphrase: String) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let passphrase = Passphrase::from(passphrase);
    run_blocking(move || {
        // Сессия занята всё время проверки: параллельные попытки не обойдут паузу.
        let mut session = state.lock();
        if let Some(wait) = session.limiter.wait(Instant::now()) {
            return Err(CommandError::TooSoon { retry_in_ms: millis(wait) });
        }
        let path = PathBuf::from(path);
        match Vault::open(&path, passphrase) {
            Ok(vault) => {
                session.limiter.succeeded();
                session.vault = Some(vault);
                session.config.last_vault = Some(path);
                session.save_config();
                Ok(())
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
    state.lock().vault = None;
}

/// Шифрование занимает около секунды: уводим его из потока, который обслуживает окно.
async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|err| Err(CommandError::Io { message: err.to_string() }))
}
