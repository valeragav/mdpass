//! Синхронизация с облаком: связь хранилища с файлом в облаке, синхронизация
//! в фоне.
//!
//! Вход в облако — своим OAuth-токеном: пользователь получает его на странице
//! облака сам (там же регистрирует своё приложение с доступом к служебной папке)
//! и вводит токен в настройках, указав облако. mdpass за кодом сам не ходит
//! и ничего не регистрирует. Токен лежит в системном хранилище паролей, отдельно
//! для каждого облака — переключение между ними не теряет прежний токен.
//! В настройках — только Client ID (для справки) и какое хранилище с каким файлом
//! в облаке связано. Запросы к облаку идут в отдельном потоке и без блокировки
//! сессии: медленная сеть не держит окно.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mdpass_core::backup::BackupKind;
use mdpass_core::cloud::{
    Backup, CloudError, CloudStore, Local, SyncError, SyncState, Synced, download_new, sync_with,
};
use mdpass_core::config::{CloudLink, Provider};
use mdpass_core::vault::Vault;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::commands::{CommandError, VaultFile, describe, millis, run_blocking};
use crate::session::{AppState, MergeNotice, Session};

const KEYRING_SERVICE: &str = "io.github.valeragav.mdpass";
/// Состояние синхронизации изменилось.
const STATUS_EVENT: &str = "cloud-status";
/// Синхронизация изменила записи: интерфейсу пора их перечитать.
const CHANGED_EVENT: &str = "vault-changed";
/// Как часто синхронизироваться, пока хранилище открыто.
const PERIOD: Duration = Duration::from_secs(60);
/// Пауза после правки: несколько правок подряд уходят одной загрузкой.
const SETTLE: Duration = Duration::from_millis(800);

static WAKE: OnceLock<Sender<()>> = OnceLock::new();

/// Синхронизировать как можно скорее: после разблокировки или правки.
pub fn wake() {
    if let Some(wake) = WAKE.get() {
        let _ = wake.send(());
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SyncStatus {
    #[default]
    Idle,
    Syncing,
    Synced {
        at_ms: u64,
    },
    Offline {
        message: String,
    },
    /// Токен недействителен или доступ отозвали: нужен новый.
    NeedsLogin,
    Failed {
        message: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudView {
    /// Облако, для которого сохранён токен. `None` — ни одно не подключено.
    provider: Option<Provider>,
    /// Токен введён.
    connected: bool,
    /// Client ID, который ввели вместе с токеном. Только чтобы показать в настройках.
    client_id: Option<String>,
    /// Связано ли открытое хранилище с облаком.
    linked: bool,
    /// Зашифровано ли открытое хранилище: открытое в облако не отправляется.
    encrypted: bool,
    /// Имя файла в облаке у открытого хранилища.
    file_name: Option<String>,
    status: SyncStatus,
}

impl From<CloudError> for CommandError {
    fn from(err: CloudError) -> Self {
        log::warn!("облако: {err}");
        Self::Cloud { message: err.to_string() }
    }
}

impl From<SyncError> for CommandError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::Vault(err) => err.into(),
            SyncError::Locked => Self::Locked,
            other => Self::Cloud { message: other.to_string() },
        }
    }
}

// Токен в системном хранилище паролей: свой на каждое облако.

fn keyring_entry(provider: Provider) -> Result<keyring::Entry, CommandError> {
    keyring::Entry::new(KEYRING_SERVICE, provider.as_str()).map_err(keyring_error)
}

fn keyring_error(err: keyring::Error) -> CommandError {
    log::warn!("системное хранилище паролей: {err}");
    CommandError::Cloud {
        message: format!("Системное хранилище паролей недоступно: {err}")
    }
}

fn load_token(provider: Provider) -> Option<String> {
    keyring_entry(provider).ok()?.get_password().ok()
}

fn save_token(provider: Provider, token: &str) -> Result<(), CommandError> {
    keyring_entry(provider)?.set_password(token).map_err(keyring_error)
}

fn delete_token(provider: Provider) {
    if let Ok(entry) = keyring_entry(provider) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(err) => log::warn!("не удалось удалить токен облака: {err}"),
        }
    }
}

