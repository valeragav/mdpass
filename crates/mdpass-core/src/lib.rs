//! Ядро mdpass. Здесь вся работа с хранилищем без зависимости от Tauri:
//! формат Markdown, шифрование, запись файла и настройки приложения.

pub mod config;
pub mod format;
mod fs_atomic;
pub mod vault;
