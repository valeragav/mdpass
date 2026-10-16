use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use mdpass_core::cloud::{CloudError, CloudStore, SyncError, SyncState, Synced, download_new, sync};
use mdpass_core::format::{EntryDraft, Favorite, Field};
use mdpass_core::vault::{Passphrase, Vault, VaultOptions};
use tempfile::TempDir;

const FAST: VaultOptions = VaultOptions { work_factor: 4 };

fn pass(s: &str) -> Passphrase {
    Passphrase::from(s.to_string())
}

fn draft(title: &str, password: &str) -> EntryDraft {
    EntryDraft {
        path: vec!["work".into()],
        title: title.into(),
        fields: vec![Field::new("password", password)],
        tags: vec![],
        favorite: Favorite::No,
    }
}

/// Облако в памяти. Общее для нескольких устройств.
#[derive(Default)]
struct Remote {
    file: Option<(Vec<u8>, u64)>,
    /// Умеет ли облако записывать «только если не меняли».
    conditional: bool,
    offline: bool,
    /// Следующий запрос версии вернёт устаревшую: так выглядит запись другого
    /// устройства, которая случилась между запросом версии и загрузкой.
    stale_version: Option<String>,
    uploads: usize,
}

#[derive(Clone)]
struct Cloud(Rc<RefCell<Remote>>);

impl Cloud {
    fn new(conditional: bool) -> Self {
        Self(Rc::new(RefCell::new(Remote { conditional, ..Remote::default() })))
    }
}

impl CloudStore for Cloud {
    fn version(&mut self) -> Result<Option<String>, CloudError> {
        let mut remote = self.0.borrow_mut();
        if remote.offline {
            return Err(CloudError::Offline("нет сети".into()));
        }
        if let Some(stale) = remote.stale_version.take() {
            return Ok(Some(stale));
        }
        Ok(remote.file.as_ref().map(|(_, v)| v.to_string()))
    }

    fn download(&mut self) -> Result<(Vec<u8>, String), CloudError> {
        let remote = self.0.borrow();
        if remote.offline {
            return Err(CloudError::Offline("нет сети".into()));
        }
        let (bytes, version) = remote.file.clone().ok_or(CloudError::Other("файла нет".into()))?;
        Ok((bytes, version.to_string()))
    }

    fn upload(&mut self, bytes: &[u8], expected: Option<&str>) -> Result<String, CloudError> {
        let mut remote = self.0.borrow_mut();
        if remote.offline {
            return Err(CloudError::Offline("нет сети".into()));
        }
        let next = remote.file.as_ref().map_or(1, |(_, v)| v + 1);
        let current = remote.file.as_ref().map(|(_, v)| v.to_string());
        if remote.conditional && current.as_deref() != expected {
            return Err(CloudError::Changed);
        }
        remote.file = Some((bytes.to_vec(), next));
        remote.uploads += 1;
        Ok(next.to_string())
    }
}

/// Устройство: свой локальный файл и своё состояние синхронизации.
struct Device {
    _dir: TempDir,
    path: PathBuf,
    vault: Vault,
    state: SyncState,
}

impl Device {
    /// Первое устройство: создаёт хранилище.
    fn first() -> Self {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.md.age");
        let vault = Vault::create(&path, pass("p"), FAST).unwrap();
        Self { _dir: dir, path, vault, state: SyncState::default() }
    }

    /// Новое устройство: скачивает хранилище из облака.
    fn join(cloud: &mut Cloud) -> Self {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.md.age");
        let state = download_new(cloud, &path).unwrap();
        let vault = Vault::open(&path, pass("p")).unwrap();
        Self { _dir: dir, path, vault, state }
    }

    fn sync(&mut self, cloud: &mut Cloud) -> Result<Synced, SyncError> {
        sync(&mut self.vault, cloud, &mut self.state, |_| Ok(()))
    }

    fn add(&mut self, title: &str, password: &str) {
        self.vault.document_mut().add(draft(title, password)).unwrap();
        self.vault.save().unwrap();
    }

    fn set_password(&mut self, title: &str, password: &str) {
        let entry = self.vault.document().entries().into_iter().find(|e| e.title == title).unwrap();
        self.vault.document_mut().update(entry.id, draft(title, password)).unwrap();
        self.vault.save().unwrap();
    }

