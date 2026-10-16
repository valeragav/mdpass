use std::fs;

use mdpass_core::format::{Document, EntryDraft, Favorite, Field};
use mdpass_core::sync::{MAX_PARENTS, restamp, stamp};
use mdpass_core::vault::Vault;
use tempfile::TempDir;

fn entry(title: &str) -> EntryDraft {
    EntryDraft {
        path: vec!["work".into()],
        title: title.into(),
        fields: vec![Field::new("login", "admin")],
        tags: vec![],
        favorite: Favorite::No,
    }
}

#[test]
fn stamp_goes_right_after_the_format_header() {
    let mut doc = Document::parse("<!-- mdpass:1 -->\n\n**a**\nk: v\n");
    let first = restamp(&mut doc, &[]);
    assert!(first.parents.is_empty());
    let text = doc.to_markdown();
    assert_eq!(
        text,
        format!("<!-- mdpass:1 -->\n<!-- mdpass-sync rev={} at={} -->\n\n**a**\nk: v\n", first.rev, first.at.unwrap())
    );
    assert_eq!(stamp(&Document::parse(&text)), Some(first));
}

#[test]
fn file_without_header_gets_both_lines() {
    let mut doc = Document::parse("**a**\nk: v\n");
    let s = restamp(&mut doc, &[]);
    assert_eq!(
        doc.to_markdown(),
        format!("<!-- mdpass:1 -->\n<!-- mdpass-sync rev={} at={} -->\n\n**a**\nk: v\n", s.rev, s.at.unwrap())
    );
}

#[test]
fn crlf_file_keeps_its_line_ends() {
    let mut doc = Document::parse("<!-- mdpass:1 -->\r\n\r\n**a**\r\nk: v\r\n");
    let s = restamp(&mut doc, &[]);
    let header = format!("<!-- mdpass:1 -->\r\n<!-- mdpass-sync rev={} at={} -->\r\n\r\n", s.rev, s.at.unwrap());
    assert!(doc.to_markdown().starts_with(&header));
}

#[test]
fn each_restamp_remembers_the_previous_versions() {
    let mut doc = Document::default();
    let first = restamp(&mut doc, &[]);
    let second = restamp(&mut doc, &[]);
    let third = restamp(&mut doc, &[]);
    assert_ne!(first.rev, second.rev);
    assert_eq!(third.parents, [second.rev.clone(), first.rev.clone()]);
    // Строка одна: прежняя заменяется.
    assert_eq!(doc.to_markdown().matches("mdpass-sync").count(), 1);
    assert_eq!(stamp(&doc), Some(third));
}

#[test]
fn history_is_capped() {
    let mut doc = Document::default();
    for _ in 0..MAX_PARENTS + 5 {
        restamp(&mut doc, &[]);
    }
    assert_eq!(stamp(&doc).unwrap().parents.len(), MAX_PARENTS);
}

#[test]
fn merge_keeps_history_of_both_sides() {
    let mut base = Document::default();
    let root = restamp(&mut base, &[]);
    let (mut mine, mut theirs) = (base.clone(), base.clone());
    let my = restamp(&mut mine, &[]);
    let their = restamp(&mut theirs, &[]);
    let merged = restamp(&mut theirs, &[&my]);
    assert_eq!(merged.parents, [their.rev, my.rev, root.rev]);
}

#[test]
fn broken_stamp_is_ignored() {
    for line in ["<!-- mdpass-sync -->", "<!-- mdpass-sync rev=not-hex -->", "<!-- mdpass-sync rev=ab"] {
        let doc = Document::parse(&format!("<!-- mdpass:1 -->\n{line}\n"));
        assert_eq!(stamp(&doc), None, "{line}");
    }
    // Строка не во второй строке файла — не метка.
    let doc = Document::parse("<!-- mdpass:1 -->\n\n<!-- mdpass-sync rev=ab -->\n");
    assert_eq!(stamp(&doc), None);
}

