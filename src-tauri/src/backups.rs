//! Команды экрана резервных копий.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mdpass_core::backup::{BackupKind, BackupStore, restore_damaged};
use mdpass_core::diff::{Change, diff, find_entry, restore_entry};
use mdpass_core::format::{Document, Entry, EntryId, Favorite};
use mdpass_core::vault::{Passphrase, VaultError};
use serde::Serialize;
use tauri::State;
use zeroize::Zeroizing;

use crate::commands::{CommandError, run_blocking};
use crate::entries::{clipboard_text, copy_to_clipboard, edit};
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
    /// Папка копий хранилища в папке приложения. Копии, которые раньше лежали
    /// рядом с хранилищем, переезжают туда при первом обращении.
    pub fn backup_store(&self, vault_path: &Path) -> BackupStore {
        let store = BackupStore::in_root(&self.backups_root, vault_path);
        if let Err(err) = BackupStore::for_vault(vault_path).move_to(&store) {
            log::warn!("не удалось перенести старые резервные копии: {err}");
        }
        store
    }

    /// Правило копий для открытого хранилища. `None`, если копии выключены в настройках.
    pub fn backup_policy(&self) -> Option<BackupPolicy> {
        let vault = self.vault.as_ref()?;
        self.config.backups_enabled.then(|| self.policy_for(vault.path()))
    }

    /// Копия перед восстановлением делается всегда: так восстановление можно отменить.
    fn policy_for(&self, path: &Path) -> BackupPolicy {
        BackupPolicy {
            store: self.backup_store(path),
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
    Added { path: Vec<String>, title: String },
    Removed { path: Vec<String>, title: String },
    Changed { path: Vec<String>, title: String, fields: Vec<FieldChangeView> },
}

/// Поле, которое вернётся к прежнему значению. Значение секретного поля сюда
/// не попадает: его показывают и копируют отдельными командами.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChangeView {
    key: String,
    secret: bool,
    /// Значение в копии. `None` у секретного поля и у поля, которого в копии нет.
    before: Option<String>,
    /// В копии такого поля нет: при восстановлении оно пропадёт.
    missing: bool,
}

fn change_view(change: Change, backup: &Document) -> ChangeView {
    match change {
        Change::Added { path, title } => ChangeView::Added { path, title },
        Change::Removed { path, title } => ChangeView::Removed { path, title },
        Change::Changed { path, title, keys } => {
            let old = find_entry(backup, &path, &title);
            let fields = keys.into_iter().map(|key| field_change(old.as_ref(), key)).collect();
            ChangeView::Changed { path, title, fields }
        }
    }
}

fn field_change(old: Option<&Entry>, key: String) -> FieldChangeView {
    let plain = |before: Option<String>| (false, before.clone(), before.is_none());
    // `tags` и `favorite` — не поля, а строки записи с особым смыслом.
    let (secret, before, missing) = match (old, key.as_str()) {
        (None, _) => (false, None, true),
        (Some(old), "tags") => plain(Some(old.tags.iter().map(|t| format!("[{t}]")).collect::<Vec<_>>().join(", "))),
        (Some(old), "favorite") => plain(match old.favorite {
            Favorite::No => None,
            Favorite::Yes => Some("true".into()),
            Favorite::At(n) => Some(n.to_string()),
        }),
        (Some(old), key) => match old.fields.iter().find(|f| f.key == key) {
            Some(field) => (field.secret, (!field.secret).then(|| field.value.clone()), false),
            None => (false, None, true),
        },
    };
    FieldChangeView { key, secret, before, missing }
}

/// Копия, открытая на экране копий. Расшифровка занимает около секунды, поэтому
/// копия остаётся в памяти, пока её смотрят, и стирается при блокировке.
pub struct OpenBackup {
    vault: PathBuf,
    id: String,
    document: Document,
}

fn open_backup<'a>(session: &'a mut Session, id: &str) -> Result<&'a Document, CommandError> {
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    let cached = session.open_backup.as_ref().is_some_and(|b| b.vault == vault.path() && b.id == id);
    if !cached {
        let bytes = Zeroizing::new(session.backup_store(vault.path()).read(id).map_err(io_error)?);
        let document = vault.decrypt_backup(&bytes).map_err(other_passphrase)?;
        session.open_backup = Some(OpenBackup { vault: vault.path().to_path_buf(), id: id.to_string(), document });
    }
    Ok(&session.open_backup.as_ref().expect("копия только что открыта").document)
}

/// Запись из копии по разделу и названию.
fn backup_entry(session: &mut Session, id: &str, path: &[String], title: &str) -> Result<Entry, CommandError> {
    find_entry(open_backup(session, id)?, path, title).ok_or(CommandError::EntryNotFound)
}

