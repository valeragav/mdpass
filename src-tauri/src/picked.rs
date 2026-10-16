//! Файлы из системного диалога. На компьютере диалог отдаёт обычный путь. Android
//! отдаёт адрес `content://…`: открыть его можно только через плагин fs. Такой файл
//! копируется в папку приложения, и дальше приложение работает с копией.

use std::path::Path;

use serde::Deserialize;
use tauri::{AppHandle, Runtime};

use crate::commands::{CommandError, VaultFile, describe};

/// Зачем выбран файл: от этого зависит, куда положить копию.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Purpose {
    /// Хранилище: копия ложится рядом с хранилищем по умолчанию и остаётся.
    Vault,
    /// Файл для импорта: копия временная и удаляется после импорта.
    Import,
}

/// Путь к выбранному файлу. Для адреса `content://` — путь к копии в папке приложения.
#[tauri::command]
pub async fn adopt_file<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    purpose: Purpose,
) -> Result<VaultFile, CommandError> {
    #[cfg(mobile)]
    if path.starts_with("content://") {
        let copy = tauri::async_runtime::spawn_blocking({
            let app = app.clone();
            move || mobile::copy_in(&app, &path, purpose)
        })
        .await
        .map_err(|err| CommandError::Io { message: err.to_string() })??;
        return Ok(describe(&app, &copy));
    }
    let _ = purpose;
    Ok(describe(&app, Path::new(&path)))
}

/// Временная копия файла для импорта: её можно удалить после импорта.
pub fn is_import_copy(path: &Path) -> bool {
    #[cfg(mobile)]
    return mobile::IMPORT_DIR.get().is_some_and(|dir| path.parent() == Some(dir.as_path()));
    #[cfg(desktop)]
    {
        let _ = path;
        false
    }
}

/// Записывает `bytes` в файл, выбранный в диалоге сохранения.
pub fn write_chosen<R: Runtime>(
    app: &AppHandle<R>,
    chosen: tauri_plugin_dialog::FilePath,
    bytes: &[u8],
) -> Result<(), CommandError> {
    let io = |err: std::io::Error| CommandError::Io { message: err.to_string() };
    #[cfg(mobile)]
    {
        use std::io::Write;
        use tauri_plugin_fs::{FsExt, OpenOptions};
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        let mut file = app.fs().open(chosen, options).map_err(io)?;
        file.write_all(bytes).and_then(|()| file.sync_all()).map_err(io)
    }
    #[cfg(desktop)]
    {
        let _ = app;
        let to: std::path::PathBuf = chosen.into_path().map_err(|err| CommandError::Io { message: err.to_string() })?;
        std::fs::write(to, bytes).map_err(io)
    }
}

#[cfg(mobile)]
pub mod mobile {
    use std::io::Read;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use tauri::{AppHandle, Manager, Runtime};
    use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

    use super::Purpose;
    use crate::commands::CommandError;

    /// Папка временных копий для импорта.
    pub static IMPORT_DIR: OnceLock<PathBuf> = OnceLock::new();

    pub fn init(import_dir: PathBuf) {
        let _ = std::fs::remove_dir_all(&import_dir);
        let _ = IMPORT_DIR.set(import_dir);
    }

    pub fn copy_in<R: Runtime>(app: &AppHandle<R>, uri: &str, purpose: Purpose) -> Result<PathBuf, CommandError> {
        let io = |err: std::io::Error| CommandError::Io { message: err.to_string() };
        let url = uri
            .parse()
            .map_err(|_| CommandError::Invalid { message: "Не удалось открыть файл".into() })?;
        let mut options = OpenOptions::new();
        options.read(true);
        let mut bytes = Vec::new();
        app.fs().open(FilePath::Url(url), options).and_then(|mut file| file.read_to_end(&mut bytes)).map_err(io)?;

        let dir = match purpose {
            Purpose::Vault => app.path().app_data_dir().map_err(|err| CommandError::Io { message: err.to_string() })?,
            Purpose::Import => IMPORT_DIR
                .get()
                .cloned()
                .ok_or(CommandError::Io { message: "нет папки для копии".into() })?,
        };
        std::fs::create_dir_all(&dir).map_err(io)?;
        let name = file_name(uri, &bytes, purpose);
        let path = free_path(&dir, &name);
        std::fs::write(&path, &bytes).map_err(io)?;
        Ok(path)
    }

    /// Имя файла из адреса. Если в адресе его нет (например, `…/document/msf%3A42`),
    /// имя выбирается по содержимому.
    fn file_name(uri: &str, bytes: &[u8], purpose: Purpose) -> String {
        let last = uri.rsplit('/').next().unwrap_or_default();
        let decoded = percent_decode(last);
        let name = decoded.rsplit([':', '/']).next().unwrap_or_default().trim();
        let known = [".md", ".md.age", ".age", ".markdown", ".txt"];
        if !name.is_empty() && known.iter().any(|ext| name.ends_with(ext)) {
            return name.to_string();
        }
        match purpose {
            Purpose::Vault if bytes.starts_with(b"age-encryption.org/") => "mdpass.md.age".into(),
            Purpose::Vault => "mdpass.md".into(),
            Purpose::Import => "import.md".into(),
        }
    }

    /// Не затирает файл, который уже есть: `mdpass.md.age` → `mdpass (2).md.age`.
    fn free_path(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        if !path.exists() {
            return path;
        }
        let (stem, ext) = match name.find('.') {
            Some(dot) => name.split_at(dot),
            None => (name, ""),
        };
        (2..)
            .map(|n| dir.join(format!("{stem} ({n}){ext}")))
            .find(|path| !path.exists())
            .expect("свободное имя найдётся")
    }

    fn percent_decode(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%'
                && let Some(byte) = text.get(i + 1..i + 3).and_then(|hex| u8::from_str_radix(hex, 16).ok())
            {
                out.push(byte);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }
}
