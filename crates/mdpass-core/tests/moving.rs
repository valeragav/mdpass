use mdpass_core::format::{Document, FormatError};

const SAMPLE: &str = "\
# work

## server

### a
k: 1

### b
k: 2

## site

### c
k: 3

# my

### d
k: 4
";

fn titles(doc: &Document) -> Vec<String> {
    doc.entries().into_iter().map(|e| e.title).collect()
}

fn id(doc: &Document, title: &str) -> mdpass_core::format::EntryId {
    doc.entries().into_iter().find(|e| e.title == title).unwrap().id
}

fn path(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

#[test]
fn entry_moves_before_another_in_the_same_section() {
    let mut doc = Document::parse(SAMPLE);
    let (a, b) = (id(&doc, "a"), id(&doc, "b"));
    doc.move_entry(b, path(&["work", "server"]), Some(a)).unwrap();
    assert_eq!(titles(&doc), ["b", "a", "c", "d"]);
    assert_eq!(doc.to_markdown(), SAMPLE.replace("### a\nk: 1\n\n### b\nk: 2\n", "### b\nk: 2\n\n### a\nk: 1\n"));
}

#[test]
fn entry_moves_into_another_section_and_the_empty_one_goes() {
    let mut doc = Document::parse(SAMPLE);
    let c = id(&doc, "c");
    doc.move_entry(c, path(&["work", "server"]), None).unwrap();
    assert_eq!(titles(&doc), ["a", "b", "c", "d"]);
    assert_eq!(doc.entry(c).unwrap().path, ["work", "server"]);
    assert_eq!(doc.to_markdown(), SAMPLE.replace("## site\n\n", ""));
}

#[test]
fn entry_moves_up_to_the_top_level() {
    let mut doc = Document::parse(SAMPLE);
    let d = id(&doc, "d");
    doc.move_entry(d, Vec::new(), None).unwrap();
    assert_eq!(titles(&doc), ["d", "a", "b", "c"]);
    assert!(doc.entry(d).unwrap().path.is_empty());
    let md = doc.to_markdown();
    assert!(md.starts_with("### d\nk: 4\n\n# work\n"), "{md}");
    assert!(!md.contains("# my"), "{md}");
    assert_eq!(Document::parse(&md).entries().len(), 4);
}

#[test]
fn entry_moves_into_a_new_section() {
    let mut doc = Document::parse(SAMPLE);
    let a = id(&doc, "a");
    doc.move_entry(a, path(&["archive"]), None).unwrap();
    assert_eq!(doc.entry(a).unwrap().path, ["archive"]);
    assert!(doc.to_markdown().ends_with("k: 4\n\n# archive\n\n### a\nk: 1\n"));
}

#[test]
fn moving_an_entry_before_itself_changes_nothing() {
    let mut doc = Document::parse(SAMPLE);
    let a = id(&doc, "a");
    doc.move_entry(a, path(&["work", "server"]), Some(a)).unwrap();
    assert_eq!(doc.to_markdown(), SAMPLE);
}

#[test]
fn section_moves_up_a_level_with_its_entries() {
    let mut doc = Document::parse(SAMPLE);
    doc.move_section(&path(&["work", "site"]), &[], None).unwrap();
    let c = id(&doc, "c");
    assert_eq!(doc.entry(c).unwrap().path, ["site"]);
    assert!(doc.to_markdown().ends_with("### d\nk: 4\n\n# site\n\n### c\nk: 3\n"), "{}", doc.to_markdown());
}

#[test]
fn section_moves_into_another_section() {
    let mut doc = Document::parse(SAMPLE);
    doc.move_section(&path(&["my"]), &path(&["work"]), None).unwrap();
    let d = id(&doc, "d");
    assert_eq!(doc.entry(d).unwrap().path, ["work", "my"]);
    assert!(doc.to_markdown().ends_with("### c\nk: 3\n\n## my\n\n#### d\nk: 4\n"), "{}", doc.to_markdown());
    assert_eq!(titles(&Document::parse(&doc.to_markdown())), ["a", "b", "c", "d"]);
}

#[test]
fn section_moves_before_a_sibling() {
    let mut doc = Document::parse(SAMPLE);
    doc.move_section(&path(&["my"]), &[], Some(&path(&["work"]))).unwrap();
    assert_eq!(titles(&doc), ["d", "a", "b", "c"]);
    assert!(doc.to_markdown().starts_with("# my\n\n### d\nk: 4\n\n# work\n"), "{}", doc.to_markdown());
}

#[test]
fn entries_stay_at_least_at_the_entry_level_when_a_section_goes_up() {
    let mut doc = Document::parse("# a\n\n## b\n\n### e\nk: v\n");
    doc.move_section(&path(&["a", "b"]), &[], None).unwrap();
    assert_eq!(doc.to_markdown(), "# b\n\n### e\nk: v\n");
}

#[test]
fn section_cannot_move_into_itself() {
    let mut doc = Document::parse(SAMPLE);
    for target in [path(&["work"]), path(&["work", "server"])] {
        assert_eq!(doc.move_section(&path(&["work"]), &target, None), Err(FormatError::IntoItself));
    }
    assert_eq!(doc.to_markdown(), SAMPLE);
}

#[test]
fn section_cannot_land_next_to_one_with_the_same_name() {
    let source = "# a\n\n## x\n\n### e\nk: v\n\n# b\n\n## x\n\n### f\nk: v\n";
    let mut doc = Document::parse(source);
    assert_eq!(doc.move_section(&path(&["b", "x"]), &path(&["a"]), None), Err(FormatError::SectionExists("x".into())));
    assert_eq!(doc.to_markdown(), source);
}

#[test]
fn unknown_section_or_too_deep_target_is_rejected() {
    let mut doc = Document::parse(SAMPLE);
    assert_eq!(doc.move_section(&path(&["nope"]), &[], None), Err(FormatError::NotFound));

    let mut deep = Document::parse("# 1\n## 2\n### 3\n#### 4\n##### e\nk: v\n# top\n## s\n### f\nk: v\n");
    assert_eq!(deep.move_section(&path(&["top"]), &path(&["1", "2", "3", "4"]), None), Err(FormatError::TooDeep));
}
