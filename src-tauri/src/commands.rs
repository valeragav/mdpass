//! Команды, которые вызывает интерфейс.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use age::secrecy::ExposeSecret;
use mdpass_core::backup::BackupKind;
use mdpass_core::vault::{Passphrase, Vault, VaultError, VaultOptions};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::session::{AppState, RETRY_DELAY};
use crate::settings::Settings;

const DEFAULT_FILE_NAME: &str = "vault.md.age";

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CommandError {
    WrongPassphrase {
        retry_in_ms: u64,
    },
    TooSoon {
        retry_in_ms: u64,
    },
    Damaged {
        detail: String,
    },
    NotFound,
    AlreadyExists,
    EmptyPassphrase,
    /// Файл зашифрован, а пароль не введён.
    PassphraseRequired,
    ChangedOnDisk,
    /// Хранилище заблокировано: команде нужны открытые записи.
    Locked,
    EntryNotFound,
    Clipboard {
        message: String,
    },
    /// Резервная копия зашифрована другим мастер-паролем: сделана до его смены.
    OtherPassphrase,
    /// Данные записи не прошли проверку. В `message` — текст для пользователя.
    Invalid {
        message: String,
    },
    Io {
        message: String,
    },
    /// Записи обновились из облака: интерфейсу нужно их перечитать.
    Stale,
    /// Облако недоступно или отказало. В `message` — текст для пользователя.
    Cloud {
        message: String,
    },
}

impl From<VaultError> for CommandError {
    fn from(err: VaultError) -> Self {
        match err {
            VaultError::EmptyPassphrase => Self::EmptyPassphrase,
            VaultError::AlreadyExists => Self::AlreadyExists,
            VaultError::WrongPassphrase => Self::WrongPassphrase { retry_in_ms: millis(RETRY_DELAY) },
            VaultError::Damaged(detail) => {
                log::warn!("файл хранилища не читается: {detail}");
                Self::Damaged { detail }
            }
            VaultError::ChangedOnDisk => Self::ChangedOnDisk,
            VaultError::NotDamaged => Self::Invalid { message: VaultError::NotDamaged.to_string() },
            VaultError::PassphraseRequired => Self::PassphraseRequired,
            VaultError::NewerFormat(version) => Self::Invalid {
                message: format!("Файл записан более новой версией mdpass (формат {version}). Обновите приложение"),
            },
            VaultError::Io(err) if err.kind() == std::io::ErrorKind::NotFound => Self::NotFound,
            VaultError::Io(err) => {
                log::warn!("ошибка доступа к файлу: {err}");
                Self::Io { message: err.to_string() }
            }
        }
    }
}

pub(crate) fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Файл хранилища, как его показывает интерфейс.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultFile {
    path: String,
    /// Путь с `~` вместо домашней папки.
    display: String,
    name: String,
    exists: bool,
    /// Зашифрован ли файл. `None`, если файла нет или его не прочитать.
    encrypted: Option<bool>,
}

/// Как защищено хранилище.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protection {
    /// Надёжное шифрование: scrypt 2^18.
    Strong,
    /// Слабое шифрование: scrypt 2^12, открывается мгновенно.
    Weak,
    /// Без шифрования: обычный Markdown-файл.
    None,
}

impl Protection {
    pub fn of(vault: &Vault) -> Self {
        match vault.options() {
            Some(options) if options.is_strong() => Self::Strong,
            Some(_) => Self::Weak,
            None => Self::None,
        }
    }

    /// Минимальная длина мастер-пароля. При слабом шифровании разрешён короткий
    /// пароль: пользователь сам выбрал скорость вместо стойкости.
    fn min_passphrase_chars(self) -> usize {
        match self {
            Self::Weak => 4,
            Self::Strong | Self::None => 8,
        }
    }

    fn options(self) -> Option<VaultOptions> {
        match self {
            Self::Strong => Some(VaultOptions::STRONG),
            Self::Weak => Some(VaultOptions::WEAK),
            Self::None => None,
        }
    }
}

pub(crate) fn describe<R: Runtime>(app: &AppHandle<R>, path: &Path) -> VaultFile {
    let display = match app.path().home_dir().ok().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf))
    {
        Some(rest) => format!("~/{}", rest.to_string_lossy().replace('\\', "/")),
        None => path.to_string_lossy().into_owned(),
    };
    VaultFile {
        path: path.to_string_lossy().into_owned(),
        display,
        name: path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
        exists: path.is_file(),
        encrypted: path.is_file().then(|| mdpass_core::vault::file_is_encrypted(path).ok()).flatten(),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    unlocked: bool,
    /// Последнее открытое хранилище.
    last_vault: Option<VaultFile>,
    /// Недавние хранилища, последнее — первым.
    recent_vaults: Vec<VaultFile>,
    /// Куда предложить положить новое хранилище.
    default_vault: Option<VaultFile>,
    /// Защита открытого хранилища. `None`, пока хранилище заблокировано.
    protection: Option<Protection>,
    settings: Settings,
}

