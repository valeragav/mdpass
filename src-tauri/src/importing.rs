//! Команды экрана импорта из открытого Markdown-файла.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use mdpass_core::backup::BackupKind;
use mdpass_core::importer::{Analysis, Fixes, IssueKind, analyze, merge_into, stats};
use serde::{Deserialize, Serialize};
use tauri::State;
use zeroize::Zeroizing;

use crate::commands::{CommandError, run_blocking};
use crate::entries::edit;
use crate::session::AppState;

/// Файл с паролями обычно занимает килобайты. Ограничение защищает от случайно
/// выбранного огромного файла.
const MAX_SOURCE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixesInput {
    legacy_tags: bool,
    duplicate_notes: bool,
    empty_sections: bool,
    missing_space: bool,
}

impl From<FixesInput> for Fixes {
    fn from(input: FixesInput) -> Self {
        Self {
            legacy_tags: input.legacy_tags,
            duplicate_notes: input.duplicate_notes,
            empty_sections: input.empty_sections,
            missing_space: input.missing_space,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueView {
    kind: &'static str,
    titles: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPlace {
    path: Vec<String>,
    title: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    entries: usize,
    sections: usize,
    fields: usize,
    tags: usize,
    issues: Vec<IssueView>,
    /// Где окажутся записи. Значений полей здесь нет.
    places: Vec<EntryPlace>,
}

fn kind_name(kind: IssueKind) -> &'static str {
    match kind {
        IssueKind::LegacyTags => "legacyTags",
        IssueKind::DuplicateNotes => "duplicateNotes",
        IssueKind::EmptySections => "emptySections",
        IssueKind::MissingSpace => "missingSpace",
    }
}

fn read_source(path: &Path) -> Result<Zeroizing<String>, CommandError> {
    let io = |err: std::io::Error| CommandError::Io { message: err.to_string() };
    let file = fs::File::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => CommandError::NotFound,
        _ => io(err),
    })?;
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(CommandError::Invalid {
            message: "Файл больше 10 МБ: это не похоже на файл с паролями".into()
        });
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| CommandError::Invalid { message: "Файл не в кодировке UTF-8".into() })?;
    Ok(Zeroizing::new(text.to_owned()))
}

fn preview(analysis: &Analysis) -> ImportPreview {
    let counts = stats(&analysis.result);
    ImportPreview {
        entries: counts.entries,
        sections: counts.sections,
        fields: counts.fields,
        tags: counts.tags,
        issues: analysis
            .issues
            .iter()
            .map(|issue| IssueView { kind: kind_name(issue.kind), titles: issue.titles.clone() })
            .collect(),
        places: analysis
            .result
            .entries()
            .into_iter()
            .map(|entry| EntryPlace { path: entry.path, title: entry.title })
            .collect(),
    }
}

#[tauri::command(async)]
pub fn analyze_import(path: String, fixes: FixesInput) -> Result<ImportPreview, CommandError> {
    let source = read_source(Path::new(&path))?;
    Ok(preview(&analyze(&source, fixes.into())))
}

/// Переносит записи в открытое хранилище и возвращает их число.
#[tauri::command]
pub async fn run_import(state: State<'_, AppState>, path: String, fixes: FixesInput) -> Result<usize, CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let path = PathBuf::from(path);
        let source = read_source(&path)?;
        let imported = analyze(&source, fixes.into()).result;
        let mut session = state.lock();
        let count = edit(&mut session, BackupKind::Import, |doc| merge_into(doc, &imported))?;
        // Копия файла с телефона больше не нужна: в ней пароли открытым текстом.
        if crate::picked::is_import_copy(&path) {
            let _ = fs::remove_file(&path);
            session.last_import = None;
        } else {
            session.last_import = Some(path);
        }
        Ok(count)
    })
    .await
}

/// Удаляет исходный открытый файл после импорта. Удалить можно только тот файл,
/// который импортировали в этой сессии.
#[tauri::command(async)]
pub fn delete_import_source(state: State<'_, AppState>, path: String) -> Result<(), CommandError> {
    let mut session = state.lock();
    let path = PathBuf::from(path);
    if session.last_import.as_ref() != Some(&path) {
        return Err(CommandError::Invalid {
            message: "Удалить можно только файл, который вы только что импортировали".into(),
        });
    }
    fs::remove_file(&path).map_err(|err| CommandError::Io { message: err.to_string() })?;
    session.last_import = None;
    Ok(())
}
