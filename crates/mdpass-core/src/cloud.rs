//! Синхронизация хранилища с облаком. Облако — сменный транспорт за [`CloudStore`]:
//! узнать версию файла, скачать, загрузить. Слияние и всё остальное здесь общее
//! для всех облаков.
//!
//! Хранилище остаётся локальным файлом: с ним работают офлайн. На устройстве
//! хранится [`SyncState`] — какую версию файла в облаке видели последней и байты
//! недавних синхронизированных версий. Они нужны как база для слияния: правки
//! с двух устройств сливаются от общей версии, а не затирают друг друга.
//!
//! В облако уходит только зашифрованный файл: хранилище без шифрования
//! не синхронизируется.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::format::Document;
use crate::fs_atomic::write_atomic;
use crate::merge::Choice;
use crate::sync::stamp;
use crate::vault::{Merged, Vault, VaultError, copy_title};

/// Файл хранилища в облаке.
pub trait CloudStore {
    /// Версия файла в облаке: непрозрачная строка от облака (хеш, ETag, номер
    /// ревизии). `None` — файла нет.
    fn version(&mut self) -> Result<Option<String>, CloudError>;

    /// Байты файла и их версия.
    fn download(&mut self) -> Result<(Vec<u8>, String), CloudError>;

    /// Загружает файл и возвращает его новую версию. `expected` — версия, поверх
    /// которой пишем (`None` — файла не было). Если облако умеет записывать «только
    /// если не меняли», а файл уже другой, возвращает [`CloudError::Changed`].
    /// Облако, которое так не умеет, пишет поверх: такую потерю заметит следующая
    /// синхронизация по истории версий в самом файле.
    fn upload(&mut self, bytes: &[u8], expected: Option<&str>) -> Result<String, CloudError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudError {
    /// Нет сети или облако не отвечает.
    Offline(String),
    /// Вход истёк или доступ отозван: нужно войти заново.
    Unauthorized,
    /// Файл в облаке изменился, пока мы готовили запись.
    Changed,
    Other(String),
}

impl fmt::Display for CloudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offline(reason) => write!(f, "Облако недоступно: {reason}"),
            Self::Unauthorized => write!(f, "Нужно заново войти в облако"),
            Self::Changed => write!(f, "Файл в облаке изменился во время записи"),
            Self::Other(reason) => write!(f, "Ошибка облака: {reason}"),
        }
    }
}

#[derive(Debug)]
pub enum SyncError {
    Cloud(CloudError),
    Vault(VaultError),
    /// Хранилище без шифрования: открытый файл в облако не отправляется.
    NotEncrypted,
    /// Файл в облаке зашифрован другим паролем: его перешифровали на другом устройстве.
    OtherPassphrase,
    /// Файл в облаке меняется быстрее, чем мы успеваем записать. Попробовать позже.
    Busy,
    /// Хранилище заблокировали во время синхронизации.
    Locked,
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cloud(err) => err.fmt(f),
            Self::Vault(err) => err.fmt(f),
            Self::NotEncrypted => write!(f, "Хранилище без шифрования не синхронизируется с облаком"),
            Self::OtherPassphrase => write!(f, "Файл в облаке зашифрован другим мастер-паролем"),
            Self::Busy => write!(f, "Файл в облаке всё время меняется: попробуйте позже"),
            Self::Locked => write!(f, "Хранилище заблокировано"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<CloudError> for SyncError {
    fn from(err: CloudError) -> Self {
        Self::Cloud(err)
    }
}

impl From<VaultError> for SyncError {
    fn from(err: VaultError) -> Self {
        Self::Vault(err)
    }
}

/// Чем кончилась синхронизация.
#[derive(Debug)]
pub enum Synced {
    /// Здесь и в облаке одно и то же.
    UpToDate,
    /// Мои изменения загружены в облако.
    Uploaded,
    /// Своих изменений не было: взята версия из облака.
    Downloaded,
    /// Изменения с двух сторон слиты и загружены.
    Merged(Merged),
}

/// Сколько синхронизированных версий помнить как базы для слияния.
const MAX_BASES: usize = 2;
/// Сколько раз пробовать заново, если файл в облаке изменился во время записи.
const ATTEMPTS: usize = 3;
const STATE_FILE: &str = "state.json";

/// Что устройство знает об облаке для одного хранилища.
#[derive(Debug, Default, Clone)]
pub struct SyncState {
    /// Версия файла в облаке, которую видели последней.
    version: Option<String>,
    /// Недавние синхронизированные версии: номер версии из файла и байты,
    /// последняя первой.
    bases: Vec<(String, Vec<u8>)>,
}

#[derive(Serialize, Deserialize)]
struct StateFile {
    version: Option<String>,
    bases: Vec<String>,
}

impl SyncState {
    /// Читает состояние из папки. Нет папки — состояние пустое, как до первой
    /// синхронизации.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let text = match fs::read_to_string(dir.join(STATE_FILE)) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => return Err(err),
        };
        let file: StateFile = serde_json::from_str(&text).map_err(io::Error::other)?;
        let mut bases = Vec::new();
        for rev in file.bases {
            // Базы без файла пропускаем: без них слияние просто менее точное.
            if let Ok(bytes) = fs::read(dir.join(base_file(&rev))) {
                bases.push((rev, bytes));
            }
        }
        Ok(Self { version: file.version, bases })
    }

