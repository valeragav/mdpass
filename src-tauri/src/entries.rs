//! Команды для записей открытого хранилища.
//!
//! Значения секретных полей в список записей не попадают. Интерфейс получает их
//! только по явному действию: «показать» или «скопировать».

use mdpass_core::format::{Entry, EntryId};
use serde::Serialize;
use tauri::State;
use zeroize::Zeroizing;

use crate::commands::CommandError;
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
    let value = field_value(&state.lock(), entry_id, field)?;
    #[cfg(desktop)]
    {
        crate::clipboard::copy(value).map_err(|err| CommandError::Clipboard { message: err.to_string() })
    }
    #[cfg(mobile)]
    {
        drop(value);
        Err(CommandError::Clipboard {
            message: "буфер обмена на телефоне пока не поддерживается".into()
        })
    }
}
