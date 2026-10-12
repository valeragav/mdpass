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

fn titles(doc: &Document) -> Vec<String> {
    doc.entries().into_iter().map(|e| e.title).collect()
}

const SAMPLE: &str = "\
# work

## server

**alpha (стейдж)**
ssh: deploy@203.0.113.10
password: correct-horse

**beta**
port: 5432
user: app_user
password: battery-staple
note: старая таблица orders_tmp не нужна

## site

**example**
url: https://example.test/
url_admin:  https://example.test/admin
login: admin
password: hunter2
tags: [server], [work], [prod]

# my

**почта**
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
        "# work\nтекст раздела\n\n**a**\nssh:host-without-space\n   \nстрока без ключа\n\n\n# b\n".to_string(),
        "# 1\n## 2\n### 3\n#### 4\n##### 5\n###### 6\n**глубоко**\nk: v\n".to_string(),
    ];
    for input in inputs {
        assert_eq!(Document::parse(&input).to_markdown(), input);
    }
}

#[test]
fn entries_get_section_path_title_and_fields() {
    let doc = Document::parse(SAMPLE);
    let entries = doc.entries();
    assert_eq!(titles(&doc), ["alpha (стейдж)", "beta", "example", "почта"]);

    let alpha = &entries[0];
    assert_eq!(alpha.path, ["work", "server"]);
    assert_eq!(alpha.fields, vec![field("ssh", "deploy@203.0.113.10"), field("password", "correct-horse")]);

    assert_eq!(entries[3].path, ["my"]);
}

#[test]
fn every_heading_is_a_section_and_entries_belong_to_the_last_open_one() {
    let doc = Document::parse("**top**\nk: v\n# a\n## b\n**x**\nk: v\n# c\n**y**\nk: v\n# пустой\n");
    let places: Vec<_> = doc.entries().into_iter().map(|e| (e.title, e.path)).collect();
    assert_eq!(
        places,
        vec![
            ("top".to_string(), vec![]),
            ("x".to_string(), vec!["a".to_string(), "b".to_string()]),
            ("y".to_string(), vec!["c".to_string()]),
        ]
    );
}

#[test]
fn sections_go_six_levels_deep() {
    let doc = Document::parse("# 1\n## 2\n### 3\n#### 4\n##### 5\n###### 6\n**глубоко**\nk: v\n");
    assert_eq!(doc.entries()[0].path, ["1", "2", "3", "4", "5", "6"]);
}

#[test]
fn only_a_whole_bold_line_at_the_start_is_an_entry_title() {
    let doc = Document::parse("**a**\nnote: x\n  **не запись**\nтекст **жирный** внутри\n**b** и хвост\n");
    let entries = doc.entries();
    assert_eq!(titles(&doc), ["a"]);
    assert_eq!(entries[0].fields[0].value, "x\n**не запись**");
    assert_eq!(entries[0].extra, ["текст **жирный** внутри", "**b** и хвост"]);
}

#[test]
fn value_starts_after_colon_and_spaces() {
    let doc = Document::parse("**a**\nssh:host\nurl_admin:  https://x.test:8080/a\n");
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
    let doc = Document::parse("**a**\npassword: p\napi_token: t\nPIN: 1\npin_hint!: h\nlogin: l\n");
    let entries = doc.entries();
    let secret: Vec<_> = entries[0].fields.iter().map(|f| (f.key.as_str(), f.secret)).collect();
    assert_eq!(secret, [("password", true), ("api_token", true), ("PIN", true), ("pin_hint", true), ("login", false)]);
}

#[test]
fn indented_lines_continue_the_previous_value() {
    let doc = Document::parse("**a**\nnote: первая строка\n  вторая строка\nlogin: l\n");
    let fields = &doc.entries()[0].fields;
    assert_eq!(fields[0].value, "первая строка\nвторая строка");
    assert_eq!(fields[1], field("login", "l"));
}

