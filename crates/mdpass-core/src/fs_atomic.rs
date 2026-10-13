//! Запись файлов целиком или никак: данные пишутся во временный файл в той же папке,
//! сбрасываются на диск и заменяют цель одним переименованием.

use std::io::{self, Write};
use std::path::Path;

fn temp_file_near(path: &Path) -> io::Result<tempfile::NamedTempFile> {
    let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty()).unwrap_or(Path::new("."));
    tempfile::Builder::new().prefix(".mdpass-").suffix(".tmp").tempfile_in(dir)
}

/// Создаёт файл. Если он уже есть, возвращает ошибку `AlreadyExists` и ничего не меняет.
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temp = temp_file_near(path)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path).map_err(|err| err.error)?;
    sync_parent(path);
    Ok(())
}

/// Создаёт или заменяет файл.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temp = temp_file_near(path)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    persist_retrying(temp, path)?;
    sync_parent(path);
    Ok(())
}

/// Замена файла. На Windows её ненадолго блокирует антивирус или клиент синхронизации,
/// который как раз читает файл: тогда пробуем ещё несколько раз.
fn persist_retrying(mut temp: tempfile::NamedTempFile, path: &Path) -> io::Result<()> {
    for _ in 0..5 {
        match temp.persist(path) {
            Ok(_) => return Ok(()),
            Err(err) if err.error.kind() == io::ErrorKind::PermissionDenied => {
                temp = err.file;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(err) => return Err(err.error),
        }
    }
    temp.persist(path).map(drop).map_err(|err| err.error)
}

/// На Unix переименование надёжно попадает на диск только после синхронизации папки.
fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent()
        && let Ok(dir) = std::fs::File::open(dir)
    {
        let _ = dir.sync_all();
    }
    #[cfg(not(unix))]
    let _ = path;
}
