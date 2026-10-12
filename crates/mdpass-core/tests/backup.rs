use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mdpass_core::backup::{BackupKind, BackupStore, restore_damaged};
use mdpass_core::diff::{Change, diff};
use mdpass_core::format::{Document, EntryDraft, Field};
use mdpass_core::vault::{Passphrase, Vault, VaultError, VaultOptions};
use tempfile::TempDir;

const FAST: VaultOptions = VaultOptions { work_factor: 4 };

fn pass(s: &str) -> Passphrase {
    Passphrase::from(s.to_string())
}

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_791_700_000 + seconds)
}

fn draft(title: &str, password: &str) -> EntryDraft {
    EntryDraft {
        path: vec!["work".into()],
        title: title.into(),
        fields: vec![Field::new("login", "admin"), Field::new("password", password)],
        tags: vec![],
    }
}

#[test]
fn backups_live_in_a_hidden_folder_next_to_the_vault() {
    let dir = TempDir::new().unwrap();
    let store = BackupStore::for_vault(&dir.path().join("vault.md.age"));
    assert_eq!(store.dir(), dir.path().join(".vault.md.age.backups"));
}

#[test]
fn saved_backups_are_listed_newest_first_with_kind_and_time() {
    let dir = TempDir::new().unwrap();
    let store = BackupStore::for_vault(&dir.path().join("vault.md.age"));
    assert!(store.list().unwrap().is_empty());

    store.save(b"one", BackupKind::Add, at(0)).unwrap();
    store.save(b"two", BackupKind::Delete, at(60)).unwrap();
    let list = store.list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].kind, BackupKind::Delete);
    assert_eq!(list[0].created, at(60));
    assert_eq!(store.read(&list[0].id).unwrap(), b"two");
    assert_eq!(store.read(&list[1].id).unwrap(), b"one");
}

#[test]
fn backups_in_the_same_millisecond_do_not_overwrite_each_other() {
    let dir = TempDir::new().unwrap();
    let store = BackupStore::for_vault(&dir.path().join("vault.md.age"));
    store.save(b"a", BackupKind::Update, at(0)).unwrap();
    store.save(b"b", BackupKind::Update, at(0)).unwrap();
    assert_eq!(store.list().unwrap().len(), 2);
}

#[test]
fn prune_keeps_only_the_newest_backups() {
    let dir = TempDir::new().unwrap();
    let store = BackupStore::for_vault(&dir.path().join("vault.md.age"));
    for i in 0..5 {
        store.save(format!("{i}").as_bytes(), BackupKind::Update, at(i)).unwrap();
    }
    store.prune(2).unwrap();
    let list = store.list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(store.read(&list[0].id).unwrap(), b"4");
    assert_eq!(store.read(&list[1].id).unwrap(), b"3");
}

#[test]
fn foreign_files_and_path_tricks_are_ignored() {
    let dir = TempDir::new().unwrap();
    let store = BackupStore::for_vault(&dir.path().join("vault.md.age"));
    store.save(b"a", BackupKind::Update, at(0)).unwrap();
    fs::write(store.dir().join("заметки.txt"), "чужой файл").unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
    for bad in ["../vault.md.age", "заметки.txt", "", "a/b.age"] {
        assert!(store.read(bad).is_err(), "{bad}");
    }
}

#[test]
fn saving_the_vault_backs_up_the_previous_file_first() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md.age");
    let store = BackupStore::for_vault(&path);
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();
    let empty_file = fs::read(&path).unwrap();

    vault.document_mut().add(draft("alpha", "1")).unwrap();
    vault.save_with_backup(|old| store.save(old, BackupKind::Add, at(0)).map(drop)).unwrap();

    let list = store.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(store.read(&list[0].id).unwrap(), empty_file);
}

#[test]
fn failed_backup_stops_the_write() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md.age");
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();
    let before = fs::read(&path).unwrap();
    vault.document_mut().add(draft("alpha", "1")).unwrap();
    let result = vault.save_with_backup(|_| Err(std::io::Error::other("диск полон")));
    assert!(matches!(result, Err(VaultError::Io(_))));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn restoring_a_backup_brings_back_its_entries() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md.age");
    let store = BackupStore::for_vault(&path);
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();
    vault.document_mut().add(draft("alpha", "1")).unwrap();
    vault.save().unwrap();
    let with_alpha = fs::read(&path).unwrap();

    let id = vault.document().entries()[0].id;
    vault.document_mut().remove(id).unwrap();
    vault.save().unwrap();

    let restored = vault.decrypt_backup(&with_alpha).unwrap();
    vault.restore(restored, |old| store.save(old, BackupKind::Restore, at(0)).map(drop)).unwrap();
    assert_eq!(vault.document().entries()[0].title, "alpha");
    assert_eq!(Vault::open(&path, pass("p")).unwrap().document().entries().len(), 1);
    assert_eq!(store.list().unwrap()[0].kind, BackupKind::Restore);
}

