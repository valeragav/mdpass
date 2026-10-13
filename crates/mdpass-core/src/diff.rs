//! Что изменится, если вместо одного документа взять другой. Нужно экрану
//! резервных копий: «что изменится при восстановлении». Значения полей сюда
//! не попадают, только названия записей и ключи полей.

use crate::format::{Document, Entry, EntryId, FormatError};
use crate::importer::portable_draft;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Запись есть только в новом документе.
    Added { path: Vec<String>, title: String },
    /// Запись есть только в текущем документе.
    Removed { path: Vec<String>, title: String },
    /// Запись есть в обоих, но поля или теги отличаются.
    Changed { path: Vec<String>, title: String, keys: Vec<String> },
}

/// Записи сопоставляются по разделу и названию.
fn same_place(a: &Entry, b: &Entry) -> bool {
    a.path == b.path && a.title == b.title
}

/// Запись документа по разделу и названию. Если таких несколько — первая.
pub fn find_entry(doc: &Document, path: &[String], title: &str) -> Option<Entry> {
    doc.entries().into_iter().find(|e| e.path == path && e.title == title)
}

pub fn diff(current: &Document, next: &Document) -> Vec<Change> {
    let current = current.entries();
    let next = next.entries();
    let mut changes = Vec::new();

    for entry in &current {
        let (path, title) = (entry.path.clone(), entry.title.clone());
        match next.iter().find(|other| same_place(entry, other)) {
            None => changes.push(Change::Removed { path, title }),
            Some(other) => {
                let keys = changed_keys(entry, other);
                if !keys.is_empty() {
                    changes.push(Change::Changed { path, title, keys });
                }
            }
        }
    }
    for entry in &next {
        if !current.iter().any(|other| same_place(entry, other)) {
            changes.push(Change::Added { path: entry.path.clone(), title: entry.title.clone() });
        }
    }
    changes
}

fn changed_keys(current: &Entry, next: &Entry) -> Vec<String> {
    fn value<'a>(entry: &'a Entry, key: &str) -> Option<&'a str> {
        entry.fields.iter().find(|f| f.key == key).map(|f| f.value.as_str())
    }
    let mut keys: Vec<String> = Vec::new();
    for field in next.fields.iter().chain(&current.fields) {
        if !keys.contains(&field.key) && value(current, &field.key) != value(next, &field.key) {
            keys.push(field.key.clone());
        }
    }
    if current.tags != next.tags {
        keys.push("tags".into());
    }
    if current.favorite != next.favorite {
        keys.push("favorite".into());
    }
    keys
}

/// Возвращает в документ одну запись из копии: заменяет запись с тем же разделом
/// и названием или, если её уже нет, добавляет заново. Остальные записи не меняются.
/// То, что нельзя записать полем, переезжает в заметку, как при импорте.
pub fn restore_entry(current: &mut Document, from_backup: &Entry) -> Result<EntryId, FormatError> {
    let existing = find_entry(current, &from_backup.path, &from_backup.title);
    let mut entry = from_backup.clone();
    // Строки, которые и так есть в текущей записи, в заметку не дублируются.
    if existing.as_ref().is_some_and(|e| e.extra == entry.extra) {
        entry.extra.clear();
    }
    let draft = portable_draft(&entry);
    match existing {
        Some(e) => {
            current.update(e.id, draft)?;
            Ok(e.id)
        }
        None => current.add(draft),
    }
}