#[tauri::command(async)]
pub fn status<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Status {
    let session = state.lock();
    let default_vault = app.path().document_dir().ok().map(|dir| describe(&app, &dir.join(DEFAULT_FILE_NAME)));
    Status {
        unlocked: session.vault.is_some(),
        last_vault: session.config.last_vault.as_deref().map(|path| describe(&app, path)),
        recent_vaults: session.config.recent_vaults.iter().map(|path| describe(&app, path)).collect(),
        default_vault,
        protection: session.vault.as_ref().map(Protection::of),
        settings: Settings::from(&session.config),
    }
}

#[tauri::command(async)]
pub fn describe_path<R: Runtime>(app: AppHandle<R>, path: String) -> VaultFile {
    describe(&app, Path::new(&path))
}

#[tauri::command]
pub async fn create_vault(
    state: State<'_, AppState>,
    path: String,
    passphrase: Option<String>,
    protection: Protection,
) -> Result<Protection, CommandError> {
    let state = state.inner().clone();
    let passphrase = passphrase.map(Passphrase::from);
    run_blocking(move || {
        let path = PathBuf::from(path);
        let vault = match (protection.options(), passphrase) {
            (None, _) => Vault::create_plain(&path)?,
            (Some(options), Some(passphrase)) => {
                check_length(&passphrase, protection)?;
                Vault::create(&path, passphrase, options)?
            }
            (Some(_), None) => return Err(CommandError::EmptyPassphrase),
        };
        let protection = Protection::of(&vault);
        let mut session = state.lock();
        session.replace_vault(vault);
        session.touch(SystemTime::now());
        session.config.remember(path);
        session.save_config();
        Ok(protection)
    })
    .await
}

fn check_length(passphrase: &Passphrase, protection: Protection) -> Result<(), CommandError> {
    let min = protection.min_passphrase_chars();
    if passphrase.expose_secret().chars().count() < min {
        return Err(CommandError::Invalid {
            message: format!("Мастер-пароль должен быть не короче {min} символов")
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn unlock(
    state: State<'_, AppState>,
    path: String,
    passphrase: Option<String>,
) -> Result<Protection, CommandError> {
    let state = state.inner().clone();
    let passphrase = passphrase.filter(|p| !p.is_empty()).map(Passphrase::from);
    run_blocking(move || {
        // Сессия занята всё время проверки: параллельные попытки не обойдут паузу.
        let mut session = state.lock();
        if let Some(wait) = session.limiter.wait(Instant::now()) {
            return Err(CommandError::TooSoon { retry_in_ms: millis(wait) });
        }
        let path = PathBuf::from(path);
        let opened = match passphrase {
            Some(passphrase) => Vault::open(&path, passphrase),
            None => Vault::open_plain(&path),
        };
        match opened {
            Ok(vault) => {
                let protection = Protection::of(&vault);
                session.limiter.succeeded();
                session.replace_vault(vault);
                session.touch(SystemTime::now());
                session.config.remember(path);
                session.save_config();
                crate::cloud::wake();
                Ok(protection)
            }
            Err(err) => {
                if matches!(err, VaultError::WrongPassphrase) {
                    session.limiter.failed(Instant::now());
                }
                Err(err.into())
            }
        }
    })
    .await
}

#[tauri::command(async)]
pub fn lock(state: State<'_, AppState>) {
    state.lock().lock_vault();
}

/// Перечитывает файл, который изменили вне приложения. Если его перешифровали
/// другим паролем, хранилище блокируется: нужно открыть его заново.
#[tauri::command]
pub async fn reload_vault(state: State<'_, AppState>) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_blocking(move || {
        let mut session = state.lock();
        let vault = session.vault.as_mut().ok_or(CommandError::Locked)?;
        match vault.reload() {
            Ok(()) => Ok(()),
            Err(err @ (VaultError::WrongPassphrase | VaultError::PassphraseRequired)) => {
                session.lock_vault();
                Err(err.into())
            }
            Err(err) => Err(err.into()),
        }
    })
    .await
}

/// Переименовывает файл открытого хранилища в той же папке. Расширение `.md.age`
/// или `.md` подставляется по защите, папка резервных копий переезжает следом.
#[tauri::command(async)]
pub fn rename_vault<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    name: String,
) -> Result<VaultFile, CommandError> {
    let mut session = state.lock();
    let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
    let name = name.trim();
    let bad = name.is_empty()
        || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        || name.starts_with('.')
        || name.ends_with('.');
    if bad {
        return Err(CommandError::Invalid { message: "Недопустимое имя файла".into() });
    }
    let stem = name.trim_end_matches(".age").trim_end_matches(".md");
    let extension = if vault.is_encrypted() { ".md.age" } else { ".md" };
    let old_path = vault.path().to_path_buf();
    let new_path = old_path.with_file_name(format!("{stem}{extension}"));
    let vault = session.vault.as_mut().ok_or(CommandError::Locked)?;
    vault.rename_to(&new_path)?;
    let old_sync = crate::cloud::state_dir(&session, &old_path);
    if let Err(err) = session.backup_store(&old_path).move_to(&session.backup_store(&new_path)) {
        log::warn!("не удалось перенести резервные копии: {err}");
    }
    let new_sync = crate::cloud::state_dir(&session, &new_path);
    if old_sync.exists()
        && let Err(err) = std::fs::rename(&old_sync, &new_sync)
    {
        log::warn!("не удалось перенести состояние синхронизации: {err}");
    }
    session.sync_state = None;
    session.config.renamed(&old_path, new_path.clone());
    session.save_config();
    Ok(describe(&app, &new_path))
}

/// Интерфейс сообщает, что пользователь что-то делает: откладывает автоблокировку.
#[tauri::command(async)]
pub fn touch(state: State<'_, AppState>) {
    state.lock().touch(SystemTime::now());
}

/// Меняет защиту открытого хранилища: пароль, силу шифрования или снимает шифрование.
/// У зашифрованного хранилища сначала проверяется текущий пароль.
/// После смены защиты файл получает подходящее имя: `vault.md.age` или `vault.md`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionChanged {
    protection: Protection,
    vault: VaultFile,
}

