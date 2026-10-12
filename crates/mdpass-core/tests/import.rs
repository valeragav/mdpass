use mdpass_core::format::{Document, EntryDraft, Field};
use mdpass_core::importer::{Fixes, IssueKind, analyze, convert_legacy_entries, merge_into, stats};

/// Выдуманный открытый файл с теми же неровностями, что бывают в ручных заметках.
const SOURCE: &str = "\
# work

## server

### vps1 (инфра)
ssh: deploy@203.0.113.10
password: aaa

### vps2 (бизнес)
ssh:deploy@198.51.100.24
password: bbb

### БД анкеты
port: 5432
note1: таблицу tmp можно удалять
note2: таблицу tmp можно удалять

## site

### leading
url: https://example.test/
teg: [server], [work], [prod]

# my

### почта
login: me@example.test
password: ccc
строка без ключа

## заметки
";

fn titles(doc: &Document) -> Vec<String> {
    doc.entries().into_iter().map(|e| e.title).collect()
}

#[test]
fn all_issues_are_found_with_the_entries_they_touch() {
    let analysis = analyze(SOURCE, Fixes::default());
    let found: Vec<_> = analysis.issues.iter().map(|i| (i.kind, i.titles.clone())).collect();
    assert_eq!(
        found,
        vec![
            (IssueKind::LegacyTags, vec!["leading".to_string()]),
            (IssueKind::DuplicateNotes, vec!["БД анкеты".to_string()]),
            (IssueKind::EmptySections, vec!["заметки".to_string()]),
            (IssueKind::MissingSpace, vec!["vps2 (бизнес)".to_string()]),
        ]
    );
}

#[test]
fn fixes_clean_the_document() {
    let result = analyze(SOURCE, Fixes::default()).result;
    assert_eq!(titles(&result), ["vps1 (инфра)", "vps2 (бизнес)", "БД анкеты", "leading", "почта"]);

    let entries = result.entries();
    let leading = &entries[3];
    assert_eq!(leading.tags, ["server", "work", "prod"]);
    assert!(leading.fields.iter().all(|f| f.key != "teg"));

    let notes: Vec<_> = entries[2].fields.iter().filter(|f| f.key.starts_with("note")).collect();
    assert_eq!(notes.len(), 1);
    assert_eq!((notes[0].key.as_str(), notes[0].value.as_str()), ("note", "таблицу tmp можно удалять"));

    let md = result.to_markdown();
    assert!(md.contains("**vps1 (инфра)**\nssh: deploy@203.0.113.10\n"), "{md}");
    assert!(md.contains("ssh: deploy@198.51.100.24\n"), "{md}");
    assert!(md.contains("строка без ключа\n"), "строки без ключа не теряются");
    assert!(!md.contains("заметки"));
}

#[test]
fn unchecked_fixes_only_turn_entry_headings_bold() {
    let none = Fixes { legacy_tags: false, duplicate_notes: false, empty_sections: false, missing_space: false };
    let analysis = analyze(SOURCE, none);
    assert_eq!(analysis.issues.len(), 4, "проблемы показываются, даже если их не исправлять");
    let expected = SOURCE
        .replace("### vps1 (инфра)", "**vps1 (инфра)**")
        .replace("### vps2 (бизнес)", "**vps2 (бизнес)**")
        .replace("### БД анкеты", "**БД анкеты**")
        .replace("### leading", "**leading**")
        .replace("### почта", "**почта**")
        .replace("## заметки", "**заметки**");
    assert_eq!(analysis.result.to_markdown(), expected);
}

#[test]
fn files_in_the_new_format_are_not_converted() {
    let source = "# work\n\n## server\n\n**a**\nk: v\n\n# пустой раздел\n";
    assert_eq!(convert_legacy_entries(source), source.replace("# пустой раздел", "**пустой раздел**"));
    let nested = "# work\n## server\n**a**\nk: v\n";
    assert_eq!(convert_legacy_entries(nested), nested);
}

#[test]
fn different_notes_are_kept_apart() {
    let analysis = analyze("### a\nnote1: раз\nnote2: два\n", Fixes::default());
    assert!(analysis.issues.is_empty());
    assert_eq!(analysis.result.entries()[0].fields.len(), 2);
}

#[test]
fn stats_count_entries_sections_fields_and_tags() {
    let result = analyze(SOURCE, Fixes::default()).result;
    let s = stats(&result);
    assert_eq!((s.entries, s.sections, s.fields, s.tags), (5, 4, 9, 3));
}

#[test]
fn import_into_an_empty_vault_keeps_the_file_layout() {
    let result = analyze(SOURCE, Fixes::default()).result;
    let mut vault = Document::default();
    assert_eq!(merge_into(&mut vault, &result).unwrap(), 5);
    assert_eq!(vault.to_markdown(), format!("<!-- mdpass:1 -->\n\n{}", result.to_markdown()));
}

#[test]
fn import_into_a_filled_vault_appends_entries_and_keeps_loose_lines_as_a_note() {
    let result = analyze(SOURCE, Fixes::default()).result;
    let mut vault = Document::default();
    vault
        .add(EntryDraft {
            path: vec!["work".into()],
            title: "старая".into(),
            fields: vec![Field::new("k", "v")],
            tags: vec![],
        })
        .unwrap();
    assert_eq!(merge_into(&mut vault, &result).unwrap(), 5);
    let entries = vault.entries();
    assert_eq!(entries.len(), 6);
    let pochta = entries.iter().find(|e| e.title == "почта").unwrap();
    assert!(pochta.fields.iter().any(|f| f.key == "note" && f.value == "строка без ключа"));
}

// Найдено стресс-тестом: раздел без названия или ключ вроде «#x» роняли весь импорт.
#[test]
fn odd_sections_keys_and_tags_do_not_stop_an_import() {
    let result = analyze("# \n### a\n#x: значение\ntags: [ok], [b]ad]\nlogin: l\n", Fixes::default()).result;
    let mut vault = Document::default();
    vault
        .add(EntryDraft {
            path: vec!["work".into()],
            title: "есть".into(),
            fields: vec![Field::new("k", "v")],
            tags: vec![],
        })
        .unwrap();
    assert_eq!(merge_into(&mut vault, &result).unwrap(), 1);
    let a = vault.entries().into_iter().find(|e| e.title == "a").unwrap();
    assert!(a.path.is_empty());
    assert!(a.fields.iter().any(|f| f.key == "login"));
    assert!(a.fields.iter().any(|f| f.key == "note" && f.value.contains("#x: значение")));
    assert!(a.tags.iter().all(|t| !t.contains(']')));
}
