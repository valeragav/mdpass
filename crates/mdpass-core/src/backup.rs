//! Резервные копии хранилища.
//!
//! Перед каждой записью файл в том виде, в каком он лежал на диске, копируется
//! в папку приложения: `<папка копий>/vault.md.age-<хеш пути>/`. Хеш полного пути
//! не даёт смешаться копиям одноимённых хранилищ из разных папок. Копия защищена
//! так же, как файл в момент копирования. В имени копии только время и вид
//! действия, названий записей там нет.
//!
//! Раньше копии лежали рядом с хранилищем, в `.vault.md.age.backups/`. Такие
//! папки забираются в папку приложения при первом обращении.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::fs_atomic::{write_atomic, write_new};
use crate::vault::{Passphrase, Vault, VaultError, decrypt_file};

const EXTENSION: &str = ".age";

/// Перед каким действием сделана копия.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupKind {
    Add,
    Update,
    Delete,
    Passphrase,
    Restore,
    Import,
    Move,
}

impl BackupKind {
    const ALL: [Self; 7] =
        [Self::Add, Self::Update, Self::Delete, Self::Passphrase, Self::Restore, Self::Import, Self::Move];

    fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Passphrase => "passphrase",
            Self::Restore => "restore",
            Self::Import => "import",
            Self::Move => "move",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupInfo {
    /// Имя файла копии. По нему копию читают и восстанавливают.
    pub id: String,
    pub created: SystemTime,
    pub kind: BackupKind,
}

#[derive(Debug, Clone)]
pub struct BackupStore {
    dir: PathBuf,
}

impl BackupStore {
    /// Копии в папке приложения `root`: своя подпапка у каждого хранилища.
    pub fn in_root(root: &Path, vault_path: &Path) -> Self {
        let name = vault_path.file_name().map_or_else(|| "vault".into(), |n| n.to_string_lossy().into_owned());
        Self { dir: root.join(format!("{name}-{}", path_key(vault_path))) }
    }

    /// Старое место копий: скрытая папка рядом с хранилищем.
    pub fn for_vault(vault_path: &Path) -> Self {
        let name = vault_path.file_name().map_or_else(|| "vault".into(), |n| n.to_string_lossy().into_owned());
        let parent = vault_path.parent().unwrap_or(Path::new("."));
        Self { dir: parent.join(format!(".{name}.backups")) }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Копии от новых к старым. Чужие файлы в папке пропускаются.
    pub fn list(&self) -> io::Result<Vec<BackupInfo>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err),
        };
        let mut list = Vec::new();
        for entry in entries {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if let Some(info) = parse_name(&name) {
                list.push(info);
            }
        }
        list.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
        Ok(list)
    }

    pub fn save(&self, bytes: &[u8], kind: BackupKind, now: SystemTime) -> io::Result<BackupInfo> {
        fs::create_dir_all(&self.dir)?;
        let stamp = format_stamp(now);
        for attempt in 0u32.. {
            let suffix = if attempt == 0 { String::new() } else { format!("-{attempt}") };
            let id = format!("{stamp}-{}{suffix}{EXTENSION}", kind.as_str());
            match write_new(&self.dir.join(&id), bytes) {
                Ok(()) => return Ok(BackupInfo { id, created: truncate_to_millis(now), kind }),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(err),
            }
        }
        unreachable!("бесконечный перебор суффиксов")
    }

    pub fn read(&self, id: &str) -> io::Result<Vec<u8>> {
        fs::read(self.path_of(id)?)
    }

    /// Копирует файл копии в другое место, например по «Сохранить как файл…».
    pub fn export(&self, id: &str, to: &Path) -> io::Result<()> {
        fs::copy(self.path_of(id)?, to).map(drop)
    }

    /// Переносит копии в другую папку копий, например когда файл хранилища
    /// переименовали. Копии, которые там уже есть, не перезаписываются.
    pub fn move_to(&self, target: &Self) -> io::Result<()> {
        move_dir(&self.dir, &target.dir)
    }

    /// Удаляет старые копии, оставляя `keep` самых новых.
    pub fn prune(&self, keep: usize) -> io::Result<()> {
        for old in self.list()?.into_iter().skip(keep) {
            fs::remove_file(self.dir.join(&old.id))?;
        }
        Ok(())
    }

    /// Путь к копии. Принимает только имена, которые создаёт само хранилище копий:
    /// так из интерфейса нельзя прочитать произвольный файл.
    fn path_of(&self, id: &str) -> io::Result<PathBuf> {
        if parse_name(id).is_none() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "нет такой резервной копии"));
        }
        Ok(self.dir.join(id))
    }
}

