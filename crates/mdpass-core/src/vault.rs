//! Хранилище на диске: Markdown-документ, зашифрованный паролем в формате age (scrypt),
//! или обычный открытый Markdown-файл, если шифрование выключено.
//!
//! Зашифрованный файл — обычный age, его можно расшифровать утилитой `age -d`.
//! Запись атомарная: новый файл пишется рядом во временный и заменяет старый одним
//! переименованием. Перед записью приложение сверяет файл с тем, что записало само
//! в последний раз, чтобы не затереть изменения, сделанные вне приложения.

use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::iter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use age::secrecy::{ExposeSecret, SecretString};
use age::{DecryptError, Decryptor, Encryptor};
use zeroize::Zeroizing;

use crate::format::{Document, FORMAT_VERSION};
use crate::fs_atomic::{write_atomic, write_new};

/// Мастер-пароль. Стирается из памяти при удалении.
pub type Passphrase = SecretString;

/// Начало каждого файла age.
const AGE_MAGIC: &[u8] = b"age-encryption.org/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultOptions {
    /// Сложность scrypt, log2(N). Каждая единица вдвое замедляет подбор пароля
    /// и открытие хранилища.
    pub work_factor: u8,
}

impl VaultOptions {
    /// 2^18: около секунды на открытие, 256 МБ памяти на каждую попытку подбора.
    pub const STRONG: Self = Self { work_factor: 18 };
    /// 2^12: файл открывается мгновенно, но подбор пароля в 64 раза дешевле.
    pub const WEAK: Self = Self { work_factor: 12 };

    /// Считается ли такая сложность надёжной.
    pub fn is_strong(self) -> bool {
        self.work_factor >= 16
    }
}

impl Default for VaultOptions {
    fn default() -> Self {
        Self::STRONG
    }
}

#[derive(Debug)]
pub enum VaultError {
    EmptyPassphrase,
    AlreadyExists,
    WrongPassphrase,
    /// Файл зашифрован, а пароль не передан.
    PassphraseRequired,
    /// Файл записан более новой версией mdpass: эта версия может его испортить.
    NewerFormat(u32),
    /// Файл повреждён, изменён или это не хранилище. В строке — техническая причина.
    Damaged(String),
    /// Файл на диске изменился с момента открытия или последней записи.
    ChangedOnDisk,
    /// Восстанавливать «повреждённый» файл не нужно: он открывается.
    NotDamaged,
    Io(io::Error),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPassphrase => write!(f, "Мастер-пароль не может быть пустым"),
            Self::AlreadyExists => write!(f, "Файл уже существует"),
            Self::WrongPassphrase => write!(f, "Неверный мастер-пароль"),
            Self::PassphraseRequired => write!(f, "Файл зашифрован: нужен мастер-пароль"),
            Self::NewerFormat(version) => {
                write!(f, "Файл записан более новой версией mdpass (формат {version}). Обновите приложение")
            }
            Self::Damaged(reason) => write!(f, "Файл повреждён или изменён вне приложения: {reason}"),
            Self::ChangedOnDisk => write!(f, "Файл хранилища изменён вне приложения"),
            Self::NotDamaged => write!(f, "Файл открывается: восстанавливать его из копии не нужно"),
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

/// Пароль и сложность шифрования. Нет ключа — файл хранится открытым.
struct Key {
    passphrase: Passphrase,
    options: VaultOptions,
}

pub struct Vault {
    path: PathBuf,
    key: Option<Key>,
    document: Document,
    /// Байты файла, которые приложение прочитало или записало последними.
    on_disk: Vec<u8>,
}

/// Зашифрованы ли байты файла.
pub fn is_encrypted(bytes: &[u8]) -> bool {
    bytes.starts_with(AGE_MAGIC)
}

/// Зашифрован ли файл на диске. Читает только начало файла.
pub fn file_is_encrypted(path: &Path) -> io::Result<bool> {
    let mut start = [0u8; AGE_MAGIC.len()];
    let mut file = fs::File::open(path)?;
    let mut read = 0;
    while read < start.len() {
        match file.read(&mut start[read..])? {
            0 => break,
            n => read += n,
        }
    }
    Ok(is_encrypted(&start[..read]))
}

impl Vault {
    /// Создаёт пустое зашифрованное хранилище. Существующий файл не перезаписывается.
    pub fn create(path: &Path, passphrase: Passphrase, options: VaultOptions) -> Result<Self, VaultError> {
        check_passphrase(&passphrase)?;
        Self::create_with(path, Some(Key { passphrase, options }))
    }

