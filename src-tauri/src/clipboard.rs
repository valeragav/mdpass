//! Копирование значений в буфер обмена с очисткой через заданное время.
//!
//! Значение помечается как «не сохранять в журнал буфера», чтобы оно не попало
//! в историю Windows (Win+V), облачный буфер и менеджеры буфера на macOS и Linux.

use std::sync::Mutex;
use std::time::Duration;

use arboard::Clipboard;
#[cfg(target_os = "macos")]
use arboard::SetExtApple;
#[cfg(target_os = "linux")]
use arboard::SetExtLinux;
#[cfg(target_os = "windows")]
use arboard::SetExtWindows;
use zeroize::Zeroizing;

/// Последнее скопированное значение и номер копирования. По номеру старый таймер
/// понимает, что после него скопировали что-то ещё.
static LAST: Mutex<Option<(u64, Zeroizing<String>)>> = Mutex::new(None);

/// Копирует значение. Если `clear_after` задано, через это время буфер очистится,
/// а при блокировке и выходе — сразу.
pub fn copy(value: Zeroizing<String>, clear_after: Option<Duration>) -> Result<(), arboard::Error> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set().exclude_from_history().text(value.as_str())?;
    let Some(clear_after) = clear_after else {
        *LAST.lock().unwrap_or_else(|p| p.into_inner()) = None;
        return Ok(());
    };

    let generation = {
        let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
        let generation = last.as_ref().map_or(1, |(g, _)| g + 1);
        *last = Some((generation, value));
        generation
    };
    std::thread::spawn(move || {
        std::thread::sleep(clear_after);
        clear_if_current(Some(generation));
    });
    Ok(())
}

/// Очищает буфер, если в нём всё ещё наше значение. Чужое не трогаем:
/// пользователь мог за это время скопировать что-то своё.
pub fn clear_ours() {
    clear_if_current(None);
}

fn clear_if_current(generation: Option<u64>) {
    let mut last = LAST.lock().unwrap_or_else(|p| p.into_inner());
    let Some((current, value)) = last.as_ref() else { return };
    if generation.is_some_and(|g| g != *current) {
        return;
    }
    if let Ok(mut clipboard) = Clipboard::new()
        && clipboard.get_text().is_ok_and(|text| Zeroizing::new(text).as_str() == value.as_str())
    {
        let _ = clipboard.clear();
    }
    *last = None;
}