/// Восстанавливает хранилище из копии, когда сам файл не открывается.
///
/// Копия сначала проверяется паролем; открытой копии пароль не нужен. Повреждённый файл не удаляется: он
/// переименовывается в `<имя>.damaged-<время>` рядом с хранилищем.
pub fn restore_damaged(
    vault_path: &Path,
    store: &BackupStore,
    id: &str,
    passphrase: Option<Passphrase>,
    now: SystemTime,
) -> Result<Vault, VaultError> {
    let bytes = store.read(id)?;
    decrypt_file(&bytes, passphrase.as_ref())?;
    if vault_path.exists() {
        let name = vault_path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let aside = vault_path.with_file_name(format!("{name}.damaged-{}", format_stamp(now)));
        fs::rename(vault_path, aside)?;
    }
    write_atomic(vault_path, &bytes)?;
    match passphrase {
        Some(passphrase) => Vault::open(vault_path, passphrase),
        None => Vault::open_plain(vault_path),
    }
}

/// Короткий устойчивый отпечаток пути: FNV-1a от полного пути. На Windows регистр
/// букв в пути не важен, поэтому путь приводится к нижнему регистру.
fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = if cfg!(windows) { text.to_lowercase().replace('/', "\\") } else { text.into_owned() };
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:08x}", hash >> 32)
}

/// Переносит папку целиком. Если переименовать нельзя (другой диск или папка
/// уже есть), файлы копируются по одному, существующие не перезаписываются.
fn move_dir(from: &Path, to: &Path) -> io::Result<()> {
    if !from.is_dir() || from == to {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if !to.exists() && fs::rename(from, to).is_ok() {
        return Ok(());
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_file() && !target.exists() {
            fs::copy(entry.path(), &target)?;
        }
    }
    fs::remove_dir_all(from)
}

/// `20261012T114203123Z-update.age` или с суффиксом `…-update-1.age`.
fn parse_name(name: &str) -> Option<BackupInfo> {
    let stem = name.strip_suffix(EXTENSION)?;
    let (stamp, rest) = stem.split_once('-')?;
    let kind_text = match rest.split_once('-') {
        Some((kind, suffix)) if suffix.chars().all(|c| c.is_ascii_digit()) && !suffix.is_empty() => kind,
        Some(_) => return None,
        None => rest,
    };
    Some(BackupInfo { id: name.to_string(), created: parse_stamp(stamp)?, kind: BackupKind::parse(kind_text)? })
}

fn truncate_to_millis(time: SystemTime) -> SystemTime {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    UNIX_EPOCH + Duration::from_millis(u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

/// Время в UTC без зависимостей от библиотек дат: `YYYYMMDDTHHMMSSmmmZ`.
fn format_stamp(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs();
    let (year, month, day) = civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0));
    let rem = secs % 86_400;
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        since.subsec_millis()
    )
}

fn parse_stamp(stamp: &str) -> Option<SystemTime> {
    let bytes = stamp.as_bytes();
    if bytes.len() != 19 || bytes[8] != b'T' || bytes[18] != b'Z' {
        return None;
    }
    let num = |range: std::ops::Range<usize>| -> Option<u64> {
        let part = stamp.get(range)?;
        part.chars().all(|c| c.is_ascii_digit()).then(|| part.parse().ok())?
    };
    let (year, month, day) = (num(0..4)?, num(4..6)?, num(6..8)?);
    let (hour, minute, second, millis) = (num(9..11)?, num(11..13)?, num(13..15)?, num(15..18)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = u64::try_from(days_from_civil(i64::try_from(year).ok()?, month, day)).ok()?;
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second;
    Some(UNIX_EPOCH + Duration::from_secs(secs) + Duration::from_millis(millis))
}

/// Дата по числу дней от 1970-01-01. Алгоритм Говарда Хиннанта.
fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u64;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u64;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn days_from_civil(year: i64, month: u64, day: u64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let month = month as i64;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_round_trips_through_the_file_name() {
        for secs in [0u64, 951_782_400, 1_791_712_345, 4_102_444_799] {
            let time = UNIX_EPOCH + Duration::from_millis(secs * 1000 + 789);
            assert_eq!(parse_stamp(&format_stamp(time)), Some(time), "{}", format_stamp(time));
        }
        assert_eq!(format_stamp(UNIX_EPOCH + Duration::from_secs(951_782_400)), "20000229T000000000Z");
    }

    #[test]
    fn names_that_are_not_backups_are_rejected() {
        for name in
            ["x.age", "20261012T114203123Z-nope.age", "20261012T114203123Z-add-x.age", "20261012T114203123Z-add.txt"]
        {
            assert!(parse_name(name).is_none(), "{name}");
        }
        assert!(parse_name("20261012T114203123Z-add-2.age").is_some());
    }
}