    /// Создаёт пустое хранилище без шифрования: обычный Markdown-файл.
    pub fn create_plain(path: &Path) -> Result<Self, VaultError> {
        Self::create_with(path, None)
    }

    fn create_with(path: &Path, key: Option<Key>) -> Result<Self, VaultError> {
        let mut document = Document::default();
        document.ensure_header();
        let bytes = encode(&document, key.as_ref())?;
        write_new(path, &bytes).map_err(|err| match err.kind() {
            io::ErrorKind::AlreadyExists => VaultError::AlreadyExists,
            _ => VaultError::Io(err),
        })?;
        Ok(Self { path: path.to_path_buf(), key, document, on_disk: bytes })
    }

    /// Открывает хранилище. Открытый файл открывается и так, пароль тогда не нужен.
    pub fn open(path: &Path, passphrase: Passphrase) -> Result<Self, VaultError> {
        Self::open_with(path, Some(passphrase))
    }

    /// Открывает хранилище без шифрования. Зашифрованный файл даёт
    /// [`VaultError::PassphraseRequired`].
    pub fn open_plain(path: &Path) -> Result<Self, VaultError> {
        Self::open_with(path, None)
    }

    fn open_with(path: &Path, passphrase: Option<Passphrase>) -> Result<Self, VaultError> {
        let bytes = fs::read(path)?;
        if !is_encrypted(&bytes) {
            let document = checked(Document::parse(&plain_text(&bytes)?))?;
            return Ok(Self { path: path.to_path_buf(), key: None, document, on_disk: bytes });
        }
        let passphrase = passphrase.ok_or(VaultError::PassphraseRequired)?;
        let markdown = decrypt(&bytes, &passphrase)?;
        let document = checked(Document::parse(&markdown))?;
        // Сохраняем ту сложность scrypt, с которой файл был зашифрован.
        let options =
            scrypt_work_factor(&bytes).map_or_else(VaultOptions::default, |work_factor| VaultOptions { work_factor });
        Ok(Self { path: path.to_path_buf(), key: Some(Key { passphrase, options }), document, on_disk: bytes })
    }

