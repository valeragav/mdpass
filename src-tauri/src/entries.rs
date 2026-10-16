//! Команды для записей открытого хранилища.
//!
//! Значения секретных полей в список записей не попадают. Интерфейс получает их
//! только по явному действию: «показать» или «скопировать».

use mdpass_core::backup::BackupKind;
use mdpass_core::format::{Document, Entry, EntryDraft, EntryId, Favorite, Field, FormatError};
use mdpass_core::password;
use mdpass_core::vault::Merged;
use serde::{Deserialize, Serialize};
use tauri::State;
use zeroize::Zeroizing;

use crate::backups::{BackupPolicy, no_backup};
use crate::commands::{CommandError, run_blocking};
use crate::session::{AppState, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    id: u64,
    path: Vec<String>,
    title: String,
    tags: Vec<String>,
    favorite: bool,
    /// Номер в избранном из строки `favorite: 2`. `None` — без номера или не в избранном.
    favorite_order: Option<u32>,
    fields: Vec<FieldView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldView {
    key: String,
    /// `None` у секретного поля.
    value: Option<String>,
    secret: bool,
}

impl From<&Entry> for EntryView {
    fn from(entry: &Entry) -> Self {
        Self {
            id: entry.id.get(),
            path: entry.path.clone(),
            title: entry.title.clone(),
            tags: entry.tags.clone(),
            favorite: entry.favorite.is_on(),
            favorite_order: match entry.favorite {
                Favorite::At(n) => Some(n),
                _ => None,
            },
            fields: entry
                .fields
                .iter()
                .map(|field| FieldView {
                    key: field.key.clone(),
                    value: (!field.secret).then(|| field.value.clone()),
                    secret: field.secret,
                })
                .collect(),
        }
    }
}

fn field_value(session: &Session, entry_id: u64, field: usize) -> Result<Zeroizing<String>, CommandError> {
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    if !session.ids_fresh {
        return Err(CommandError::Stale);
    }
    let entry = vault.document().entry(EntryId::from_raw(entry_id)).ok_or(CommandError::EntryNotFound)?;
    let field = entry.fields.get(field).ok_or(CommandError::EntryNotFound)?;
    Ok(Zeroizing::new(field.value.clone()))
}

/// Что положить в буфер. Поле `ssh` копируется готовой командой `ssh user@host`,
/// чтобы её сразу можно было вставить в терминал.
pub(crate) fn clipboard_text(key: &str, value: Zeroizing<String>) -> Zeroizing<String> {
    let value_is_command = value.trim_start().starts_with("ssh ");
    if key.eq_ignore_ascii_case("ssh") && !value_is_command && !value.trim().is_empty() {
        Zeroizing::new(format!("ssh {}", value.trim()))
    } else {
        value
    }
}

fn field_key(session: &Session, entry_id: u64, field: usize) -> Result<String, CommandError> {
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    let entry = vault.document().entry(EntryId::from_raw(entry_id)).ok_or(CommandError::EntryNotFound)?;
    Ok(entry.fields.get(field).ok_or(CommandError::EntryNotFound)?.key.clone())
}

#[tauri::command(async)]
pub fn entries(state: State<'_, AppState>) -> Result<Vec<EntryView>, CommandError> {
    let mut session = state.lock();
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    let entries = vault.document().entries().iter().map(EntryView::from).collect();
    session.ids_fresh = true;
    Ok(entries)
}

#[tauri::command(async)]
pub fn reveal_field(state: State<'_, AppState>, entry_id: u64, field: usize) -> Result<String, CommandError> {
    let value = field_value(&state.lock(), entry_id, field)?;
    Ok(value.as_str().to_owned())
}

#[tauri::command(async)]
pub fn copy_field(state: State<'_, AppState>, entry_id: u64, field: usize) -> Result<(), CommandError> {
    let session = state.lock();
    let value = clipboard_text(&field_key(&session, entry_id, field)?, field_value(&session, entry_id, field)?);
    copy_to_clipboard(&session, value)
}

/// Кладёт значение в буфер обмена и очищает его через время из настроек.
pub(crate) fn copy_to_clipboard(session: &Session, value: Zeroizing<String>) -> Result<(), CommandError> {
    #[cfg(desktop)]
    {
        crate::clipboard::copy(value, session.clipboard_clear_after())
            .map_err(|err| CommandError::Clipboard { message: err.to_string() })
    }
    #[cfg(mobile)]
    {
        drop(value);
        Err(CommandError::Clipboard {
            message: "буфер обмена на телефоне пока не поддерживается".into()
        })
    }
}

/// Запись из формы редактирования.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftInput {
    path: Vec<String>,
    title: String,
    tags: Vec<String>,
    favorite: bool,
    fields: Vec<FieldInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInput {
    key: String,
    value: Option<String>,
    secret: bool,
    /// Номер поля в текущей записи, значение которого нужно оставить. Так форма
    /// сохраняет нетронутые секреты, не получая их значений.
    keep_from: Option<usize>,
}

impl DraftInput {
    fn into_draft(self, current: Option<&Entry>) -> EntryDraft {
        let fields = self
            .fields
            .into_iter()
            .map(|input| {
                let kept = input
                    .keep_from
                    .and_then(|i| current.and_then(|entry| entry.fields.get(i)))
                    .map(|field| field.value.clone());
                let value = input.value.or(kept).unwrap_or_default();
                Field { key: input.key, value, secret: input.secret }
            })
            .collect();
        // Форма знает только «в избранном или нет». Место в избранном остаётся прежним.
        let favorite = match (self.favorite, current.map(|entry| entry.favorite)) {
            (false, _) => Favorite::No,
            (true, Some(kept)) if kept.is_on() => kept,
            (true, _) => Favorite::Yes,
        };
        EntryDraft { path: self.path, title: self.title, fields, tags: self.tags, favorite }
    }
}

impl From<FormatError> for CommandError {
    fn from(err: FormatError) -> Self {
        match err {
            FormatError::NotFound => Self::EntryNotFound,
            other => Self::Invalid { message: other.to_string() },
        }
    }
}

/// Результат правки после слияния: номера записей в слитом документе другие.
pub(crate) trait AfterMerge {
    fn after_merge(self, merged: &Merged) -> Self;
}

impl AfterMerge for () {
    fn after_merge(self, _: &Merged) -> Self {}
}

impl AfterMerge for usize {
    fn after_merge(self, _: &Merged) -> Self {
        self
    }
}

impl AfterMerge for EntryId {
    fn after_merge(self, merged: &Merged) -> Self {
        merged.id(self).unwrap_or(self)
    }
}

/// Меняет документ и сразу записывает файл. Если файл изменили вне приложения,
/// изменения сливаются (см. `Vault::save_with_backup`). Если запись не удалась,
/// документ в памяти возвращается к прежнему виду, чтобы он не расходился с файлом.
pub(crate) fn edit<T: AfterMerge>(
    session: &mut Session,
    kind: BackupKind,
    change: impl FnOnce(&mut Document) -> Result<T, FormatError>,
) -> Result<T, CommandError> {
    let policy = session.backup_policy();
    edit_with(session, policy, kind, change)
}

/// Как `edit`, но с заданным правилом копий: например, восстановление делает копию
/// всегда, даже если копии выключены, — иначе его нельзя было бы отменить.
pub(crate) fn edit_with<T: AfterMerge>(
    session: &mut Session,
    policy: Option<BackupPolicy>,
    kind: BackupKind,
    change: impl FnOnce(&mut Document) -> Result<T, FormatError>,
) -> Result<T, CommandError> {
    if !session.ids_fresh {
        return Err(CommandError::Stale);
    }
    let vault = session.vault.as_mut().ok_or(CommandError::Locked)?;
    let before = vault.document().clone();
    let result = change(vault.document_mut())?;
    let saved = match &policy {
        Some(policy) => vault.save_with_backup(policy.hook(kind)),
        None => vault.save_with_backup(no_backup),
    };
    let saved = match saved {
        Ok(saved) => saved,
        Err(err) => {
            *vault.document_mut() = before;
            return Err(err.into());
        }
    };
    crate::cloud::wake();
    let Some(merged) = saved.merged else { return Ok(result) };
    log::info!("файл изменили вне приложения: изменения слиты, конфликтов {}", merged.conflicts.len());
    crate::conflicts::remember(session, &merged);
    Ok(result.after_merge(&merged))
}

#[tauri::command]
pub async fn add_entry(state: State<'_, AppState>, draft: DraftInput) -> Result<u64, CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        edit(&mut session, BackupKind::Add, |doc| doc.add(draft.into_draft(None))).map(EntryId::get)
    })
    .await
}

