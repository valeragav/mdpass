//! Конфликты слияния, которые ждут выбора пользователя.
//!
//! При записи или синхронизации конфликт сразу решается «оставить обе»: моя версия
//! записи ложится рядом с пометкой «(копия с этого устройства)». Так ничего не
//! теряется, даже если приложение закроют, не выбрав. Здесь сессия помнит такие
//! записи, а окно выбора сворачивает две версии в одну: мою, с другого устройства
//! или оставляет обе.
//!
//! Записи ищутся по разделу и названию, а не по номеру: номера меняются при каждом
//! слиянии. Значения секретных полей в интерфейс не уходят — их показывает обычная
//! команда «показать» по номеру записи и поля.

use mdpass_core::backup::BackupKind;
use mdpass_core::format::{Document, Entry, FormatError};
use mdpass_core::merge::{EXTRA, SECTION, TAGS, TITLE, collapse};
use mdpass_core::vault::{Merged, copy_title};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{CommandError, run_blocking};
use crate::entries::edit;
use crate::session::{AppState, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    /// Запись изменили на двух устройствах по-разному.
    Edited,
    /// Я удалил запись, а на другом устройстве её изменили: она вернулась.
    DeletedHere,
    /// На другом устройстве запись удалили, а я её изменил: она осталась.
    DeletedThere,
}

#[derive(Debug, Clone)]
pub struct Pending {
    kind: ConflictKind,
    /// Запись после слияния.
    path: Vec<String>,
    title: String,
    /// Моя копия рядом с ней (только при `Edited`).
    copy_path: Vec<String>,
    copy_title: String,
    /// Название моей версии без пометки копии.
    my_title: String,
    /// Что разошлось: ключи полей и части записи.
    keys: Vec<String>,
    their_device: Option<String>,
    their_at: Option<u64>,
    my_at: Option<u64>,
}

/// Запоминает конфликты, которые остались после слияния.
pub fn remember(session: &mut Session, merged: &Merged) {
    let their_device = merged.theirs.as_ref().and_then(|s| s.device.clone());
    let their_at = merged.theirs.as_ref().and_then(|s| s.at);
    let my_at = merged.mine.as_ref().and_then(|s| s.at);
    for conflict in &merged.conflicts {
        let (kind, place, copy) = match (&conflict.mine, &conflict.theirs) {
            (Some(mine), Some(theirs)) => (ConflictKind::Edited, theirs, Some((mine, copy_title(conflict)))),
            (None, Some(theirs)) => (ConflictKind::DeletedHere, theirs, None),
            (Some(mine), None) => (ConflictKind::DeletedThere, mine, None),
            (None, None) => continue,
        };
        let pending = Pending {
            kind,
            path: place.path.clone(),
            title: place.title.clone(),
            copy_path: copy.as_ref().map(|(mine, _)| mine.path.clone()).unwrap_or_default(),
            copy_title: copy.as_ref().map(|(_, title)| title.clone()).unwrap_or_default(),
            my_title: conflict.mine.as_ref().map_or_else(|| place.title.clone(), |m| m.title.clone()),
            keys: conflict.keys.clone(),
            their_device: their_device.clone(),
            their_at,
            my_at,
        };
        session.conflicts.retain(|p| !(p.path == pending.path && p.title == pending.title));
        session.conflicts.push(pending);
    }
}

fn find(doc: &Document, path: &[String], title: &str) -> Option<Entry> {
    doc.entries().into_iter().find(|e| e.path == path && e.title == title)
}