#[tauri::command]
pub async fn change_protection<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    current: Option<String>,
    new: Option<String>,
    protection: Protection,
) -> Result<ProtectionChanged, CommandError> {
    let state = state.inner().clone();
    let current = current.map(Passphrase::from);
    let new = new.map(Passphrase::from);
    run_blocking(move || {
        let mut session = state.lock();
        if let Some(wait) = session.limiter.wait(Instant::now()) {
            return Err(CommandError::TooSoon { retry_in_ms: millis(wait) });
        }
        let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
        if vault.is_encrypted() && !current.as_ref().is_some_and(|c| vault.passphrase_matches(c)) {
            session.limiter.failed(Instant::now());
            return Err(CommandError::WrongPassphrase { retry_in_ms: millis(RETRY_DELAY) });
        }
        let key = match (protection.options(), new) {
            (None, _) => None,
            (Some(options), Some(new)) => {
                check_length(&new, protection)?;
                Some((new, options))
            }
            (Some(_), None) => return Err(CommandError::EmptyPassphrase),
        };
        session.limiter.succeeded();
        let encrypting = key.is_some();
        let policy = session.backup_policy();
        let vault = session.vault.as_mut().ok_or(CommandError::Locked)?;
        match &policy {
            Some(policy) => vault.set_protection(key, policy.hook(BackupKind::Passphrase))?,
            None => vault.set_protection(key, |_| Ok(()))?,
        }

        // Имя файла следует за защитой. Если нужное имя занято чужим файлом,
        // защита всё равно уже сменилась, файл просто остаётся под старым именем.
        let old_path = vault.path().to_path_buf();
        let wanted = vault.conventional_path();
        if wanted != old_path && vault.rename_to(&wanted).is_ok() {
            if let Err(err) = session.backup_store(&old_path).move_to(&session.backup_store(&wanted)) {
                log::warn!("не удалось перенести резервные копии: {err}");
            }
            session.config.renamed(&old_path, wanted);
            session.save_config();
        }
        let vault = session.vault.as_ref().ok_or(CommandError::Locked)?;
        // Хранилище теперь шифруется: открытые копии прежних версий удаляются, иначе
        // все пароли ещё долго лежали бы в папке приложения открытым текстом.
        if encrypting && let Err(err) = session.backup_store(vault.path()).remove_plain() {
            log::warn!("не удалось удалить незашифрованные копии: {err}");
        }
        Ok(ProtectionChanged { protection: Protection::of(vault), vault: describe(&app, vault.path()) })
    })
    .await
}

/// Шифрование занимает около секунды: уводим его из потока, который обслуживает окно.
pub(crate) async fn run_blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|err| Err(CommandError::Io { message: err.to_string() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(s: &str) -> Passphrase {
        Passphrase::from(s.to_string())
    }

    #[test]
    fn weak_encryption_allows_a_four_character_passphrase() {
        assert!(check_length(&pass("abcd"), Protection::Weak).is_ok());
        assert!(check_length(&pass("abc"), Protection::Weak).is_err());
        assert!(check_length(&pass("abcd"), Protection::Strong).is_err());
        assert!(check_length(&pass("abcdefgh"), Protection::Strong).is_ok());
    }
}