/// Доступ к файлу хранилища в выбранном облаке.
fn store(provider: Provider, token: &str, file_name: &str) -> Box<dyn CloudStore> {
    match provider {
        Provider::Yandex => Box::new(mdpass_cloud::yandex::YandexDisk::new(token, file_name)),
        Provider::Google => Box::new(mdpass_cloud::google::GoogleDrive::new(token, file_name)),
    }
}

/// Хранилища в облаке, доступные на этот токен.
fn list_vaults(provider: Provider, token: &str) -> Result<Vec<String>, CloudError> {
    match provider {
        Provider::Yandex => mdpass_cloud::yandex::list_vaults(token),
        Provider::Google => mdpass_cloud::google::list_vaults(token),
    }
}

// Состояние синхронизации на диске.

/// Папка состояния синхронизации хранилища: рядом с папкой его резервных копий,
/// с тем же именем.
pub(crate) fn state_dir(session: &Session, vault: &Path) -> PathBuf {
    let store = session.backup_store(vault);
    let name = store.dir().file_name().map(|n| n.to_os_string()).unwrap_or_default();
    session.backups_root.with_file_name("sync").join(name)
}

/// Хранилище из сессии для одного шага синхронизации, под её блокировкой.
struct SessionLocal {
    state: AppState,
    path: PathBuf,
}

impl Local for SessionLocal {
    fn with<R>(
        &mut self,
        step: impl FnOnce(&mut Vault, &mut SyncState, &mut Backup) -> Result<R, SyncError>,
    ) -> Result<R, SyncError> {
        let mut session = self.state.lock();
        if session.vault.as_ref().is_none_or(|v| v.path() != self.path) || session.cloud_hold {
            return Err(SyncError::Locked);
        }
        let dir = state_dir(&session, &self.path);
        if session.sync_state.as_ref().is_none_or(|(path, _)| *path != self.path) {
            let loaded = SyncState::load(&dir).unwrap_or_else(|err| {
                log::warn!("не удалось прочитать состояние синхронизации: {err}");
                SyncState::default()
            });
            session.sync_state = Some((self.path.clone(), loaded));
        }
        let policy = session.backup_policy();
        let session = &mut *session;
        let (Some(vault), Some((_, state))) = (session.vault.as_mut(), session.sync_state.as_mut()) else {
            return Err(SyncError::Locked);
        };
        let mut hook = policy.as_ref().map(|policy| policy.hook(BackupKind::Sync));
        let mut backup = |bytes: &[u8]| match hook.take() {
            Some(hook) => hook(bytes),
            None => Ok(()),
        };
        let result = step(vault, state, &mut backup);
        if let Err(err) = state.save(&dir) {
            log::warn!("не удалось записать состояние синхронизации: {err}");
        }
        result
    }
}

// Фоновая синхронизация.

