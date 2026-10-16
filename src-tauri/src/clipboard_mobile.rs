//! Буфер обмена на телефоне: через плагин Tauri, с очисткой через заданное время,
//! при блокировке и выходе — как на компьютере (`clipboard.rs`).
//!
//! Плагину нужен `AppHandle`: его запоминает [`init`] при запуске приложения.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Wry};
use tauri_plugin_clipboard_manager::ClipboardExt;
use zeroize::Zeroizing;

static APP: OnceLock<AppHandle<Wry>> = OnceLock::new();

/// Номер копирования: таймер прежнего копирования не должен стереть новое значение.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Последнее скопированное значение и его номер.
static LAST: Mutex<Option<(u64, Zeroizing<String>)>> = Mutex::new(None);

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Запоминает приложение: без него до буфера не добраться.
pub fn init(app: AppHandle<Wry>) {
    let _ = APP.set(app);
}

fn app() -> Result<&'static AppHandle<Wry>, Error> {
    APP.get().ok_or_else(|| Error("буфер обмена ещё не готов".into()))
}

/// Копирует значение. Если `clear_after` задано, через это время буфер очистится,
/// а при блокировке и выходе — сразу.
pub fn copy(value: Zeroizing<String>, clear_after: Option<Duration>) -> Result<(), Error> {
    app()?.clipboard().write_text(value.as_str()).map_err(|err| Error(err.to_string()))?;
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    *LAST.lock().unwrap_or_else(|p| p.into_inner()) = Some((generation, value));
    if let Some(clear_after) = clear_after {
        std::thread::spawn(move || {
            std::thread::sleep(clear_after);
            clear_if_current(Some(generation));
        });
    }
    Ok(())
}

/// Очищает буфер, если в нём всё ещё наше значение.
pub fn clear_ours() {
    clear_if_current(None);
}

fn clear_if_current(generation: Option<u64>) {
    let Ok(app) = app() else { return };
    let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
    let Some((current, value)) = last.as_ref() else { return };
    if generation.is_some_and(|g| g != *current) {
        return;
    }
    // Свернутому приложению Android может не дать прочитать буфер: тогда очищаем
    // и так — лучше стереть чужое, чем оставить пароль.
    let ours = match app.clipboard().read_text() {
        Ok(text) => Zeroizing::new(text).as_str() == value.as_str(),
        Err(_) => true,
    };
    if ours {
        let _ = app.clipboard().write_text(String::new());
    }
    *last = None;
}