#[tauri::command]
pub async fn update_entry(state: State<'_, AppState>, entry_id: u64, draft: DraftInput) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        let id = EntryId::from_raw(entry_id);
        edit(&mut session, BackupKind::Update, |doc| {
            let current = doc.entry(id).ok_or(FormatError::NotFound)?;
            doc.update(id, draft.into_draft(Some(&current)))
        })
    })
    .await
}

#[tauri::command]
pub async fn set_favorite(state: State<'_, AppState>, entry_id: u64, favorite: bool) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        edit(&mut session, BackupKind::Update, |doc| doc.set_favorite(EntryId::from_raw(entry_id), favorite))
    })
    .await
}

/// Порядок избранного: записи `entry_ids` получают номера 1, 2, 3…
#[tauri::command]
pub async fn order_favorites(state: State<'_, AppState>, entry_ids: Vec<u64>) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        let ids: Vec<EntryId> = entry_ids.into_iter().map(EntryId::from_raw).collect();
        edit(&mut session, BackupKind::Move, |doc| doc.order_favorites(&ids))
    })
    .await
}

#[tauri::command]
pub async fn delete_entry(state: State<'_, AppState>, entry_id: u64) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        edit(&mut session, BackupKind::Delete, |doc| doc.remove(EntryId::from_raw(entry_id)))
    })
    .await
}