/// Запускает поток синхронизации: по сигналу `wake` и раз в минуту.
pub fn start(app: AppHandle) {
    let (wake, wakes) = channel();
    if WAKE.set(wake).is_err() {
        return;
    }
    std::thread::spawn(move || {
        loop {
            match wakes.recv_timeout(PERIOD) {
                Ok(()) => std::thread::sleep(SETTLE),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            while wakes.try_recv().is_ok() {}
            run_once(&app);
        }
    });
}

fn set_status<R: Runtime>(app: &AppHandle<R>, status: SyncStatus) {
    app.state::<AppState>().lock().cloud_status = status.clone();
    let _ = app.emit(STATUS_EVENT, status);
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(millis).unwrap_or_default()
}

fn run_once<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>().inner().clone();
    let (path, provider, file_name, before) = {
        let session = state.lock();
        if session.cloud_hold {
            return;
        }
        let Some(vault) = &session.vault else { return };
        let Some(link) = session.config.cloud_link(vault.path()) else { return };
        (vault.path().to_path_buf(), link.provider, link.file_name.clone(), vault.bytes().to_vec())
    };
    let Some(token) = load_token(provider) else {
        set_status(app, SyncStatus::NeedsLogin);
        return;
    };
    set_status(app, SyncStatus::Syncing);

    let mut cloud = store(provider, &token, &file_name);
    let result = sync_with(&mut SessionLocal { state: state.clone(), path: path.clone() }, &mut *cloud);

    let status = match &result {
        Ok(synced) => {
            log::info!("синхронизация: {}", describe_synced(synced));
            SyncStatus::Synced { at_ms: now_ms() }
        }
        Err(SyncError::Cloud(CloudError::Offline(reason))) => SyncStatus::Offline { message: reason.clone() },
        Err(SyncError::Cloud(CloudError::Unauthorized)) => SyncStatus::NeedsLogin,
        Err(SyncError::Locked) => SyncStatus::Idle,
        Err(err) => {
            log::warn!("синхронизация не удалась: {err}");
            SyncStatus::Failed { message: err.to_string() }
        }
    };

    let changed = {
        let mut session = state.lock();
        if let Ok(Synced::Merged(merged)) = &result {
            let both_kept: Vec<String> =
                merged.conflicts.iter().filter_map(|c| c.mine.as_ref().map(|e| e.title.clone())).collect();
            if !both_kept.is_empty() {
                session.merge_notice = Some(MergeNotice { both_kept });
            }
        }
        let changed = session.vault.as_ref().is_some_and(|v| v.path() == path && v.bytes() != before.as_slice());
        if changed {
            session.ids_fresh = false;
        }
        changed
    };
    if changed {
        let _ = app.emit(CHANGED_EVENT, ());
    }
    set_status(app, status);
}

fn describe_synced(synced: &Synced) -> &'static str {
    match synced {
        Synced::UpToDate => "без изменений",
        Synced::Uploaded => "загружено в облако",
        Synced::Downloaded => "взята версия из облака",
        Synced::Merged(_) => "изменения слиты",
    }
}

// Команды.

#[tauri::command]
pub fn cloud_status(state: State<'_, AppState>) -> CloudView {
    let session = state.lock();
    let link = session.vault.as_ref().and_then(|v| session.config.cloud_link(v.path()));
    let provider = session.config.cloud_provider;
    CloudView {
        provider,
        connected: provider.is_some_and(|p| load_token(p).is_some()),
        client_id: session.config.cloud_client_id.clone(),
        linked: link.is_some(),
        encrypted: session.vault.as_ref().is_some_and(Vault::is_encrypted),
        file_name: link.map(|l| l.file_name.clone()),
        status: session.cloud_status.clone(),
    }
}

/// Подключает облако готовым токеном: его и Client ID пользователь вводит сам,
/// получив их на странице облака. mdpass никуда сам за ними не ходит.
#[tauri::command]
pub fn cloud_connect(
    state: State<'_, AppState>,
    provider: Provider,
    client_id: String,
    token: String,
) -> Result<(), CommandError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(CommandError::Invalid { message: "Введите токен".into() });
    }
    save_token(provider, token)?;
    let mut session = state.lock();
    let client_id = client_id.trim();
    session.config.cloud_provider = Some(provider);
    session.config.cloud_client_id = (!client_id.is_empty()).then(|| client_id.to_string());
    session.cloud_status = SyncStatus::Idle;
    session.save_config();
    drop(session);
    wake();
    Ok(())
}

/// Выходит из облака на этом компьютере: токен удаляется, хранилища больше
/// не синхронизируются. Файлы в облаке остаются.
#[tauri::command]
pub fn cloud_logout(state: State<'_, AppState>) {
    let mut session = state.lock();
    if let Some(provider) = session.config.cloud_provider {
        delete_token(provider);
    }
    session.config.cloud_provider = None;
    session.config.cloud_client_id = None;
    session.config.cloud_links.clear();
    session.sync_state = None;
    session.cloud_status = SyncStatus::Idle;
    session.save_config();
}

