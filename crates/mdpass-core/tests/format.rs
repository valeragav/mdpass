use mdpass_core::format::{Document, EntryDraft, Field, FormatError};

fn field(key: &str, value: &str) -> Field {
    Field::new(key, value)
}

fn draft(path: &[&str], title: &str, fields: Vec<Field>, tags: &[&str]) -> EntryDraft {
    EntryDraft {
        path: path.iter().map(|s| s.to_string()).collect(),
        title: title.to_string(),
        fields,
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

const SAMPLE: &str = "\
# work

## server

### alpha (стейдж)
ssh: deploy@203.0.113.10
password: correct-horse

### beta
port: 5432
user: app_user
password: battery-staple
note: старая таблица orders_tmp не нужна

## site

### example
url: https://example.test/
url_admin:  https://example.test/admin
login: admin
password: hunter2
tags: [server], [work], [prod]

# my

### почта
login: me@example.test
password: secret
";

// Файл без изменений после разбора и сборки совпадает с исходным побайтно.
#[test]
fn unchanged_document_round_trips_byte_for_byte() {
    let inputs = [
        SAMPLE.to_string(),
        SAMPLE.replace('\n', "\r\n"),
        SAMPLE.trim_end().to_string(),
        format!("\u{feff}{SAMPLE}"),
        String::new(),
        "просто текст без заголовков\n".to_string(),
        "# work\nтекст раздела\n\n### a\nssh:host-without-space\n   \nстрока без ключа\n\n\n# b\n".to_string(),
    ];
    for input in inputs {
        assert_eq!(Document::parse(&input).to_markdown(), input);
    }
}

#[test]
fn entries_get_section_path_title_and_fields() {
    let doc = Document::parse(SAMPLE);
    let entries = doc.entries();
    let titles: Vec<_> = entries.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["alpha (стейдж)", "beta", "example", "почта"]);

    let alpha = &entries[0];
    assert_eq!(alpha.path, ["work", "server"]);
    assert_eq!(alpha.fields, vec![field("ssh", "deploy@203.0.113.10"), field("password", "correct-horse")]);

    assert_eq!(entries[3].path, ["my"]);
}

#[test]
fn value_starts_after_colon_and_spaces() {
    let doc = Document::parse("### a\nssh:host\nurl_admin:  https://x.test:8080/a\n");
    let fields = &doc.entries()[0].fields;
    assert_eq!(fields[0].value, "host");
    assert_eq!(fields[1].value, "https://x.test:8080/a");
}

#[test]
fn tags_are_parsed_from_brackets() {
    let doc = Document::parse(SAMPLE);
    let example = &doc.entries()[2];
    assert_eq!(example.tags, ["server", "work", "prod"]);
    assert!(example.fields.iter().all(|f| f.key != "tags"));
}

#[test]
fn secret_fields_are_detected_by_name_or_marker() {
    let doc = Document::parse("### a\npassword: p\napi_token: t\nPIN: 1\npin_hint!: h\nlogin: l\n");
    let entries = doc.entries();
    let secret: Vec<_> = entries[0].fields.iter().map(|f| (f.key.as_str(), f.secret)).collect();
    assert_eq!(secret, [("password", true), ("api_token", true), ("PIN", true), ("pin_hint", true), ("login", false)]);
}

#[test]
fn indented_lines_continue_the_previous_value() {
    let doc = Document::parse("### a\nnote: первая строка\n  вторая строка\nlogin: l\n");
    let fields = &doc.entries()[0].fields;
    assert_eq!(fields[0].value, "первая строка\nвторая строка");
    assert_eq!(fields[1], field("login", "l"));
}

#[test]
fn heading_with_child_headings_is_a_section_not_an_entry() {
    let doc = Document::parse("# work\nтекст\n## server\n### a\nk: v\n");
    let entries = doc.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, ["work", "server"]);
}

#[test]
fn adding_an_entry_creates_missing_sections() {
    let mut doc = Document::parse("");
    doc.add(draft(&["work", "server"], "alpha", vec![field("ssh", "h"), field("password", "p")], &["prod"])).unwrap();
    assert_eq!(doc.to_markdown(), "# work\n\n## server\n\n### alpha\nssh: h\npassword: p\ntags: [prod]\n");
}

#[test]
fn adding_to_an_existing_section_appends_after_its_last_entry() {
    let mut doc = Document::parse(SAMPLE);
    doc.add(draft(&["work", "server"], "gamma", vec![field("ssh", "g")], &[])).unwrap();
    let titles: Vec<_> = doc.entries().iter().map(|e| e.title.clone()).collect();
    assert_eq!(titles, ["alpha (стейдж)", "beta", "gamma", "example", "почта"]);
    let md = doc.to_markdown();
    assert!(md.contains("note: старая таблица orders_tmp не нужна\n\n### gamma\nssh: g\n\n## site\n"), "{md}");
}

#[test]
fn adding_uses_the_line_endings_of_the_file() {
    let mut doc = Document::parse("# a\r\n\r\n### x\r\nk: v\r\n");
    doc.add(draft(&["a"], "y", vec![field("k", "w")], &[])).unwrap();
    assert_eq!(doc.to_markdown(), "# a\r\n\r\n### x\r\nk: v\r\n\r\n### y\r\nk: w\r\n");
}