    /// Записывает состояние в папку. Лишние старые базы удаляются.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        for (rev, bytes) in &self.bases {
            let path = dir.join(base_file(rev));
            if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
                write_atomic(&path, bytes)?;
            }
        }
        let file =
            StateFile { version: self.version.clone(), bases: self.bases.iter().map(|(r, _)| r.clone()).collect() };
        write_atomic(&dir.join(STATE_FILE), serde_json::to_string_pretty(&file).map_err(io::Error::other)?.as_bytes())?;
        for entry in fs::read_dir(dir)? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            let known = self.bases.iter().any(|(rev, _)| base_file(rev) == name);
            if name.ends_with(BASE_SUFFIX) && !known {
                let _ = fs::remove_file(dir.join(name));
            }
        }
        Ok(())
    }

    /// Номера версий у баз, которые запомнили, ещё не зная пароля: при подключении
    /// нового устройства файл скачивается до того, как его открыли.
    fn fill_revs(&mut self, vault: &Vault) {
        for (rev, bytes) in &mut self.bases {
            if rev.is_empty()
                && let Ok(doc) = vault.decrypt_backup(bytes)
            {
                *rev = rev_of(&doc);
            }
        }
    }

    fn newest(&self) -> Option<&[u8]> {
        self.bases.first().map(|(_, bytes)| bytes.as_slice())
    }

    /// Запоминает версию в облаке и её байты.
    fn remember(&mut self, version: String, rev: String, bytes: &[u8]) {
        self.version = Some(version);
        self.bases.retain(|(known, _)| *known != rev);
        self.bases.insert(0, (rev, bytes.to_vec()));
        self.bases.truncate(MAX_BASES);
    }

    /// Затёрли ли мою последнюю версию: в истории их файла её нет. Так бывает
    /// у облака без записи «только если не меняли», когда два устройства загрузили
    /// файл почти одновременно. Без истории в файле этого не узнать.
    fn overwritten_by(&self, theirs: &Document) -> bool {
        let Some((mine, _)) = self.bases.first() else { return false };
        match stamp(theirs) {
            Some(their) => !mine.is_empty() && !their.history().any(|rev| rev == mine),
            None => false,
        }
    }

    /// База для слияния с `theirs`: последняя из моих версий, которая есть в их
    /// истории. Если в их файле нет истории (его записала старая версия mdpass или
    /// правили вручную) — моя последняя версия. Иначе общей версии нет.
    fn base_for(&self, vault: &Vault, theirs: &Document) -> Document {
        let found = match stamp(theirs) {
            Some(their) => {
                let history: Vec<&str> = their.history().collect();
                self.bases.iter().find(|(rev, _)| history.contains(&rev.as_str())).map(|(_, b)| b.as_slice())
            }
            None => self.newest(),
        };
        found.and_then(|bytes| vault.decrypt_backup(bytes).ok()).unwrap_or_default()
    }
}

const BASE_SUFFIX: &str = ".base";

fn base_file(rev: &str) -> String {
    let safe: String = rev.chars().filter(char::is_ascii_alphanumeric).collect();
    format!("{safe}{BASE_SUFFIX}")
}

/// Номер версии из файла хранилища. У файла без истории — пустой.
fn rev_of(doc: &Document) -> String {
    stamp(doc).map(|s| s.rev).unwrap_or_default()
}

/// Доступ к хранилищу на время одного шага синхронизации. В приложении шаг идёт под
/// блокировкой сессии, а запросы к облаку — между шагами, без неё: медленная сеть
/// не держит окно. Между шагами хранилище могут изменить или заблокировать.
pub trait Local {
    /// Выполняет шаг. `backup` получает байты локального файла перед тем, как его
    /// заменить. Хранилище уже заблокировали — [`SyncError::Locked`].
    fn with<R>(
        &mut self,
        step: impl FnOnce(&mut Vault, &mut SyncState, &mut Backup) -> Result<R, SyncError>,
    ) -> Result<R, SyncError>;
}

/// Резервная копия локального файла перед заменой.
pub type Backup<'a> = dyn FnMut(&[u8]) -> io::Result<()> + 'a;

/// Что делать после шага с хранилищем.
enum Step {
    Done(Synced),
    /// Загрузить слитую версию поверх версии `expected`.
    Upload {
        bytes: Vec<u8>,
        rev: String,
        expected: String,
        merged: Option<Merged>,
    },
}

