//! Импорт открытого Markdown-файла с паролями.
//!
//! Файл разбирается тем же форматом, что и хранилище. Найденные неровности
//! показываются пользователю, и каждую он может исправить или оставить:
//! старый ключ `teg` вместо `tags`, одинаковые заметки `note1`, `note2`, пустые
//! разделы, слитное «ключ:значение». Исправления вносятся прямо в разобранный
//! документ, поэтому всё остальное в файле остаётся как было.
//!
//! Старые файлы записывали записи заголовком: `### vps1`. Перед разбором такие
//! заголовки превращаются в жирное название `**vps1**`.

use crate::format::{
    Document, Entry, EntryDraft, EntryId, Field, FormatError, content, entry_title_of, heading_of, is_valid_key,
};

/// Переписывает заголовки-записи старого вида в жирные названия. Заголовок
/// считается записью, если в нём нет вложенных заголовков и жирных записей —
/// так файлы и читались раньше. Остальные строки не меняются.
pub fn convert_legacy_entries(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.split_inclusive('\n').collect();
    let headings: Vec<(usize, usize, String)> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| heading_of(content(line)).map(|(level, title)| (i, level, title)))
        .collect();
    let mut out: Vec<String> = lines.iter().map(|line| line.to_string()).collect();
    for (n, (start, level, title)) in headings.iter().enumerate() {
        let next = headings.get(n + 1);
        let end = next.map_or(lines.len(), |h| h.0);
        let leaf = next.is_none_or(|h| h.1 <= *level);
        let has_entries = lines[start + 1..end].iter().any(|line| entry_title_of(content(line)).is_some());
        if leaf && !has_entries && !title.is_empty() {
            let line = lines[*start];
            let ending = &line[content(line).len()..];
            out[*start] = format!("**{title}**{ending}");
        }
    }
    out.concat()
}

/// Какие исправления применить. По умолчанию все, как в дизайне.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixes {
    pub legacy_tags: bool,
    pub duplicate_notes: bool,
    pub empty_sections: bool,
    pub missing_space: bool,
}

impl Default for Fixes {
    fn default() -> Self {
        Self { legacy_tags: true, duplicate_notes: true, empty_sections: true, missing_space: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    /// Теги записаны под ключом `teg` и похожими.
    LegacyTags,
    /// Несколько заметок `note1`, `note2`… с одинаковым текстом.
    DuplicateNotes,
    /// Заголовок без полей: в хранилище это была бы пустая запись.
    EmptySections,
    /// `ключ:значение` без пробела после двоеточия.
    MissingSpace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub kind: IssueKind,
    /// Записи, в которых нашлась проблема.
    pub titles: Vec<String>,
}

pub struct Analysis {
    pub issues: Vec<Issue>,
    /// Документ после выбранных исправлений.
    pub result: Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub entries: usize,
    pub sections: usize,
    pub fields: usize,
    pub tags: usize,
}

const LEGACY_TAG_KEYS: [&str; 5] = ["teg", "tegs", "tag", "тег", "теги"];

fn is_legacy_tags(key: &str) -> bool {
    LEGACY_TAG_KEYS.contains(&key.to_lowercase().as_str())
}

fn is_note(key: &str) -> bool {
    let key = key.to_lowercase();
    key.strip_prefix("note").is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()))
}

/// Заметки записи, если их несколько и текст у всех одинаковый.
fn duplicate_notes(entry: &Entry) -> bool {
    let notes: Vec<&Field> = entry.fields.iter().filter(|f| is_note(&f.key)).collect();
    notes.len() > 1 && notes.iter().all(|f| f.value.trim() == notes[0].value.trim())
}

fn is_empty(entry: &Entry) -> bool {
    entry.fields.is_empty() && entry.tags.is_empty() && entry.extra.is_empty()
}

/// Есть ли в исходном тексте записи строка `ключ:значение` без пробела.
fn missing_space(source: &str) -> bool {
    source.lines().skip(1).any(|line| {
        if line.starts_with([' ', '\t']) {
            return false;
        }
        match line.split_once(':') {
            Some((key, value)) => {
                !key.is_empty() && !key.contains(char::is_whitespace) && value.starts_with(|c: char| !c.is_whitespace())
            }
            None => false,
        }
    })
}

pub fn analyze(markdown: &str, fixes: Fixes) -> Analysis {
    let mut doc = Document::parse(&convert_legacy_entries(markdown));
    let entries = doc.entries();

    let ids = |test: &dyn Fn(&Entry) -> bool| entries.iter().filter(|e| test(e)).map(|e| e.id).collect::<Vec<_>>();
    let found: Vec<(IssueKind, Vec<EntryId>)> = [
        (IssueKind::LegacyTags, ids(&|e| e.fields.iter().any(|f| is_legacy_tags(&f.key)))),
        (IssueKind::DuplicateNotes, ids(&duplicate_notes)),
        (IssueKind::EmptySections, ids(&is_empty)),
        (IssueKind::MissingSpace, ids(&|e| doc.entry_source(e.id).is_some_and(missing_space))),
    ]
    .into_iter()
    .filter(|(_, ids)| !ids.is_empty())
    .collect();

    let title = |id: EntryId| entries.iter().find(|e| e.id == id).map(|e| e.title.clone()).unwrap_or_default();
    let issues = found
        .iter()
        .map(|(kind, ids)| Issue { kind: *kind, titles: ids.iter().map(|&id| title(id)).collect() })
        .collect();

    for (kind, ids) in &found {
        let enabled = match kind {
            IssueKind::LegacyTags => fixes.legacy_tags,
            IssueKind::DuplicateNotes => fixes.duplicate_notes,
            IssueKind::EmptySections => fixes.empty_sections,
            IssueKind::MissingSpace => fixes.missing_space,
        };
        if enabled {
            for &id in ids {
                // Записи взяты из этого же документа, поэтому ошибок формата здесь не бывает.
                let _ = apply(&mut doc, *kind, id);
            }
        }
    }
    Analysis { issues, result: doc }
}