#[test]
fn adding_to_a_file_without_final_newline_starts_on_a_new_line() {
    let mut doc = Document::parse("### x\nk: v");
    doc.add(draft(&[], "y", vec![field("k", "w")], &[])).unwrap();
    assert_eq!(doc.to_markdown(), "### x\nk: v\n\n### y\nk: w\n");
}

#[test]
fn updating_an_entry_rewrites_only_that_entry() {
    let mut doc = Document::parse(SAMPLE);
    let beta = doc.entries()[1].id;
    doc.update(beta, draft(&["work", "server"], "beta", vec![field("port", "6543")], &[])).unwrap();
    let expected = SAMPLE.replace(
        "port: 5432\nuser: app_user\npassword: battery-staple\nnote: старая таблица orders_tmp не нужна\n",
        "port: 6543\n",
    );
    assert_eq!(doc.to_markdown(), expected);
}

#[test]
fn updated_entry_keeps_its_id_and_lines_without_a_key() {
    let mut doc = Document::parse("### a\nk: v\nстрока без ключа\n");
    let id = doc.entries()[0].id;
    doc.update(id, draft(&[], "a", vec![field("k", "w")], &[])).unwrap();
    assert_eq!(doc.entry(id).unwrap().fields, vec![field("k", "w")]);
    assert_eq!(doc.to_markdown(), "### a\nk: w\nстрока без ключа\n");
}

#[test]
fn custom_secret_and_multiline_values_are_written_back() {
    let mut doc = Document::parse("");
    let mut hint = field("pin_hint", "h");
    hint.secret = true;
    let mut plain_password = field("password", "p");
    plain_password.secret = false;
    doc.add(draft(&[], "a", vec![hint, plain_password, field("note", "раз\nдва")], &[])).unwrap();
    let md = doc.to_markdown();
    assert_eq!(md, "### a\npin_hint!: h\npassword: p\nnote: раз\n  два\n");
    // Поле с «секретным» именем всегда секретное: снять признак у него нельзя.
    let fields = Document::parse(&md).entries()[0].fields.clone();
    assert!(fields[0].secret && fields[1].secret && !fields[2].secret);
}

#[test]
fn moving_an_entry_to_another_section_removes_the_emptied_section() {
    let mut doc = Document::parse("# work\n\n## old\n\n### a\nk: v\n\n## keep\n\n### b\nk: v\n");
    let a = doc.entries()[0].id;
    doc.update(a, draft(&["work", "keep"], "a", vec![field("k", "v")], &[])).unwrap();
    assert_eq!(doc.to_markdown(), "# work\n\n## keep\n\n### b\nk: v\n\n### a\nk: v\n");
    assert_eq!(doc.entry(a).unwrap().path, ["work", "keep"]);
}

#[test]
fn removing_an_entry_drops_sections_left_without_entries() {
    let mut doc = Document::parse(SAMPLE);
    let pochta = doc.entries()[3].id;
    doc.remove(pochta).unwrap();
    assert_eq!(doc.to_markdown(), SAMPLE[..SAMPLE.find("# my").unwrap()]);
    assert_eq!(doc.entries().len(), 3);
}

#[test]
fn removing_keeps_a_section_that_has_its_own_text() {
    let mut doc = Document::parse("# work\nважный текст\n\n### a\nk: v\n");
    let a = doc.entries()[0].id;
    doc.remove(a).unwrap();
    assert_eq!(doc.to_markdown(), "# work\nважный текст\n\n");
}

#[test]
fn invalid_drafts_are_rejected() {
    let mut doc = Document::parse("");
    let cases = [
        (draft(&[], "  ", vec![], &[]), FormatError::EmptyTitle),
        (draft(&[], "a\nb", vec![], &[]), FormatError::LineBreakInName),
        (draft(&["", "x"], "a", vec![], &[]), FormatError::EmptySection),
        (draft(&["1", "2", "3", "4", "5", "6"], "a", vec![], &[]), FormatError::TooDeep),
        (draft(&[], "a", vec![field("два слова", "v")], &[]), FormatError::InvalidKey("два слова".into())),
        (draft(&[], "a", vec![field("k:", "v")], &[]), FormatError::InvalidKey("k:".into())),
        (draft(&[], "a", vec![field("tags", "v")], &[]), FormatError::InvalidKey("tags".into())),
        (draft(&[], "a", vec![], &["a,b"]), FormatError::InvalidTag("a,b".into())),
    ];
    for (bad, err) in cases {
        assert_eq!(doc.add(bad), Err(err));
    }
    assert_eq!(doc.to_markdown(), "");
}

#[test]
fn unknown_entry_id_is_an_error() {
    let mut doc = Document::parse("### a\nk: v\n");
    let id = doc.entries()[0].id;
    doc.remove(id).unwrap();
    assert_eq!(doc.remove(id), Err(FormatError::NotFound));
    assert_eq!(doc.update(id, draft(&[], "a", vec![], &[])), Err(FormatError::NotFound));
}

#[test]
fn debug_output_hides_secret_values() {
    let doc = Document::parse("### a\npassword: hunter2\nlogin: admin\n");
    let entries = format!("{:?}", doc.entries());
    assert!(!entries.contains("hunter2") && entries.contains("admin"), "{entries}");
    assert!(!format!("{doc:?}").contains("hunter2"));
}