/// Синхронизирует хранилище с облаком. Конфликты решаются «оставить обе».
pub fn sync(
    vault: &mut Vault,
    cloud: &mut dyn CloudStore,
    state: &mut SyncState,
    backup: impl FnMut(&[u8]) -> io::Result<()>,
) -> Result<Synced, SyncError> {
    struct Direct<'a, B> {
        vault: &'a mut Vault,
        state: &'a mut SyncState,
        backup: B,
    }
    impl<B: FnMut(&[u8]) -> io::Result<()>> Local for Direct<'_, B> {
        fn with<R>(
            &mut self,
            step: impl FnOnce(&mut Vault, &mut SyncState, &mut Backup) -> Result<R, SyncError>,
        ) -> Result<R, SyncError> {
            step(self.vault, self.state, &mut self.backup)
        }
    }
    sync_with(&mut Direct { vault, state, backup }, cloud)
}

/// Как [`sync`], но с хранилищем через [`Local`]: запросы к облаку идут между шагами.
pub fn sync_with(local: &mut impl Local, cloud: &mut dyn CloudStore) -> Result<Synced, SyncError> {
    local.with(|vault, state, _| {
        if !vault.is_encrypted() {
            return Err(SyncError::NotEncrypted);
        }
        state.fill_revs(vault);
        Ok(())
    })?;
    for _ in 0..ATTEMPTS {
        let (bytes, rev, changed, known) = local.with(|vault, state, _| {
            let changed = state.newest() != Some(vault.bytes());
            Ok((vault.bytes().to_vec(), rev_of(vault.document()), changed, state.version.clone()))
        })?;
        let remote = cloud.version()?;

        // В облаке то же, что видели, или файла нет: загрузить свои правки, если есть.
        if remote.is_none() || remote == known {
            if remote.is_some() && !changed {
                return Ok(Synced::UpToDate);
            }
            match cloud.upload(&bytes, remote.as_deref()) {
                Ok(version) => {
                    local.with(|_, state, _| {
                        state.remember(version, rev, &bytes);
                        Ok(())
                    })?;
                    return Ok(Synced::Uploaded);
                }
                Err(CloudError::Changed) => continue,
                Err(err) => return Err(err.into()),
            }
        }

        let (remote_bytes, version) = cloud.download()?;
        let step = local.with(|vault, state, backup| {
            if remote_bytes == vault.bytes() {
                state.remember(version.clone(), rev_of(vault.document()), &remote_bytes);
                return Ok(Step::Done(Synced::UpToDate));
            }
            let theirs = vault.decrypt_backup(&remote_bytes).map_err(|err| match err {
                VaultError::WrongPassphrase | VaultError::PassphraseRequired => SyncError::OtherPassphrase,
                other => SyncError::Vault(other),
            })?;
            let changed = state.newest() != Some(vault.bytes());
            if !changed && !state.overwritten_by(&theirs) {
                vault.adopt(remote_bytes.clone(), &mut *backup)?;
                state.remember(version.clone(), rev_of(&theirs), &remote_bytes);
                return Ok(Step::Done(Synced::Downloaded));
            }
            let base = state.base_for(vault, &theirs);
            let saved = vault.merge_in(&base, &theirs, &mut *backup, |c| Choice::Both { title: copy_title(c) })?;
            // Слитая версия выросла из их версии: она — новая база, даже если загрузить
            // сейчас не выйдет.
            state.remember(version.clone(), rev_of(&theirs), &remote_bytes);
            Ok(Step::Upload {
                bytes: vault.bytes().to_vec(),
                rev: rev_of(vault.document()),
                expected: version.clone(),
                merged: saved.merged,
            })
        })?;
        let Step::Upload { bytes, rev, expected, merged } = step else {
            let Step::Done(synced) = step else { unreachable!() };
            return Ok(synced);
        };
        match cloud.upload(&bytes, Some(&expected)) {
            Ok(uploaded) => {
                local.with(|_, state, _| {
                    state.remember(uploaded, rev, &bytes);
                    Ok(())
                })?;
                return Ok(Synced::Merged(merged.expect("слияние")));
            }
            Err(CloudError::Changed) => continue,
            Err(err) => return Err(err.into()),
        }
    }
    Err(SyncError::Busy)
}

/// Скачивает хранилище из облака в новый локальный файл: подключение нового
/// устройства. Существующий файл не перезаписывается. После этого файл открывают
/// как обычно, паролем.
pub fn download_new(cloud: &mut dyn CloudStore, path: &Path) -> Result<SyncState, SyncError> {
    let (bytes, version) = cloud.download()?;
    if !crate::vault::is_encrypted(&bytes) {
        return Err(SyncError::NotEncrypted);
    }
    crate::fs_atomic::write_new(path, &bytes).map_err(|err| SyncError::Vault(err.into()))?;
    let mut state = SyncState::default();
    // Номер версии узнаем после открытия: пока файл не расшифрован, база — без номера.
    state.remember(version, String::new(), &bytes);
    Ok(state)
}
