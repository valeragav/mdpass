//! Команды для записей открытого хранилища.
//!
//! Значения секретных полей в список записей не попадают. Интерфейс получает их
//! только по явному действию: «показать» или «скопировать».

use mdpass_core::backup::BackupKind;
use mdpass_core::format::{Document, Entry, EntryDraft, EntryId, Field, FormatError};
use mdpass_core::password;
use serde::{Deserialize, Serialize};
use tauri::State;
use zeroize::Zeroizing;

use crate::backups::no_backup;
use crate::commands::{CommandError, run_blocking};
use crate::session::{AppState, Session};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    id: u64,
    path: Vec<String>,
    title: String,
    tags: Vec<String>,
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
    let entry = vault.document().entry(EntryId::from_raw(entry_id)).ok_or(CommandError::EntryNotFound)?;
    let field = entry.fields.get(field).ok_or(CommandError::EntryNotFound)?;
    Ok(Zeroizing::new(field.value.clone()))
}

/// Что положить в буфер. Поле `ssh` копируется готовой командой `ssh user@host`,
/// чтобы её сразу можно было вставить в терминал.
fn clipboard_text(key: &str, value: Zeroizing<String>) -> Zeroizing<String> {
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

#[tauri::command]
pub fn entries(state: State<'_, AppState>) -> Result<Vec<EntryView>, CommandError> {
    let session = state.lock();
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    Ok(vault.document().entries().iter().map(EntryView::from).collect())
}

#[tauri::command]
pub fn reveal_field(state: State<'_, AppState>, entry_id: u64, field: usize) -> Result<String, CommandError> {
    let value = field_value(&state.lock(), entry_id, field)?;
    Ok(value.as_str().to_owned())
}

#[tauri::command]
pub fn copy_field(state: State<'_, AppState>, entry_id: u64, field: usize) -> Result<(), CommandError> {
    let session = state.lock();
    let value = clipboard_text(&field_key(&session, entry_id, field)?, field_value(&session, entry_id, field)?);
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
        EntryDraft { path: self.path, title: self.title, fields, tags: self.tags }
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

/// Меняет документ и сразу записывает файл. Если запись не удалась, документ
/// в памяти возвращается к прежнему виду, чтобы он не расходился с файлом.
pub(crate) fn edit<T>(
    session: &mut Session,
    kind: BackupKind,
    change: impl FnOnce(&mut Document) -> Result<T, FormatError>,
) -> Result<T, CommandError> {
    let policy = session.backup_policy();
    let vault = session.vault.as_mut().ok_or(CommandError::Locked)?;
    let before = vault.document().clone();
    let result = change(vault.document_mut())?;
    let saved = match &policy {
        Some(policy) => vault.save_with_backup(policy.hook(kind)),
        None => vault.save_with_backup(no_backup),
    };
    if let Err(err) = saved {
        *vault.document_mut() = before;
        return Err(err.into());
    }
    Ok(result)
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

#[tauri::command]
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