/// Переносит запись в раздел `path`: перед записью `before_id` или в конец раздела.
#[tauri::command]
pub async fn move_entry(
    state: State<'_, AppState>,
    entry_id: u64,
    path: Vec<String>,
    before_id: Option<u64>,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        edit(&mut session, BackupKind::Move, |doc| {
            doc.move_entry(EntryId::from_raw(entry_id), path, before_id.map(EntryId::from_raw))
        })
    })
    .await
}

/// Переносит раздел со всем содержимым в раздел `parent` (пустой — верхний уровень).
#[tauri::command]
pub async fn move_section(
    state: State<'_, AppState>,
    path: Vec<String>,
    parent: Vec<String>,
    before: Option<Vec<String>>,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        edit(&mut session, BackupKind::Move, |doc| doc.move_section(&path, &parent, before.as_deref()))
    })
    .await
}

#[tauri::command(async)]
pub fn generate_password(length: Option<usize>) -> String {
    password::generate(length.unwrap_or(password::DEFAULT_LENGTH)).as_str().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(key: &str, value: &str) -> String {
        clipboard_text(key, Zeroizing::new(value.to_string())).as_str().to_owned()
    }

    #[test]
    fn ssh_field_is_copied_as_a_command() {
        assert_eq!(text("ssh", "deploy@203.0.113.10"), "ssh deploy@203.0.113.10");
        assert_eq!(text("SSH", " deploy@host "), "ssh deploy@host");
    }

    #[test]
    fn ready_commands_and_other_fields_are_copied_as_is() {
        assert_eq!(text("ssh", "ssh -p 2222 deploy@host"), "ssh -p 2222 deploy@host");
        assert_eq!(text("ssh", ""), "");
        assert_eq!(text("login", "deploy@host"), "deploy@host");
        assert_eq!(text("password", "ssh x"), "ssh x");
    }
}
