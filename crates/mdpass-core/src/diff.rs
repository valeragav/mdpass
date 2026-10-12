//! Что изменится, если вместо одного документа взять другой. Нужно экрану
//! резервных копий: «что изменится при восстановлении». Значения полей сюда
//! не попадают, только названия записей и ключи полей.

use crate::format::{Document, Entry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Запись есть только в новом документе.
    Added { title: String },
    /// Запись есть только в текущем документе.
    Removed { title: String },
    /// Запись есть в обоих, но поля или теги отличаются.
    Changed { title: String, keys: Vec<String> },
}

/// Записи сопоставляются по разделу и названию.
fn same_place(a: &Entry, b: &Entry) -> bool {
    a.path == b.path && a.title == b.title
}

pub fn diff(current: &Document, next: &Document) -> Vec<Change> {
    let current = current.entries();
    let next = next.entries();
    let mut changes = Vec::new();

    for entry in &current {
        match next.iter().find(|other| same_place(entry, other)) {
            None => changes.push(Change::Removed { title: entry.title.clone() }),
            Some(other) => {
                let keys = changed_keys(entry, other);
                if !keys.is_empty() {
                    changes.push(Change::Changed { title: entry.title.clone(), keys });
                }
            }
        }
    }
    for entry in &next {
        if !current.iter().any(|other| same_place(entry, other)) {
            changes.push(Change::Added { title: entry.title.clone() });
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
    keys
}
