//! Хранилище на диске: Markdown-документ, зашифрованный паролем в формате age (scrypt).
//!
//! Файл — обычный age, его можно расшифровать утилитой `age -d`. Запись атомарная:
//! новый файл пишется рядом во временный и заменяет старый одним переименованием.
//! Перед записью приложение сверяет файл с тем, что записало само в последний раз,
//! чтобы не затереть изменения, сделанные вне приложения.

use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::iter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use age::secrecy::{ExposeSecret, SecretString};
use age::{DecryptError, Decryptor, Encryptor};
use zeroize::Zeroizing;

use crate::format::Document;

/// Мастер-пароль. Стирается из памяти при удалении.
pub type Passphrase = SecretString;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultOptions {
    /// Сложность scrypt, log2(N). Каждая единица вдвое замедляет подбор пароля
    /// и открытие хранилища.
    pub work_factor: u8,
}

impl Default for VaultOptions {
    /// 2^18: около секунды на открытие, 256 МБ памяти на каждую попытку подбора.
    fn default() -> Self {
        Self { work_factor: 18 }
    }
}

#[derive(Debug)]
pub enum VaultError {
    EmptyPassphrase,
    AlreadyExists,
    WrongPassphrase,
    /// Файл повреждён, изменён или это не хранилище. В строке — техническая причина.
    Damaged(String),
    /// Файл на диске изменился с момента открытия или последней записи.
    ChangedOnDisk,
    Io(io::Error),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPassphrase => write!(f, "Мастер-пароль не может быть пустым"),
            Self::AlreadyExists => write!(f, "Файл уже существует"),
            Self::WrongPassphrase => write!(f, "Неверный мастер-пароль"),
            Self::Damaged(reason) => write!(f, "Файл повреждён или изменён вне приложения: {reason}"),
            Self::ChangedOnDisk => write!(f, "Файл хранилища изменён вне приложения"),
            Self::Io(err) => write!(f, "Ошибка доступа к файлу: {err}"),
        }
    }
}

impl std::error::Error for VaultError {}

impl From<io::Error> for VaultError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

pub struct Vault {
    path: PathBuf,
    passphrase: Passphrase,
    options: VaultOptions,
    document: Document,
    /// Байты файла, которые приложение прочитало или записало последними.
    on_disk: Vec<u8>,
}

impl Vault {
    /// Создаёт пустое хранилище. Существующий файл не перезаписывается.
    pub fn create(path: &Path, passphrase: Passphrase, options: VaultOptions) -> Result<Self, VaultError> {
        check_passphrase(&passphrase)?;
        let document = Document::default();
        let bytes = encrypt(&Zeroizing::new(document.to_markdown()), &passphrase, options)?;
        write_new(path, &bytes)?;
        Ok(Self { path: path.to_path_buf(), passphrase, options, document, on_disk: bytes })
    }

    pub fn open(path: &Path, passphrase: Passphrase) -> Result<Self, VaultError> {
        let bytes = fs::read(path)?;
        let markdown = decrypt(&bytes, &passphrase)?;
        // Сохраняем ту сложность scrypt, с которой файл был зашифрован.
        let options =
            scrypt_work_factor(&bytes).map_or_else(VaultOptions::default, |work_factor| VaultOptions { work_factor });
        Ok(Self { path: path.to_path_buf(), passphrase, options, document: Document::parse(&markdown), on_disk: bytes })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    pub fn passphrase_matches(&self, candidate: &Passphrase) -> bool {
        self.passphrase.expose_secret() == candidate.expose_secret()
    }

    /// Шифрует документ и записывает файл.
    pub fn save(&mut self) -> Result<(), VaultError> {
        let bytes = encrypt(&Zeroizing::new(self.document.to_markdown()), &self.passphrase, self.options)?;
        self.replace_file(bytes)
    }

    /// Шифрует файл новым паролем и сразу записывает его.
    pub fn change_passphrase(&mut self, passphrase: Passphrase, options: VaultOptions) -> Result<(), VaultError> {
        check_passphrase(&passphrase)?;
        let bytes = encrypt(&Zeroizing::new(self.document.to_markdown()), &passphrase, options)?;
        self.replace_file(bytes)?;
        self.passphrase = passphrase;
        self.options = options;
        Ok(())
    }

    fn replace_file(&mut self, bytes: Vec<u8>) -> Result<(), VaultError> {
        let current = match fs::read(&self.path) {
            Ok(current) => current,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(VaultError::ChangedOnDisk),
            Err(err) => return Err(err.into()),
        };
        if current != self.on_disk {
            return Err(VaultError::ChangedOnDisk);
        }
        write_atomic(&self.path, &bytes)?;
        self.on_disk = bytes;
        Ok(())
    }
}

/// Пауза после неверного пароля: замедляет подбор пароля вручную.
#[derive(Debug, Clone)]
pub struct AttemptLimiter {
    delay: Duration,
    blocked_until: Option<Instant>,
}

impl AttemptLimiter {
    pub fn new(delay: Duration) -> Self {
        Self { delay, blocked_until: None }
    }