fn backup_field(entry: &Entry, key: &str) -> Result<Zeroizing<String>, CommandError> {
    let field = entry.fields.iter().find(|f| f.key == key).ok_or(CommandError::EntryNotFound)?;
    Ok(Zeroizing::new(field.value.clone()))
}

fn store_for(session: &Session, path: Option<String>) -> Result<BackupStore, CommandError> {
    match path {
        Some(path) => Ok(session.backup_store(Path::new(&path))),
        None => session.vault.as_ref().map(|v| session.backup_store(v.path())).ok_or(CommandError::Locked),
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
        let mut session = state.lock();
        let backup = open_backup(&mut session, &id)?.clone();
        let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
        Ok(diff(vault.document(), &backup).into_iter().map(|change| change_view(change, &backup)).collect())
    })
    .await
}

/// Прежнее значение поля из копии.
#[tauri::command]
pub async fn reveal_backup_field(
    state: State<'_, AppState>,
    id: String,
    path: Vec<String>,
    title: String,
    key: String,
) -> Result<String, CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        let entry = backup_entry(&mut session, &id, &path, &title)?;
        Ok(backup_field(&entry, &key)?.as_str().to_owned())
    })
    .await
}

/// Копирует прежнее значение поля из копии, как обычное поле записи.
#[tauri::command]
pub async fn copy_backup_field(
    state: State<'_, AppState>,
    id: String,
    path: Vec<String>,
    title: String,
    key: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        let entry = backup_entry(&mut session, &id, &path, &title)?;
        let value = clipboard_text(&key, backup_field(&entry, &key)?);
        copy_to_clipboard(&session, value)
    })
    .await
}

/// Возвращает из копии одну запись. Остальные записи не меняются.
#[tauri::command]
pub async fn restore_backup_entry(
    state: State<'_, AppState>,
    id: String,
    path: Vec<String>,
    title: String,
) -> Result<u64, CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        let entry = backup_entry(&mut session, &id, &path, &title)?;
        edit(&mut session, BackupKind::Restore, |doc| restore_entry(doc, &entry)).map(EntryId::get)
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
        let bytes = Zeroizing::new(policy.store.read(&id).map_err(io_error)?);
        let document = vault.decrypt_backup(&bytes).map_err(other_passphrase)?;
        let vault = session.vault.as_mut().ok_or(CommandError::Locked)?;
        vault.restore(document, policy.hook(BackupKind::Restore))?;
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
    passphrase: Option<String>,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let passphrase = passphrase.filter(|p| !p.is_empty()).map(Passphrase::from);
    run_blocking(move || {
        let mut session = state.lock();
        if let Some(wait) = session.limiter.wait(Instant::now()) {
            return Err(CommandError::TooSoon { retry_in_ms: crate::commands::millis(wait) });
        }
        let path = PathBuf::from(path);
        let store = session.backup_store(&path);
        match restore_damaged(&path, &store, &id, passphrase, SystemTime::now()) {
            Ok(vault) => {
                session.limiter.succeeded();
                session.vault = Some(vault);
                session.touch(Instant::now());
                session.config.remember(path);
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

/// Где лежат копии открытого хранилища: для строки в настройках.
#[tauri::command]
pub fn backups_location(state: State<'_, AppState>) -> Result<String, CommandError> {
    let session = state.lock();
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    Ok(session.backup_store(vault.path()).dir().to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use mdpass_core::format::{EntryDraft, Field};

    use super::*;

    #[test]
    fn changed_fields_show_old_plain_values_but_not_secrets() {
        let mut current = Document::default();
        let mut backup = Document::default();
        let draft = |login: &str, password: &str, extra: Option<Field>| EntryDraft {
            path: vec!["work".into()],
            title: "alpha".into(),
            fields: [Field::new("login", login), Field::new("password", password)].into_iter().chain(extra).collect(),
            tags: vec![],
            favorite: Favorite::No,
        };
        current.add(draft("new-login", "new-secret", Some(Field::new("url", "u")))).unwrap();
        backup.add(draft("old-login", "old-secret", None)).unwrap();

        let views: Vec<ChangeView> = diff(&current, &backup).into_iter().map(|c| change_view(c, &backup)).collect();
        let ChangeView::Changed { fields, .. } = &views[0] else { panic!("ожидалось изменение") };
        let by_key = |key: &str| fields.iter().find(|f| f.key == key).unwrap();
        assert_eq!(by_key("login").before.as_deref(), Some("old-login"));
        assert!(by_key("password").secret);
        assert_eq!(by_key("password").before, None);
        assert!(by_key("url").missing);
        assert!(!format!("{views:?}").contains("old-secret"));
    }
}