    /// Записи как (название, пароль). Читаются заново из файла на диске.
    fn seen(&self) -> Vec<(String, String)> {
        let reopened = Vault::open(&self.path, pass("p")).unwrap();
        assert_eq!(reopened.document().to_markdown(), self.vault.document().to_markdown());
        reopened.document().entries().into_iter().map(|e| (e.title.clone(), e.fields[0].value.clone())).collect()
    }
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

#[test]
fn first_sync_uploads_and_second_device_gets_the_vault() {
    for conditional in [true, false] {
        let mut cloud = Cloud::new(conditional);
        let mut a = Device::first();
        a.add("почта", "1");
        assert!(matches!(a.sync(&mut cloud).unwrap(), Synced::Uploaded));
        assert!(matches!(a.sync(&mut cloud).unwrap(), Synced::UpToDate));

        let mut b = Device::join(&mut cloud);
        assert_eq!(b.seen(), pairs(&[("почта", "1")]));
        assert!(matches!(b.sync(&mut cloud).unwrap(), Synced::UpToDate));
    }
}

#[test]
fn edits_travel_between_devices() {
    let mut cloud = Cloud::new(true);
    let mut a = Device::first();
    a.add("почта", "1");
    a.sync(&mut cloud).unwrap();
    let mut b = Device::join(&mut cloud);

    a.set_password("почта", "2");
    assert!(matches!(a.sync(&mut cloud).unwrap(), Synced::Uploaded));
    assert!(matches!(b.sync(&mut cloud).unwrap(), Synced::Downloaded));
    assert_eq!(b.seen(), pairs(&[("почта", "2")]));
}

#[test]
fn edits_on_both_devices_merge() {
    for conditional in [true, false] {
        let mut cloud = Cloud::new(conditional);
        let mut a = Device::first();
        a.add("почта", "1");
        a.add("банк", "1");
        a.sync(&mut cloud).unwrap();
        let mut b = Device::join(&mut cloud);

        a.set_password("почта", "2");
        b.set_password("банк", "2");
        b.add("сервер", "1");
        a.sync(&mut cloud).unwrap();
        let Synced::Merged(merged) = b.sync(&mut cloud).unwrap() else { panic!("ждали слияние") };
        assert!(merged.conflicts.is_empty());
        assert!(matches!(a.sync(&mut cloud).unwrap(), Synced::Downloaded));

        let want = pairs(&[("почта", "2"), ("банк", "2"), ("сервер", "1")]);
        assert_eq!(a.seen(), want);
        assert_eq!(b.seen(), want);
        assert!(matches!(a.sync(&mut cloud).unwrap(), Synced::UpToDate));
        assert!(matches!(b.sync(&mut cloud).unwrap(), Synced::UpToDate));
    }
}

#[test]
fn same_entry_on_both_devices_keeps_both_versions() {
    let mut cloud = Cloud::new(true);
    let mut a = Device::first();
    a.add("почта", "1");
    a.sync(&mut cloud).unwrap();
    let mut b = Device::join(&mut cloud);

    a.set_password("почта", "с ноутбука");
    b.set_password("почта", "с работы");
    a.sync(&mut cloud).unwrap();
    let Synced::Merged(merged) = b.sync(&mut cloud).unwrap() else { panic!("ждали слияние") };
    assert_eq!(merged.conflicts.len(), 1);
    assert_eq!(b.seen(), pairs(&[("почта", "с ноутбука"), ("почта (копия с этого устройства)", "с работы")]));
}

#[test]
fn upload_race_is_retried_with_a_merge() {
    // Облако с условной записью: пока b собирался загрузить, a успел записать своё.
    let mut cloud = Cloud::new(true);
    let mut a = Device::first();
    a.add("почта", "1");
    a.add("банк", "1");
    a.sync(&mut cloud).unwrap();
    let mut b = Device::join(&mut cloud);
    let seen_by_b = cloud.0.borrow().file.clone().unwrap().1;

    a.set_password("почта", "2");
    a.sync(&mut cloud).unwrap();
    b.set_password("банк", "2");
    cloud.0.borrow_mut().stale_version = Some(seen_by_b.to_string());
    assert!(matches!(b.sync(&mut cloud).unwrap(), Synced::Merged(_)));

    assert!(matches!(a.sync(&mut cloud).unwrap(), Synced::Downloaded));
    let want = pairs(&[("почта", "2"), ("банк", "2")]);
    assert_eq!(a.seen(), want);
    assert_eq!(b.seen(), want);
}

#[test]
fn overwritten_version_is_recovered_on_a_cloud_without_conditional_writes() {
    // Яндекс Диск и подобные пишут поверх: b не заметил запись a и затёр её.
    let mut cloud = Cloud::new(false);
    let mut a = Device::first();
    a.add("почта", "1");
    a.add("банк", "1");
    a.sync(&mut cloud).unwrap();
    let mut b = Device::join(&mut cloud);
    let before_a = cloud.0.borrow().file.clone().unwrap().1;

    a.set_password("почта", "2");
    a.sync(&mut cloud).unwrap();
    b.set_password("банк", "2");
    cloud.0.borrow_mut().stale_version = Some(before_a.to_string());
    assert!(matches!(b.sync(&mut cloud).unwrap(), Synced::Uploaded), "b затёр версию a");

    // a видит в облаке версию, в истории которой нет его последней записи, и сливает
    // от общей версии.
    assert!(matches!(a.sync(&mut cloud).unwrap(), Synced::Merged(_)));
    b.sync(&mut cloud).unwrap();
    let want = pairs(&[("почта", "2"), ("банк", "2")]);
    assert_eq!(a.seen(), want);
    assert_eq!(b.seen(), want);
}

#[test]
fn offline_keeps_working_locally() {
    let mut cloud = Cloud::new(true);
    let mut a = Device::first();
    a.add("почта", "1");
    a.sync(&mut cloud).unwrap();

    cloud.0.borrow_mut().offline = true;
    a.set_password("почта", "2");
    assert!(matches!(a.sync(&mut cloud), Err(SyncError::Cloud(CloudError::Offline(_)))));
    assert_eq!(a.seen(), pairs(&[("почта", "2")]));

    cloud.0.borrow_mut().offline = false;
    assert!(matches!(a.sync(&mut cloud).unwrap(), Synced::Uploaded));
}

#[test]
fn other_passphrase_in_the_cloud_is_reported() {
    let mut cloud = Cloud::new(true);
    let mut a = Device::first();
    a.add("почта", "1");
    a.sync(&mut cloud).unwrap();
    let mut b = Device::join(&mut cloud);

    a.vault.change_passphrase(pass("новый"), FAST).unwrap();
    a.sync(&mut cloud).unwrap();
    b.set_password("почта", "2");
    assert!(matches!(b.sync(&mut cloud), Err(SyncError::OtherPassphrase)));
    assert_eq!(b.seen(), pairs(&[("почта", "2")]), "своё не тронуто");
}

#[test]
fn unencrypted_vault_is_not_synced() {
    let dir = TempDir::new().unwrap();
    let mut vault = Vault::create_plain(&dir.path().join("vault.md")).unwrap();
    let mut cloud = Cloud::new(true);
    let mut state = SyncState::default();
    assert!(matches!(sync(&mut vault, &mut cloud, &mut state, |_| Ok(())), Err(SyncError::NotEncrypted)));
    assert_eq!(cloud.0.borrow().uploads, 0);
}

#[test]
fn state_survives_a_restart() {
    let mut cloud = Cloud::new(false);
    let mut a = Device::first();
    a.add("почта", "1");
    a.add("банк", "1");
    a.sync(&mut cloud).unwrap();
    let mut b = Device::join(&mut cloud);

    // a пишет и закрывается, состояние — на диск.
    let state_dir = TempDir::new().unwrap();
    a.set_password("почта", "2");
    a.sync(&mut cloud).unwrap();
    a.state.save(state_dir.path()).unwrap();

    // b правит офлайн, а a тем временем ещё раз пишет и закрывается.
    b.set_password("банк", "2");
    a.state = SyncState::load(state_dir.path()).unwrap();
    a.add("сервер", "1");
    a.sync(&mut cloud).unwrap();
    a.state.save(state_dir.path()).unwrap();

    b.sync(&mut cloud).unwrap();
    a.state = SyncState::load(state_dir.path()).unwrap();
    a.vault = Vault::open(&a.path, pass("p")).unwrap();
    a.sync(&mut cloud).unwrap();
    let want = pairs(&[("почта", "2"), ("банк", "2"), ("сервер", "1")]);
    assert_eq!(a.seen(), want);
    assert_eq!(b.seen(), want);
}