#[test]
fn adding_an_entry_creates_missing_sections() {
    let mut doc = Document::parse("");
    doc.add(draft(&["work", "server"], "alpha", vec![field("ssh", "h"), field("password", "p")], &["prod"])).unwrap();
    assert_eq!(doc.to_markdown(), "# work\n\n## server\n\n**alpha**\nssh: h\npassword: p\ntags: [prod]\n");
}

#[test]
fn adding_six_levels_deep_is_allowed() {
    let mut doc = Document::parse("");
    doc.add(draft(&["1", "2", "3", "4", "5", "6"], "глубоко", vec![field("k", "v")], &[])).unwrap();
    assert_eq!(doc.to_markdown(), "# 1\n\n## 2\n\n### 3\n\n#### 4\n\n##### 5\n\n###### 6\n\n**глубоко**\nk: v\n");
}

#[test]
fn adding_to_an_existing_section_appends_after_its_last_entry() {
    let mut doc = Document::parse(SAMPLE);
    doc.add(draft(&["work", "server"], "gamma", vec![field("ssh", "g")], &[])).unwrap();
    assert_eq!(titles(&doc), ["alpha (стейдж)", "beta", "gamma", "example", "почта"]);
    let md = doc.to_markdown();
    assert!(md.contains("note: старая таблица orders_tmp не нужна\n\n**gamma**\nssh: g\n\n## site\n"), "{md}");
}

#[test]
fn entry_of_a_section_goes_before_its_subsections() {
    let mut doc = Document::parse("# work\n\n## server\n\n**a**\nk: v\n");
    doc.add(draft(&["work"], "своя", vec![field("k", "v")], &[])).unwrap();
    assert_eq!(doc.entry(doc.entries()[0].id).unwrap().path, ["work"]);
    assert_eq!(doc.to_markdown(), "# work\n\n**своя**\nk: v\n\n## server\n\n**a**\nk: v\n");
}

#[test]
fn top_level_entry_goes_before_the_first_heading() {
    let mut doc = Document::parse("# work\n\n**a**\nk: v\n");
    doc.add(draft(&[], "верх", vec![field("k", "v")], &[])).unwrap();
    assert_eq!(doc.to_markdown(), "**верх**\nk: v\n\n# work\n\n**a**\nk: v\n");
}

#[test]
fn adding_uses_the_line_endings_of_the_file() {
    let mut doc = Document::parse("# a\r\n\r\n**x**\r\nk: v\r\n");
    doc.add(draft(&["a"], "y", vec![field("k", "w")], &[])).unwrap();
    assert_eq!(doc.to_markdown(), "# a\r\n\r\n**x**\r\nk: v\r\n\r\n**y**\r\nk: w\r\n");
}

#[test]
fn adding_to_a_file_without_final_newline_starts_on_a_new_line() {
    let mut doc = Document::parse("**x**\nk: v");
    doc.add(draft(&[], "y", vec![field("k", "w")], &[])).unwrap();
    assert_eq!(doc.to_markdown(), "**x**\nk: v\n\n**y**\nk: w\n");
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
    let mut doc = Document::parse("**a**\nk: v\nстрока без ключа\n");
    let id = doc.entries()[0].id;
    doc.update(id, draft(&[], "a", vec![field("k", "w")], &[])).unwrap();
    assert_eq!(doc.entry(id).unwrap().fields, vec![field("k", "w")]);
    assert_eq!(doc.to_markdown(), "**a**\nk: w\nстрока без ключа\n");
}

#[test]
fn custom_secret_and_multiline_values_are_written_back() {
    let mut doc = Document::parse("");
    let mut hint = field("pin_hint", "h");
    hint.secret = true;
    let mut plain_password = field("password", "p");
    plain_password.secret = false;
    doc.add(draft(&[], "a", vec![hint, plain_password, field("note", "раз\n**два**")], &[])).unwrap();
    let md = doc.to_markdown();
    assert_eq!(md, "**a**\npin_hint!: h\npassword: p\nnote: раз\n  **два**\n");
    // Поле с «секретным» именем всегда секретное: снять признак у него нельзя.
    let reread = Document::parse(&md);
    assert_eq!(reread.entries().len(), 1, "жирная строка внутри значения не становится записью");
    let fields = reread.entries()[0].fields.clone();
    assert!(fields[0].secret && fields[1].secret && !fields[2].secret);
}