fn apply(doc: &mut Document, kind: IssueKind, id: EntryId) -> Result<(), FormatError> {
    let Some(entry) = doc.entry(id) else { return Ok(()) };
    match kind {
        IssueKind::EmptySections => doc.remove(id),
        IssueKind::MissingSpace => doc.rewrite(id),
        IssueKind::LegacyTags => {
            let mut draft = draft_of(&entry);
            let (legacy, fields): (Vec<Field>, Vec<Field>) =
                draft.fields.drain(..).partition(|f| is_legacy_tags(&f.key));
            draft.fields = fields;
            for field in &legacy {
                for tag in field.value.split(',') {
                    let tag = tag.trim().trim_start_matches('[').trim_end_matches(']').trim();
                    if !tag.is_empty() && !draft.tags.iter().any(|t| t == tag) {
                        draft.tags.push(tag.to_string());
                    }
                }
            }
            doc.update(id, draft)
        }
        IssueKind::DuplicateNotes => {
            let mut draft = draft_of(&entry);
            let mut kept = false;
            draft.fields.retain_mut(|field| {
                if !is_note(&field.key) {
                    return true;
                }
                if kept {
                    return false;
                }
                kept = true;
                field.key = "note".into();
                true
            });
            doc.update(id, draft)
        }
    }
}

/// Запись, которую можно добавить в другое хранилище. В исходном файле бывает то,
/// что разбор прощает, а добавление записи нет: раздел без названия, ключ вроде `#x`,
/// скобки внутри тега. Такое не теряется, а переезжает в заметку записи.
fn portable_draft(entry: &Entry) -> EntryDraft {
    let mut loose: Vec<String> = entry.extra.iter().map(|line| line.trim().to_string()).collect();
    let mut fields = Vec::new();
    for field in &entry.fields {
        if is_valid_key(field.key.trim()) {
            fields.push(field.clone());
        } else {
            loose.push(format!("{}: {}", field.key, field.value.replace('\n', " ")));
        }
    }
    let loose: Vec<String> = loose.into_iter().filter(|line| !line.is_empty()).collect();
    if !loose.is_empty() {
        let loose = loose.join("\n");
        match fields.iter_mut().find(|f| f.key.eq_ignore_ascii_case("note")) {
            Some(note) => note.value = format!("{}\n{loose}", note.value),
            None => fields.push(Field::new("note", loose)),
        }
    }
    let path = entry.path.iter().map(|part| part.trim().to_string()).filter(|part| !part.is_empty()).collect();
    let tags = entry
        .tags
        .iter()
        .map(|tag| tag.chars().filter(|c| !matches!(c, ',' | '[' | ']')).collect::<String>())
        .filter(|tag| !tag.trim().is_empty())
        .collect();
    EntryDraft { path, title: entry.title.clone(), fields, tags }
}

fn draft_of(entry: &Entry) -> EntryDraft {
    EntryDraft {
        path: entry.path.clone(),
        title: entry.title.clone(),
        fields: entry.fields.clone(),
        tags: entry.tags.clone(),
    }
}

pub fn stats(doc: &Document) -> Stats {
    let entries = doc.entries();
    let mut sections: Vec<&[String]> = Vec::new();
    let mut tags: Vec<&str> = Vec::new();
    for entry in &entries {
        for depth in 1..=entry.path.len() {
            let section = &entry.path[..depth];
            if !sections.contains(&section) {
                sections.push(section);
            }
        }
        for tag in &entry.tags {
            if !tags.contains(&tag.as_str()) {
                tags.push(tag);
            }
        }
    }
    Stats {
        entries: entries.len(),
        sections: sections.len(),
        fields: entries.iter().map(|e| e.fields.len()).sum(),
        tags: tags.len(),
    }
}

/// Переносит импортированные записи в хранилище и возвращает их число.
///
/// В пустое хранилище документ переносится целиком, вместе с текстом вне записей
/// и порядком строк. В непустое записи добавляются по одной; строки без ключа
/// становятся заметкой, потому что вне исходного файла им негде жить.
pub fn merge_into(target: &mut Document, imported: &Document) -> Result<usize, FormatError> {
    let entries = imported.entries();
    if target.is_blank() {
        *target = imported.clone();
        target.ensure_header();
        return Ok(entries.len());
    }
    let mut work = target.clone();
    for entry in &entries {
        work.add(portable_draft(entry))?;
    }
    *target = work;
    Ok(entries.len())
}
