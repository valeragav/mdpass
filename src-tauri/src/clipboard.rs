//! Копирование значений в буфер обмена с очисткой через заданное время.
//!
//! Значение помечается как «не сохранять в журнал буфера», чтобы оно не попало
//! в историю Windows (Win+V), облачный буфер и менеджеры буфера на macOS и Linux.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use arboard::Clipboard;
#[cfg(target_os = "macos")]
use arboard::SetExtApple;
#[cfg(target_os = "linux")]
use arboard::SetExtLinux;
#[cfg(target_os = "windows")]
use arboard::SetExtWindows;
use zeroize::Zeroizing;

/// Буфер обмена живёт всё время работы приложения. В Linux (X11) скопированное
/// существует, пока жив объект буфера: созданный на одно копирование, он уносил бы
/// значение с собой, и вставлять было бы нечего.
static CLIPBOARD: Mutex<Option<Clipboard>> = Mutex::new(None);

/// Номер копирования. Никогда не начинается заново: таймер прежнего копирования
/// не должен принять новое значение за своё.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Последнее скопированное значение и его номер. Запоминается и без таймера, чтобы
/// при блокировке и выходе буфер очистился.
static LAST: Mutex<Option<(u64, Zeroizing<String>)>> = Mutex::new(None);

/// Действие с буфером обмена. Порядок блокировок всегда один: сначала буфер, потом
/// `LAST`, — иначе копирование и очистка могли бы ждать друг друга.
fn with_clipboard<T>(action: impl FnOnce(&mut Clipboard) -> Result<T, arboard::Error>) -> Result<T, arboard::Error> {
    let mut slot = CLIPBOARD.lock().unwrap_or_else(|p| p.into_inner());
    let clipboard = match slot.as_mut() {
        Some(clipboard) => clipboard,
        None => slot.insert(Clipboard::new()?),
    };
    action(clipboard)
}

/// Копирует значение. Если `clear_after` задано, через это время буфер очистится,
/// а при блокировке и выходе — сразу.
pub fn copy(value: Zeroizing<String>, clear_after: Option<Duration>) -> Result<(), arboard::Error> {
    with_clipboard(|clipboard| {
        let set = clipboard.set().exclude_from_history();
        #[cfg(target_os = "windows")]
        let set = set.exclude_from_cloud();
        set.text(value.as_str())
    })?;

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

/// Очищает буфер, если в нём всё ещё наше значение. Чужое не трогаем:
/// пользователь мог за это время скопировать что-то своё.
pub fn clear_ours() {
    clear_if_current(None);
}

fn clear_if_current(generation: Option<u64>) {
    let _ = with_clipboard(|clipboard| {
        let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
        let Some((current, value)) = last.as_ref() else { return Ok(()) };
        if generation.is_some_and(|g| g != *current) {
            return Ok(());
        }
        if clipboard.get_text().is_ok_and(|text| Zeroizing::new(text).as_str() == value.as_str()) {
            let _ = clipboard.clear();
        }
        *last = None;
        Ok(())
    });
}