#[test]
fn moving_an_entry_to_another_section_removes_the_emptied_section() {
    let mut doc = Document::parse("# work\n\n## old\n\n**a**\nk: v\n\n## keep\n\n**b**\nk: v\n");
    let a = doc.entries()[0].id;
    doc.update(a, draft(&["work", "keep"], "a", vec![field("k", "v")], &[])).unwrap();
    assert_eq!(doc.to_markdown(), "# work\n\n## keep\n\n**b**\nk: v\n\n**a**\nk: v\n");
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
fn removing_keeps_other_empty_sections() {
    let mut doc = Document::parse("# пустой\n\n# work\n\n**a**\nk: v\n");
    let a = doc.entries()[0].id;
    doc.remove(a).unwrap();
    assert_eq!(doc.to_markdown(), "# пустой\n\n");
}

#[test]
fn removing_keeps_a_section_that_has_its_own_text() {
    let mut doc = Document::parse("# work\nважный текст\n\n**a**\nk: v\n");
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
        (draft(&[], "*звезда", vec![], &[]), FormatError::InvalidTitle),
        (draft(&["", "x"], "a", vec![], &[]), FormatError::EmptySection),
        (draft(&["1", "2", "3", "4", "5", "6", "7"], "a", vec![], &[]), FormatError::TooDeep),
        (draft(&[], "a", vec![field("два слова", "v")], &[]), FormatError::InvalidKey("два слова".into())),
        (draft(&[], "a", vec![field("k:", "v")], &[]), FormatError::InvalidKey("k:".into())),
        (draft(&[], "a", vec![field("tags", "v")], &[]), FormatError::InvalidKey("tags".into())),
        (draft(&[], "a", vec![field("**k", "v")], &[]), FormatError::InvalidKey("**k".into())),
        (draft(&[], "a", vec![], &["a,b"]), FormatError::InvalidTag("a,b".into())),
    ];
    for (bad, err) in cases {
        assert_eq!(doc.add(bad), Err(err));
    }
    assert_eq!(doc.to_markdown(), "");
}

#[test]
fn unknown_entry_id_is_an_error() {
    let mut doc = Document::parse("**a**\nk: v\n");
    let id = doc.entries()[0].id;
    doc.remove(id).unwrap();
    assert_eq!(doc.remove(id), Err(FormatError::NotFound));
    assert_eq!(doc.update(id, draft(&[], "a", vec![], &[])), Err(FormatError::NotFound));
}

#[test]
fn debug_output_hides_secret_values() {
    let doc = Document::parse("**a**\npassword: hunter2\nlogin: admin\n");
    let entries = format!("{:?}", doc.entries());
    assert!(!entries.contains("hunter2") && entries.contains("admin"), "{entries}");
    assert!(!format!("{doc:?}").contains("hunter2"));
}

#[test]
fn format_header_is_read_and_added_once() {
    let doc = Document::parse("<!-- mdpass:1 -->\n\n# work\n");
    assert_eq!(doc.format_version(), Some(1));
    assert_eq!(Document::parse("# work\n").format_version(), None);

    let mut empty = Document::default();
    empty.ensure_header();
    empty.ensure_header();
    assert_eq!(empty.to_markdown(), "<!-- mdpass:1 -->\n");
    assert!(empty.is_blank());
    empty.add(draft(&["work"], "a", vec![field("k", "v")], &[])).unwrap();
    assert_eq!(empty.to_markdown(), "<!-- mdpass:1 -->\n\n# work\n\n**a**\nk: v\n");
    assert!(!empty.is_blank());

    let mut old = Document::parse("**a**\nk: v\n");
    old.ensure_header();
    assert_eq!(old.to_markdown(), "<!-- mdpass:1 -->\n\n**a**\nk: v\n");
    assert_eq!(old.entries().len(), 1);
}