/// Обе версии записи в документе: запись после слияния и моя копия.
fn locate(doc: &Document, pending: &Pending) -> Option<(Entry, Option<Entry>)> {
    let original = find(doc, &pending.path, &pending.title)?;
    match pending.kind {
        ConflictKind::Edited => {
            let copy = find(doc, &pending.copy_path, &pending.copy_title)?;
            Some((original, Some(copy)))
        }
        _ => Some((original, None)),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    kind: ConflictKind,
    path: Vec<String>,
    title: String,
    /// Номер записи после слияния.
    entry_id: u64,
    /// Номер моей копии (только при `edited`).
    copy_id: Option<u64>,
    rows: Vec<RowView>,
    their_device: Option<String>,
    their_at_ms: Option<u64>,
    my_at_ms: Option<u64>,
}

/// Строка таблицы «что разошлось».
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowView {
    /// Ключ поля или часть записи: `title`, `section`, `tags`, `extra`.
    key: String,
    /// Это не поле, а часть записи.
    part: bool,
    secret: bool,
    /// Значения. `None` у секретного поля и у поля, которого в версии нет.
    mine: Option<String>,
    theirs: Option<String>,
    /// Номер поля в моей копии и в записи после слияния: по нему «показать».
    /// `None` — такого поля в этой версии нет.
    mine_field: Option<usize>,
    theirs_field: Option<usize>,
}

fn rows(pending: &Pending, original: &Entry, copy: Option<&Entry>) -> Vec<RowView> {
    let mine = copy.unwrap_or(original);
    pending
        .keys
        .iter()
        .map(|key| {
            let part = |name: &str, mine: String, theirs: String| RowView {
                key: name.into(),
                part: true,
                secret: false,
                mine: Some(mine),
                theirs: Some(theirs),
                mine_field: None,
                theirs_field: None,
            };
            match key.as_str() {
                TITLE => part("title", pending.my_title.clone(), original.title.clone()),
                SECTION => part("section", mine.path.join(" / "), original.path.join(" / ")),
                TAGS => part("tags", mine.tags.join(", "), original.tags.join(", ")),
                EXTRA => part("extra", mine.extra.join("\n"), original.extra.join("\n")),
                key => {
                    let mine_field = mine.fields.iter().position(|f| f.key == key);
                    let theirs_field = original.fields.iter().position(|f| f.key == key);
                    let secret = mine_field.is_some_and(|i| mine.fields[i].secret)
                        || theirs_field.is_some_and(|i| original.fields[i].secret);
                    let value = |entry: &Entry, index: Option<usize>| {
                        index.filter(|_| !secret).map(|i| entry.fields[i].value.clone())
                    };
                    RowView {
                        key: key.into(),
                        part: false,
                        secret,
                        mine: value(mine, mine_field),
                        theirs: value(original, theirs_field),
                        mine_field,
                        theirs_field,
                    }
                }
            }
        })
        .collect()
}

/// Конфликты, которые ждут выбора. Те, чьих записей уже нет (их удалили или
/// переименовали вручную), забываются.
#[tauri::command]
pub fn conflicts(state: State<'_, AppState>) -> Result<Vec<ConflictView>, CommandError> {
    let mut session = state.lock();
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    let doc = vault.document().clone();
    session.conflicts.retain(|pending| locate(&doc, pending).is_some());
    Ok(session
        .conflicts
        .iter()
        .filter_map(|pending| {
            let (original, copy) = locate(&doc, pending)?;
            Some(ConflictView {
                kind: pending.kind,
                path: pending.path.clone(),
                title: pending.title.clone(),
                entry_id: original.id.get(),
                copy_id: copy.as_ref().map(|c| c.id.get()),
                rows: rows(pending, &original, copy.as_ref()),
                their_device: pending.their_device.clone(),
                their_at_ms: pending.their_at.map(|s| s * 1000),
                my_at_ms: pending.my_at.map(|s| s * 1000),
            })
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictChoice {
    Mine,
    Theirs,
    Both,
}

/// Решает конфликт номер `index` из списка [`conflicts`].
#[tauri::command]
pub async fn resolve_conflict(
    state: State<'_, AppState>,
    index: usize,
    choice: ConflictChoice,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        let pending = session.conflicts.get(index).cloned().ok_or(CommandError::EntryNotFound)?;
        session.conflicts.remove(index);
        // «Оставить обе» и «оставить вернувшуюся запись» ничего не меняют: так уже записано.
        let change = matches!(
            (pending.kind, choice),
            (ConflictKind::Edited, ConflictChoice::Mine | ConflictChoice::Theirs)
                | (ConflictKind::DeletedHere, ConflictChoice::Mine)
                | (ConflictKind::DeletedThere, ConflictChoice::Theirs)
        );
        if !change {
            return Ok(());
        }
        edit(&mut session, BackupKind::Update, |doc| {
            let (original, copy) = locate(doc, &pending).ok_or(FormatError::NotFound)?;
            match (pending.kind, copy) {
                (ConflictKind::Edited, Some(copy)) => {
                    collapse(doc, &original, &copy, &pending.keys, &pending.my_title, choice == ConflictChoice::Mine)
                }
                // Удалить запись: моё удаление здесь или их удаление там.
                _ => doc.remove(original.id),
            }
        })
    })
    .await
}