#[test]
fn backup_under_another_passphrase_is_reported() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md.age");
    let mut vault = Vault::create(&path, pass("старый"), FAST).unwrap();
    let old_file = fs::read(&path).unwrap();
    vault.change_passphrase(pass("новый-пароль"), FAST).unwrap();
    assert!(matches!(vault.decrypt_backup(&old_file), Err(VaultError::WrongPassphrase)));
}

#[test]
fn diff_tells_what_restoring_would_change_without_secret_values() {
    let mut current = Document::default();
    current.add(draft("alpha", "new-secret")).unwrap();
    current.add(draft("beta", "b")).unwrap();
    let mut backup = Document::default();
    backup.add(draft("alpha", "old-secret")).unwrap();
    backup.add(draft("gamma", "g")).unwrap();

    let changes = diff(&current, &backup);
    assert_eq!(
        changes,
        vec![
            Change::Changed { title: "alpha".into(), keys: vec!["password".into()] },
            Change::Removed { title: "beta".into() },
            Change::Added { title: "gamma".into() },
        ]
    );
    assert!(!format!("{changes:?}").contains("secret"));
}

#[test]
fn diff_of_equal_documents_is_empty() {
    let mut doc = Document::default();
    doc.add(draft("alpha", "1")).unwrap();
    assert!(diff(&doc, &doc.clone()).is_empty());
}

#[test]
fn damaged_vault_is_restored_from_a_backup_and_kept_aside() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md.age");
    let store = BackupStore::for_vault(&path);
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();
    vault.document_mut().add(draft("alpha", "1")).unwrap();
    vault.save().unwrap();
    let good = fs::read(&path).unwrap();
    let backup = store.save(&good, BackupKind::Update, at(0)).unwrap();
    drop(vault);
    fs::write(&path, b"broken").unwrap();

    assert!(matches!(
        restore_damaged(&path, &store, &backup.id, Some(pass("wrong")), at(1)),
        Err(VaultError::WrongPassphrase)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"broken", "неверный пароль ничего не меняет");

    let restored = restore_damaged(&path, &store, &backup.id, Some(pass("p")), at(1)).unwrap();
    assert_eq!(restored.document().entries()[0].title, "alpha");
    assert_eq!(fs::read(&path).unwrap(), good);
    let aside: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("vault.md.age.damaged-"))
        .collect();
    assert_eq!(aside.len(), 1);
    assert_eq!(fs::read(dir.path().join(&aside[0])).unwrap(), b"broken");
}

#[test]
fn restoring_an_open_backup_keeps_the_vault_encrypted() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md");
    let mut vault = Vault::create_plain(&path).unwrap();
    vault.document_mut().add(draft("alpha", "1")).unwrap();
    vault.save().unwrap();
    let open_copy = fs::read(&path).unwrap();
    vault.change_passphrase(pass("теперь-с-паролем"), FAST).unwrap();

    let document = vault.decrypt_backup(&open_copy).unwrap();
    vault.restore(document, |_| Ok(())).unwrap();
    assert!(mdpass_core::vault::file_is_encrypted(&path).unwrap());
    assert_eq!(Vault::open(&path, pass("теперь-с-паролем")).unwrap().document().entries().len(), 1);
}

#[test]
fn damaged_open_vault_is_restored_without_a_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("vault.md");
    let store = BackupStore::for_vault(&path);
    let mut vault = Vault::create_plain(&path).unwrap();
    vault.document_mut().add(draft("alpha", "1")).unwrap();
    vault.save().unwrap();
    let backup = store.save(&fs::read(&path).unwrap(), BackupKind::Update, at(0)).unwrap();
    drop(vault);
    fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();

    let restored = restore_damaged(&path, &store, &backup.id, None, at(1)).unwrap();
    assert!(!restored.is_encrypted());
    assert_eq!(restored.document().entries()[0].title, "alpha");
}

#[test]
fn backups_follow_a_renamed_vault() {
    let dir = TempDir::new().unwrap();
    let old = BackupStore::for_vault(&dir.path().join("vault.md"));
    old.save(b"a", BackupKind::Update, at(0)).unwrap();
    let moved = old.move_to(&dir.path().join("vault.md.age")).unwrap();
    assert_eq!(moved.dir(), dir.path().join(".vault.md.age.backups"));
    assert_eq!(moved.list().unwrap().len(), 1);
    assert!(!old.dir().exists());
}