/// Связывает открытое хранилище с файлом в облаке и сразу синхронизирует. Если файл
/// с таким именем в облаке уже есть, записи сольются.
#[tauri::command]
pub fn cloud_link(state: State<'_, AppState>) -> Result<(), CommandError> {
    let mut session = state.lock();
    let provider = session
        .config
        .cloud_provider
        .ok_or(CommandError::Cloud { message: "Облако не подключено".into() })?;
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    if !vault.is_encrypted() {
        return Err(CommandError::Cloud { message: SyncError::NotEncrypted.to_string() });
    }
    let path = vault.path().to_path_buf();
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "vault.md.age".into());
    if session.config.cloud_link(&path).is_none() {
        session.config.cloud_links.push(CloudLink { vault: path, provider, file_name });
        session.save_config();
    }
    drop(session);
    wake();
    Ok(())
}

/// Отвязывает открытое хранилище от облака. Файл в облаке остаётся.
#[tauri::command]
pub fn cloud_unlink(state: State<'_, AppState>) -> Result<(), CommandError> {
    let mut session = state.lock();
    let path = session.vault.as_ref().ok_or(CommandError::Locked)?.path().to_path_buf();
    session.config.cloud_links.retain(|link| link.vault != path);
    session.save_config();
    let dir = state_dir(&session, &path);
    if let Err(err) = std::fs::remove_dir_all(&dir)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("не удалось удалить состояние синхронизации: {err}");
    }
    session.sync_state = None;
    session.cloud_status = SyncStatus::Idle;
    Ok(())
}

#[tauri::command]
pub fn cloud_sync_now() {
    wake();
}

/// Пока открыт редактор записи, записи из облака не меняются: синхронизация ждёт.
#[tauri::command]
pub fn cloud_hold(state: State<'_, AppState>, hold: bool) {
    state.lock().cloud_hold = hold;
    if !hold {
        wake();
    }
}

/// Хранилища в облаке, которые можно скачать на этот компьютер.
#[tauri::command]
pub async fn cloud_list(state: State<'_, AppState>) -> Result<Vec<String>, CommandError> {
    let provider = state
        .lock()
        .config
        .cloud_provider
        .ok_or(CommandError::Cloud { message: "Сначала подключите облако".into() })?;
    run_blocking(move || {
        let token = load_token(provider)
            .ok_or(CommandError::Cloud { message: "Сначала подключите облако".into() })?;
        Ok(list_vaults(provider, &token)?)
    })
    .await
}

/// Скачивает хранилище из облака в новый файл и связывает их. Открывают его потом
/// как обычно, мастер-паролем.
#[tauri::command]
pub async fn cloud_download<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> Result<VaultFile, CommandError> {
    let state = state.inner().clone();
    let provider = state
        .lock()
        .config
        .cloud_provider
        .ok_or(CommandError::Cloud { message: "Сначала подключите облако".into() })?;
    run_blocking(move || {
        let token = load_token(provider)
            .ok_or(CommandError::Cloud { message: "Сначала подключите облако".into() })?;
        let path = PathBuf::from(path);
        let mut cloud = store(provider, &token, &name);
        let sync_state = download_new(&mut *cloud, &path)?;
        let mut session = state.lock();
        if let Err(err) = sync_state.save(&state_dir(&session, &path)) {
            log::warn!("не удалось записать состояние синхронизации: {err}");
        }
        session.config.cloud_links.retain(|link| link.vault != path);
        session.config.cloud_links.push(CloudLink { vault: path.clone(), provider, file_name: name });
        session.config.remember(path.clone());
        session.save_config();
        Ok(describe(&app, &path))
    })
    .await
}
