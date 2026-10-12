//! Команды экрана резервных копий.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mdpass_core::backup::{BackupKind, BackupStore, restore_damaged};
use mdpass_core::diff::{Change, diff};
use mdpass_core::vault::{Passphrase, VaultError};
use serde::Serialize;
use tauri::State;

use crate::commands::{CommandError, run_blocking};
use crate::session::{AppState, RETRY_DELAY, Session};

/// Как делать копии перед записью: куда и сколько хранить.
pub struct BackupPolicy {
    store: BackupStore,
    keep: usize,
}

impl BackupPolicy {
    /// Сохраняет байты файла перед записью и удаляет лишние старые копии.
    pub fn hook(&self, kind: BackupKind) -> impl FnOnce(&[u8]) -> io::Result<()> + '_ {
        move |bytes| {
            self.store.save(bytes, kind, SystemTime::now())?;
            self.store.prune(self.keep)
        }
    }
}

impl Session {
    /// Правило копий для открытого хранилища. `None`, если копии выключены в настройках.
    pub fn backup_policy(&self) -> Option<BackupPolicy> {
        let vault = self.vault.as_ref()?;
        self.config.backups_enabled.then(|| self.policy_for(vault.path()))
    }

    /// Копия перед восстановлением делается всегда: так восстановление можно отменить.
    fn policy_for(&self, path: &Path) -> BackupPolicy {
        BackupPolicy {
            store: BackupStore::for_vault(path),
            keep: usize::try_from(self.config.backups_to_keep).unwrap_or(usize::MAX).max(1),
        }
    }
}

/// Без копии: для записи, когда копии выключены.
pub fn no_backup(_: &[u8]) -> io::Result<()> {
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupView {
    id: String,
    created_ms: u64,
    kind: BackupKind,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChangeView {
    Added { title: String },
    Removed { title: String },
    Changed { title: String, keys: Vec<String> },
}

impl From<Change> for ChangeView {
    fn from(change: Change) -> Self {
        match change {
            Change::Added { title } => Self::Added { title },
            Change::Removed { title } => Self::Removed { title },
            Change::Changed { title, keys } => Self::Changed { title, keys },
        }
    }
}

fn store_for(session: &Session, path: Option<String>) -> Result<BackupStore, CommandError> {
    match path {
        Some(path) => Ok(BackupStore::for_vault(Path::new(&path))),
        None => session.vault.as_ref().map(|v| BackupStore::for_vault(v.path())).ok_or(CommandError::Locked),
    }
}

fn io_error(err: io::Error) -> CommandError {
    CommandError::Io { message: err.to_string() }
}

/// Копии открытого хранилища или файла по пути, если хранилище не открывается.
#[tauri::command]
pub fn list_backups(state: State<'_, AppState>, path: Option<String>) -> Result<Vec<BackupView>, CommandError> {
    let store = store_for(&state.lock(), path)?;
    let list = store.list().map_err(io_error)?;
    Ok(list
        .into_iter()
        .map(|info| BackupView {
            created_ms: info
                .created
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
            id: info.id,
            kind: info.kind,
        })
        .collect())
}

/// Что изменится, если восстановить копию.
#[tauri::command]
pub async fn preview_backup(state: State<'_, AppState>, id: String) -> Result<Vec<ChangeView>, CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let session = state.lock();
        let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
        let bytes = BackupStore::for_vault(vault.path()).read(&id).map_err(io_error)?;
        let backup = vault.decrypt_backup(&bytes).map_err(other_passphrase)?;
        Ok(diff(vault.document(), &backup).into_iter().map(ChangeView::from).collect())
    })
    .await
}

/// Копия, сделанная до смены пароля, текущим паролем не открывается.
fn other_passphrase(err: VaultError) -> CommandError {
    match err {
        VaultError::WrongPassphrase => CommandError::OtherPassphrase,
        other => other.into(),
    }
}

#[tauri::command]
pub async fn restore_backup(state: State<'_, AppState>, id: String) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
        let policy = session.policy_for(vault.path());
        let bytes = policy.store.read(&id).map_err(io_error)?;
        let document = vault.decrypt_backup(&bytes).map_err(other_passphrase)?;
        let vault = session.vault.as_mut().ok_or(CommandError::Locked)?;
        vault.restore(bytes, document, policy.hook(BackupKind::Restore))?;
        Ok(())
    })
    .await
}

/// Восстанавливает повреждённый файл из копии и открывает хранилище.
#[tauri::command]
pub async fn restore_damaged_vault(
    state: State<'_, AppState>,
    path: String,
    id: String,
    passphrase: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let passphrase = Passphrase::from(passphrase);
    run_blocking(move || {
        let mut session = state.lock();
        if let Some(wait) = session.limiter.wait(Instant::now()) {
            return Err(CommandError::TooSoon { retry_in_ms: crate::commands::millis(wait) });
        }
        let path = PathBuf::from(path);
        let store = BackupStore::for_vault(&path);
        match restore_damaged(&path, &store, &id, passphrase, SystemTime::now()) {
            Ok(vault) => {
                session.limiter.succeeded();
                session.vault = Some(vault);
                session.touch(Instant::now());
                session.config.last_vault = Some(path);
                session.save_config();
                Ok(())
            }
            Err(VaultError::WrongPassphrase) => {
                session.limiter.failed(Instant::now());
                Err(CommandError::WrongPassphrase { retry_in_ms: crate::commands::millis(RETRY_DELAY) })
            }
            Err(err) => Err(err.into()),
        }
    })
    .await
}

/// Сохраняет копию отдельным файлом. Файл остаётся зашифрованным.
#[tauri::command]
pub fn export_backup(
    state: State<'_, AppState>,
    path: Option<String>,
    id: String,
    to: String,
) -> Result<(), CommandError> {
    let store = store_for(&state.lock(), path)?;
    store.export(&id, Path::new(&to)).map_err(io_error)
}