    /// Перечитывает файл с диска с текущим паролем: после того как файл изменили вне
    /// приложения, например синхронизацией. Несохранённых изменений у хранилища не
    /// бывает, поэтому ничего не теряется. Если файл перешифровали другим паролем,
    /// возвращает [`VaultError::WrongPassphrase`] и остаётся прежним.
    pub fn reload(&mut self) -> Result<(), VaultError> {
        let reopened = match &self.key {
            Some(key) => Self::open_with(&self.path, Some(key.passphrase.clone()))?,
            None => Self::open_plain(&self.path)?,
        };
        *self = reopened;
        Ok(())
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

    /// Сложность шифрования. `None` — файл не зашифрован.
    pub fn options(&self) -> Option<VaultOptions> {
        self.key.as_ref().map(|key| key.options)
    }

    pub fn is_encrypted(&self) -> bool {
        self.key.is_some()
    }

    /// Совпадает ли пароль с текущим. У открытого файла пароля нет: всегда `false`.
    pub fn passphrase_matches(&self, candidate: &Passphrase) -> bool {
        self.key.as_ref().is_some_and(|key| key.passphrase.expose_secret() == candidate.expose_secret())
    }

    /// Записывает файл с текущей защитой.
    pub fn save(&mut self) -> Result<(), VaultError> {
        self.save_with_backup(|_| Ok(()))
    }

    /// Как [`save`](Self::save), но сначала передаёт `backup` байты файла, который
    /// сейчас лежит на диске. Если копия не удалась, файл не перезаписывается.
    pub fn save_with_backup(&mut self, backup: impl FnOnce(&[u8]) -> io::Result<()>) -> Result<(), VaultError> {
        self.document.ensure_header();
        let bytes = encode(&self.document, self.key.as_ref())?;
        self.replace_file(bytes, backup)
    }

    /// Шифрует файл новым паролем и сразу записывает его.
    pub fn change_passphrase(&mut self, passphrase: Passphrase, options: VaultOptions) -> Result<(), VaultError> {
        self.set_protection(Some((passphrase, options)), |_| Ok(()))
    }

    /// Меняет защиту файла и сразу записывает его: новый пароль или сложность,
    /// `None` — снять шифрование.
    pub fn set_protection(
        &mut self,
        protection: Option<(Passphrase, VaultOptions)>,
        backup: impl FnOnce(&[u8]) -> io::Result<()>,
    ) -> Result<(), VaultError> {
        if let Some((passphrase, _)) = &protection {
            check_passphrase(passphrase)?;
        }
        let key = protection.map(|(passphrase, options)| Key { passphrase, options });
        self.document.ensure_header();
        let bytes = encode(&self.document, key.as_ref())?;
        self.replace_file(bytes, backup)?;
        self.key = key;
        Ok(())
    }

    /// Читает резервную копию. Открытая копия читается всегда, зашифрованная —
    /// текущим паролем; копия, сделанная до смены пароля, даёт
    /// [`VaultError::WrongPassphrase`].
    pub fn decrypt_backup(&self, bytes: &[u8]) -> Result<Document, VaultError> {
        match &self.key {
            Some(key) => decrypt_file(bytes, Some(&key.passphrase)),
            None if is_encrypted(bytes) => Err(VaultError::WrongPassphrase),
            None => decrypt_file(bytes, None),
        }
    }

    /// Заменяет содержимое хранилища документом из резервной копии. Файл пишется
    /// с текущей защитой: открытая старая копия не снимет шифрование.
    pub fn restore(
        &mut self,
        mut document: Document,
        backup: impl FnOnce(&[u8]) -> io::Result<()>,
    ) -> Result<(), VaultError> {
        document.ensure_header();
        let bytes = encode(&document, self.key.as_ref())?;
        self.replace_file(bytes, backup)?;
        self.document = document;
        Ok(())
    }

    /// Имя файла, которое подходит к защите: `vault.md.age` для зашифрованного,
    /// `vault.md` для открытого. Если имя уже подходит, возвращает текущий путь.
    pub fn conventional_path(&self) -> PathBuf {
        let name = self.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let wanted = match (self.is_encrypted(), name.strip_suffix(".age")) {
            (true, None) => format!("{name}.age"),
            (false, Some(plain)) if !plain.is_empty() => plain.to_string(),
            _ => return self.path.clone(),
        };
        self.path.with_file_name(wanted)
    }

    /// Переименовывает файл хранилища. Чужой файл с таким именем не перезаписывается.
    pub fn rename_to(&mut self, path: &Path) -> Result<(), VaultError> {
        if path == self.path {
            return Ok(());
        }
        if path.exists() {
            return Err(VaultError::AlreadyExists);
        }
        fs::rename(&self.path, path)?;
        self.path = path.to_path_buf();
        Ok(())
    }

    fn replace_file(&mut self, bytes: Vec<u8>, backup: impl FnOnce(&[u8]) -> io::Result<()>) -> Result<(), VaultError> {
        let current = match fs::read(&self.path) {
            Ok(current) => current,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(VaultError::ChangedOnDisk),
            Err(err) => return Err(err.into()),
        };
        if current != self.on_disk {
            return Err(VaultError::ChangedOnDisk);
        }
        backup(&current)?;
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

/// Читает файл хранилища, не открывая его: открытый как есть, зашифрованный —
/// паролем. Нужно, чтобы проверить резервную копию перед восстановлением.
pub fn decrypt_file(bytes: &[u8], passphrase: Option<&Passphrase>) -> Result<Document, VaultError> {
    if !is_encrypted(bytes) {
        return checked(Document::parse(&plain_text(bytes)?));
    }
    let passphrase = passphrase.ok_or(VaultError::PassphraseRequired)?;
    checked(Document::parse(&decrypt(bytes, passphrase)?))
}

/// Отказывается открывать файл, записанный более новой версией формата.
fn checked(document: Document) -> Result<Document, VaultError> {
    match document.format_version() {
        Some(version) if version > FORMAT_VERSION => Err(VaultError::NewerFormat(version)),
        _ => Ok(document),
    }
}

fn check_passphrase(passphrase: &Passphrase) -> Result<(), VaultError> {
    if passphrase.expose_secret().is_empty() { Err(VaultError::EmptyPassphrase) } else { Ok(()) }
}

/// Байты файла для документа: зашифрованные, если есть ключ, иначе текст как есть.
fn encode(document: &Document, key: Option<&Key>) -> Result<Vec<u8>, VaultError> {
    let markdown = Zeroizing::new(document.to_markdown());
    match key {
        Some(key) => encrypt(&markdown, &key.passphrase, key.options),
        None => Ok(markdown.as_bytes().to_vec()),
    }
}

pub(crate) fn plain_text(bytes: &[u8]) -> Result<Zeroizing<String>, VaultError> {
    std::str::from_utf8(bytes)
        .map(|text| Zeroizing::new(text.to_owned()))
        .map_err(|_| VaultError::Damaged("это не хранилище: файл не зашифрован и не текст UTF-8".into()))
}

pub(crate) fn encrypt(markdown: &str, passphrase: &Passphrase, options: VaultOptions) -> Result<Vec<u8>, VaultError> {
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