    /// Сколько ещё ждать до следующей попытки.
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.blocked_until.filter(|until| *until > now).map(|until| until - now)
    }

    pub fn failed(&mut self, now: Instant) {
        self.blocked_until = Some(now + self.delay);
    }

    pub fn succeeded(&mut self) {
        self.blocked_until = None;
    }
}

fn check_passphrase(passphrase: &Passphrase) -> Result<(), VaultError> {
    if passphrase.expose_secret().is_empty() { Err(VaultError::EmptyPassphrase) } else { Ok(()) }
}

fn encrypt(markdown: &str, passphrase: &Passphrase, options: VaultOptions) -> Result<Vec<u8>, VaultError> {
    let mut recipient = age::scrypt::Recipient::new(passphrase.clone());
    recipient.set_work_factor(options.work_factor);
    let encryptor = Encryptor::with_recipients(iter::once(&recipient as &dyn age::Recipient))
        .map_err(|err| io::Error::other(err.to_string()))?;
    let mut out = Vec::new();
    let mut writer = encryptor.wrap_output(&mut out)?;
    writer.write_all(markdown.as_bytes())?;
    writer.finish()?;
    Ok(out)
}

fn decrypt(bytes: &[u8], passphrase: &Passphrase) -> Result<Zeroizing<String>, VaultError> {
    let damaged = |err: &dyn fmt::Display| VaultError::Damaged(err.to_string());
    let decryptor = Decryptor::new_buffered(bytes).map_err(|err| damaged(&err))?;
    if !decryptor.is_scrypt() {
        return Err(VaultError::Damaged("файл зашифрован не паролем".into()));
    }
    let identity = age::scrypt::Identity::new(passphrase.clone());
    let mut reader = decryptor.decrypt(iter::once(&identity as &dyn age::Identity)).map_err(|err| match err {
        DecryptError::DecryptionFailed | DecryptError::KeyDecryptionFailed => VaultError::WrongPassphrase,
        other => damaged(&other),
    })?;
    let mut plain = Zeroizing::new(Vec::new());
    reader.read_to_end(&mut plain).map_err(|err| damaged(&err))?;
    match String::from_utf8(std::mem::take(&mut *plain)) {
        Ok(text) => Ok(Zeroizing::new(text)),
        Err(err) => {
            drop(Zeroizing::new(err.into_bytes()));
            Err(VaultError::Damaged("внутри не текст UTF-8".into()))
        }
    }
}

/// Сложность scrypt из заголовка age: строка `-> scrypt <соль> <log2(N)>`.
fn scrypt_work_factor(bytes: &[u8]) -> Option<u8> {
    let marker = b"\n-> scrypt ";
    let start = bytes.windows(marker.len()).position(|w| w == marker)? + 1;
    let line = bytes[start..].split(|&b| b == b'\n').next()?;
    std::str::from_utf8(line).ok()?.split(' ').nth(3)?.parse().ok()
}

fn temp_file_near(path: &Path) -> io::Result<tempfile::NamedTempFile> {
    let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty()).unwrap_or(Path::new("."));
    tempfile::Builder::new().prefix(".mdpass-").suffix(".tmp").tempfile_in(dir)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let mut temp = temp_file_near(path)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path).map_err(|err| match err.error.kind() {
        io::ErrorKind::AlreadyExists => VaultError::AlreadyExists,
        _ => VaultError::Io(err.error),
    })?;
    sync_parent(path);
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temp = temp_file_near(path)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|err| err.error)?;
    sync_parent(path);
    Ok(())
}

/// На Unix переименование надёжно попадает на диск только после синхронизации папки.
fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent()
        && let Ok(dir) = fs::File::open(dir)
    {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = path;
}