#[test]
fn stamped_empty_vault_is_blank() {
    let mut doc = Document::default();
    restamp(&mut doc, &[]);
    assert!(doc.is_blank());
    doc.add(entry("a")).unwrap();
    assert!(!doc.is_blank());
}

#[test]
fn every_vault_write_is_a_new_version() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md");
    let mut vault = Vault::create_plain(&path).unwrap();
    let created = stamp(vault.document()).unwrap();

    vault.document_mut().add(entry("a")).unwrap();
    vault.save().unwrap();
    let saved = stamp(vault.document()).unwrap();
    assert_eq!(saved.parents, std::slice::from_ref(&created.rev));

    // То, что в памяти, совпадает с файлом.
    let on_disk = Document::parse(&fs::read_to_string(&path).unwrap());
    assert_eq!(stamp(&on_disk), Some(saved));
}

#[test]
fn failed_write_leaves_no_version_behind() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md");
    let mut vault = Vault::create_plain(&path).unwrap();
    let created = stamp(vault.document()).unwrap();

    // Файл удалили: записать нельзя.
    fs::remove_file(&path).unwrap();
    vault.document_mut().add(entry("a")).unwrap();
    assert!(vault.save().is_err());
    assert_eq!(stamp(vault.document()), Some(created));
}

#[test]
fn merged_write_remembers_both_histories() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md");
    let mut vault = Vault::create_plain(&path).unwrap();
    let created = stamp(vault.document()).unwrap();

    let mut other = Vault::open_plain(&path).unwrap();
    other.document_mut().add(entry("a")).unwrap();
    other.save().unwrap();
    let theirs = stamp(other.document()).unwrap();

    vault.document_mut().add(entry("b")).unwrap();
    assert!(vault.save().unwrap().merged.is_some());
    let merged = stamp(vault.document()).unwrap();
    assert_eq!(merged.parents, [theirs.rev, created.rev]);
}

#[test]
fn old_app_keeps_the_stamp_line_untouched() {
    // Старая версия mdpass не знает строку и выводит её как обычный текст.
    let text = "<!-- mdpass:1 -->\n<!-- mdpass-sync rev=ab12 -->\n\n**a**\nk: v\n";
    let mut doc = Document::parse(text);
    assert_eq!(doc.to_markdown(), text);
    let id = doc.entries()[0].id;
    doc.update(id, entry("b")).unwrap();
    assert!(doc.to_markdown().starts_with("<!-- mdpass:1 -->\n<!-- mdpass-sync rev=ab12 -->\n"));
}

#[test]
fn device_name_and_time_round_trip() {
    use mdpass_core::sync::restamp_as;
    let mut doc = Document::default();
    let written = restamp_as(&mut doc, &[], Some("Рабочий ПК-2 <-->"));
    let line = doc.to_markdown().lines().nth(1).unwrap().to_string();
    // В строке нет пробелов и «-->» из имени: они бы её сломали.
    assert!(!line.contains("ПК-2") && !line.contains("<-->"), "{line}");
    let read = stamp(&Document::parse(&doc.to_markdown())).unwrap();
    assert_eq!(read.device.as_deref(), Some("Рабочий ПК-2 <-->"));
    assert_eq!(read, written);
    assert!(read.at.unwrap() > 1_700_000_000);
}

#[test]
fn device_name_is_written_only_into_an_encrypted_vault() {
    use mdpass_core::sync::set_device_name;
    use mdpass_core::vault::VaultOptions;
    set_device_name(Some("Ноутбук".into()));
    let dir = TempDir::new().unwrap();

    let plain = Vault::create_plain(&dir.path().join("plain.md")).unwrap();
    assert_eq!(stamp(plain.document()).unwrap().device, None);

    let encrypted = Vault::create(
        &dir.path().join("vault.md.age"),
        mdpass_core::vault::Passphrase::from("p".to_string()),
        VaultOptions { work_factor: 4 },
    )
    .unwrap();
    assert_eq!(stamp(encrypted.document()).unwrap().device.as_deref(), Some("Ноутбук"));
    set_device_name(None);
}
