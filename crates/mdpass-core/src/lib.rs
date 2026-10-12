//! Ядро mdpass. Здесь вся работа с хранилищем без зависимости от Tauri:
//! формат Markdown, шифрование, запись файла и настройки приложения.

pub mod backup;
pub mod config;
pub mod diff;
pub mod format;
mod fs_atomic;
pub mod password;
pub mod vault;
