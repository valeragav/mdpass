use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mdpass_core::format::{EntryDraft, Field};
use mdpass_core::vault::{AttemptLimiter, Passphrase, Vault, VaultError, VaultOptions};
use tempfile::TempDir;

// Слабый scrypt, чтобы тесты шли быстро. В приложении стоит значение по умолчанию.
const FAST: VaultOptions = VaultOptions { work_factor: 4 };

fn pass(s: &str) -> Passphrase {
    Passphrase::from(s.to_string())
}

fn vault_path(dir: &TempDir) -> PathBuf {
    dir.path().join("vault.md.age")
}

fn sample_draft() -> EntryDraft {
    EntryDraft {
        path: vec!["work".into()],
        title: "alpha".into(),
        fields: vec![Field::new("ssh", "h"), Field::new("password", "p")],
        tags: vec![],
    }
}

fn files_in(dir: &TempDir) -> Vec<String> {
    let mut names: Vec<_> =
        fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn created_vault_opens_with_the_same_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    let mut vault = Vault::create(&path, pass("верный"), FAST).unwrap();
    vault.document_mut().add(sample_draft()).unwrap();
    vault.save().unwrap();

    let reopened = Vault::open(&path, pass("верный")).unwrap();
    assert_eq!(reopened.document().to_markdown(), vault.document().to_markdown());
    assert_eq!(reopened.path(), path);
}

#[test]
fn new_vault_starts_empty_and_is_written_at_once() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    Vault::create(&path, pass("p"), FAST).unwrap();
    let opened = Vault::open(&path, pass("p")).unwrap();
    assert!(opened.document().entries().is_empty());
}

#[test]
fn file_is_plain_age_and_holds_no_plaintext() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();
    vault.document_mut().add(sample_draft()).unwrap();
    vault.save().unwrap();

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(b"age-encryption.org/v1\n"));
    assert!(!String::from_utf8_lossy(&bytes).contains("alpha"));

    // Файл расшифровывается стандартным age без mdpass.
    let identity = age::scrypt::Identity::new(age::secrecy::SecretString::from("p".to_string()));
    let plain = age::decrypt(&identity, &bytes).unwrap();
    assert_eq!(String::from_utf8(plain).unwrap(), vault.document().to_markdown());
}

#[test]
fn create_refuses_to_overwrite_an_existing_file() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    fs::write(&path, "чужой файл").unwrap();
    assert!(matches!(Vault::create(&path, pass("p"), FAST), Err(VaultError::AlreadyExists)));
    assert_eq!(fs::read_to_string(&path).unwrap(), "чужой файл");
}

#[test]
fn wrong_passphrase_is_reported_as_such() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    Vault::create(&path, pass("верный"), FAST).unwrap();
    assert!(matches!(Vault::open(&path, pass("неверный")), Err(VaultError::WrongPassphrase)));
}

#[test]
fn damaged_file_is_not_reported_as_wrong_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();
    vault.document_mut().add(sample_draft()).unwrap();
    vault.save().unwrap();
    let good = fs::read(&path).unwrap();

    let mut flipped = good.clone();
    let last = flipped.len() - 5;
    flipped[last] ^= 0x01;
    let truncated = good[..good.len() - 10].to_vec();
    let not_age = b"# work\n### alpha\npassword: p\n".to_vec();

    for damaged in [flipped, truncated, not_age, Vec::new()] {
        fs::write(&path, &damaged).unwrap();
        assert!(matches!(Vault::open(&path, pass("p")), Err(VaultError::Damaged(_))), "{damaged:?}");
    }
}

#[test]
fn missing_file_is_an_io_error() {
    let dir = TempDir::new().unwrap();
    let err = Vault::open(&vault_path(&dir), pass("p")).err().unwrap();
    assert!(matches!(err, VaultError::Io(e) if e.kind() == std::io::ErrorKind::NotFound));
}

#[test]
fn save_leaves_no_temporary_files() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();
    for _ in 0..3 {
        vault.document_mut().add(sample_draft()).unwrap();
        vault.save().unwrap();
    }
    assert_eq!(files_in(&dir), ["vault.md.age"]);
}

#[test]
fn save_refuses_when_the_file_changed_outside_the_app() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();

    // Другая копия приложения записала файл.
    let mut other = Vault::open(&path, pass("p")).unwrap();
    other.document_mut().add(sample_draft()).unwrap();
    other.save().unwrap();
    let written_by_other = fs::read(&path).unwrap();

    vault.document_mut().add(sample_draft()).unwrap();
    assert!(matches!(vault.save(), Err(VaultError::ChangedOnDisk)));
    assert_eq!(fs::read(&path).unwrap(), written_by_other);
}

#[test]
fn save_refuses_when_the_file_was_deleted() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    let mut vault = Vault::create(&path, pass("p"), FAST).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(matches!(vault.save(), Err(VaultError::ChangedOnDisk)));
}

#[test]
fn changing_the_passphrase_re_encrypts_the_file() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    let mut vault = Vault::create(&path, pass("старый"), FAST).unwrap();
    vault.document_mut().add(sample_draft()).unwrap();
    vault.change_passphrase(pass("новый"), FAST).unwrap();

    assert!(matches!(Vault::open(&path, pass("старый")), Err(VaultError::WrongPassphrase)));
    let reopened = Vault::open(&path, pass("новый")).unwrap();
    assert_eq!(reopened.document().entries().len(), 1);
}

#[test]
fn passphrase_check_compares_with_the_current_one() {
    let dir = TempDir::new().unwrap();
    let vault = Vault::create(&vault_path(&dir), pass("верный"), FAST).unwrap();
    assert!(vault.passphrase_matches(&pass("верный")));
    assert!(!vault.passphrase_matches(&pass("неверный")));
}

#[test]
fn empty_passphrase_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    assert!(matches!(Vault::create(&path, pass(""), FAST), Err(VaultError::EmptyPassphrase)));
    assert!(!path.exists());
}

#[test]
fn limiter_blocks_attempts_for_a_while_after_a_failure() {
    let start = Instant::now();
    let mut limiter = AttemptLimiter::new(Duration::from_secs(3));
    assert_eq!(limiter.wait(start), None);

    limiter.failed(start);
    assert_eq!(limiter.wait(start + Duration::from_secs(1)), Some(Duration::from_secs(2)));
    assert_eq!(limiter.wait(start + Duration::from_secs(3)), None);

    limiter.failed(start + Duration::from_secs(10));
    limiter.succeeded();
    assert_eq!(limiter.wait(start + Duration::from_secs(10)), None);
}
