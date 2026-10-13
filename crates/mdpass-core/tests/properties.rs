//! Проверки формата на случайных файлах и случайных цепочках операций.

use mdpass_core::format::{Document, Entry, EntryDraft, Field};
use proptest::prelude::*;

/// Строки, из которых состоят файлы хранилища, и немного мусора вокруг них.
fn line() -> impl Strategy<Value = String> {
    prop_oneof![
        "#{1,7} [a-zа-я ]{0,6}",
        r"\*\*[a-zа-я *]{0,6}\*\*[ ]?",
        "[a-z_]{1,6}!?: ?[a-zа-я:/ ]{0,8}",
        "tags: \\[[a-z]{1,4}\\](, \\[[a-z]{1,4}\\]){0,2}",
        "  [a-zа-я *]{0,6}",
        "[ \t]{0,2}",
        "[a-zа-я *#:<!>-]{0,12}",
        Just("<!-- mdpass:1 -->".to_string()),
    ]
}

fn file() -> impl Strategy<Value = String> {
    (prop::collection::vec(line(), 0..40), any::<bool>(), any::<bool>(), any::<bool>()).prop_map(
        |(lines, crlf, final_newline, bom)| {
            let eol = if crlf { "\r\n" } else { "\n" };
            let mut text = lines.join(eol);
            if final_newline && !text.is_empty() {
                text.push_str(eol);
            }
            if bom { format!("\u{feff}{text}") } else { text }
        },
    )
}

/// Что видит пользователь: раздел, название, поля и теги.
type Seen = Vec<(Vec<String>, String, Vec<(String, String, bool)>, Vec<String>)>;

fn seen(entries: &[Entry]) -> Seen {
    entries
        .iter()
        .map(|e| {
            let fields = e.fields.iter().map(|f| (f.key.clone(), f.value.clone(), f.secret)).collect();
            (e.path.clone(), e.title.clone(), fields, e.tags.clone())
        })
        .collect()
}

#[derive(Debug, Clone)]
enum Op {
    Add { path: Vec<String>, title: String, value: String },
    Update { pick: usize, title: String, value: String },
    Remove { pick: usize },
    MoveEntry { pick: usize, path: Vec<String>, before: Option<usize> },
    MoveSection { pick: usize, parent: usize },
}

fn name() -> impl Strategy<Value = String> {
    "[a-zа-я][a-zа-я ]{0,4}[a-zа-я]?"
}

fn path() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(prop_oneof![Just("work".to_string()), Just("my".to_string()), name()], 0..7)
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (path(), name(), "[a-z ]{0,6}(\n[a-z*]{1,4})?").prop_map(|(path, title, value)| Op::Add { path, title, value }),
        (any::<usize>(), name(), "[a-z ]{0,6}").prop_map(|(pick, title, value)| Op::Update { pick, title, value }),
        any::<usize>().prop_map(|pick| Op::Remove { pick }),
        (any::<usize>(), path(), prop::option::of(any::<usize>())).prop_map(|(pick, path, before)| Op::MoveEntry {
            pick,
            path,
            before
        }),
        (any::<usize>(), any::<usize>()).prop_map(|(pick, parent)| Op::MoveSection { pick, parent }),
    ]
}

fn draft(path: Vec<String>, title: String, value: String) -> EntryDraft {
    EntryDraft {
        path,
        title,
        fields: vec![Field::new("login", value), Field::new("password", "p")],
        tags: vec![],
        favorite: false,
    }
}

/// Все разделы документа: пути всех записей и их предков.
fn sections(doc: &Document) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    for entry in doc.entries() {
        for depth in 1..=entry.path.len() {
            let section = entry.path[..depth].to_vec();
            if !out.contains(&section) {
                out.push(section);
            }
        }
    }
    out
}

fn apply(doc: &mut Document, op: Op) {
    let entries = doc.entries();
    let entry = |pick: usize| (!entries.is_empty()).then(|| &entries[pick % entries.len()]);
    // Ошибки формата здесь допустимы (слишком глубоко, тот же раздел и т. п.):
    // проверяем, что после них документ остаётся целым.
    let _ = match op {
        Op::Add { path, title, value } => doc.add(draft(path, title, value)).map(drop),
        Op::Update { pick, title, value } => match entry(pick) {
            Some(e) => doc.update(e.id, draft(e.path.clone(), title, value)),
            None => Ok(()),
        },
        Op::Remove { pick } => match entry(pick) {
            Some(e) => doc.remove(e.id),
            None => Ok(()),
        },
        Op::MoveEntry { pick, path, before } => match entry(pick) {
            Some(e) => doc.move_entry(e.id, path, before.and_then(entry).map(|b| b.id)),
            None => Ok(()),
        },
        Op::MoveSection { pick, parent } => {
            let all = sections(doc);
            if all.is_empty() {
                Ok(())
            } else {
                let path = all[pick % all.len()].clone();
                let mut parents = all.clone();
                parents.push(Vec::new());
                doc.move_section(&path, &parents[parent % parents.len()], None)
            }
        }
    };
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2000, ..ProptestConfig::default() })]

    /// Файл без правок после разбора и сборки не меняется ни на байт.
    #[test]
    fn any_file_round_trips(text in file()) {
        prop_assert_eq!(Document::parse(&text).to_markdown(), text);
    }

    /// Разбор не теряет записи: собранный файл читается в те же записи.
    #[test]
    fn parsed_entries_survive_a_rewrite(text in file()) {
        let doc = Document::parse(&text);
        let again = Document::parse(&doc.to_markdown());
        prop_assert_eq!(seen(&again.entries()), seen(&doc.entries()));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 500, ..ProptestConfig::default() })]

    /// После любой цепочки операций файл читается обратно в те же записи.
    #[test]
    fn edits_keep_the_file_consistent(start in file(), ops in prop::collection::vec(op(), 1..25)) {
        let mut doc = Document::parse(&start);
        for op in ops {
            apply(&mut doc, op.clone());
            let reread = Document::parse(&doc.to_markdown());
            prop_assert_eq!(seen(&reread.entries()), seen(&doc.entries()), "после {:?}\n{}", op, doc.to_markdown());
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1000, ..ProptestConfig::default() })]

    /// Импорт любого файла с любым набором исправлений не падает, а результат
    /// читается обратно в те же записи.
    #[test]
    fn import_of_any_file_is_consistent(text in file(), fixes in any::<[bool; 4]>()) {
        use mdpass_core::importer::{Fixes, analyze, merge_into};
        let fixes = Fixes { legacy_tags: fixes[0], duplicate_notes: fixes[1], empty_sections: fixes[2], missing_space: fixes[3] };
        let result = analyze(&text, fixes).result;
        let reread = Document::parse(&result.to_markdown());
        prop_assert_eq!(seen(&reread.entries()), seen(&result.entries()));

        let mut vault = Document::default();
        vault.add(draft(vec!["work".into()], "есть".into(), "x".into())).unwrap();
        prop_assert!(merge_into(&mut vault, &result).is_ok());
    }
}
