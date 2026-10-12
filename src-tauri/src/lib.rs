mod backups;
#[cfg(desktop)]
mod clipboard;
mod commands;
mod entries;
mod importing;
mod session;
mod settings;

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

use crate::session::{AppState, Session};

/// Событие для интерфейса: хранилище закрыто без его участия. В данных — причина:
/// `idle` (бездействие) или `minimized` (окно свернули).
const LOCKED_EVENT: &str = "vault-locked";

/// Как часто проверять бездействие.
const IDLE_CHECK: Duration = Duration::from_secs(5);

/// Идентификатор приложения раньше был `com.mdpass.app`. Настройки и резервные копии
/// из старых папок переносятся в новые один раз, при первом запуске.
fn move_old_app_data(app: &AppHandle) {
    const OLD_IDENTIFIER: &str = "com.mdpass.app";
    let paths = app.path();
    let moves = [
        paths
            .app_config_dir()
            .ok()
            .map(|dir| (dir.with_file_name(OLD_IDENTIFIER).join("config.json"), dir.join("config.json"))),
        paths
            .app_local_data_dir()
            .ok()
            .map(|dir| (dir.with_file_name(OLD_IDENTIFIER).join("backups"), dir.join("backups"))),
    ];
    for (from, to) in moves.into_iter().flatten() {
        if !from.exists() || to.exists() {
            continue;
        }
        let moved = to.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::rename(&from, &to));
        if let Err(err) = moved {
            eprintln!("не удалось перенести {} в {}: {err}", from.display(), to.display());
        }
    }
}

/// Rust сам следит за бездействием: хранилище закроется, даже если интерфейс завис.
fn watch_idle(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(IDLE_CHECK);
            let state = app.state::<AppState>();
            let mut session = state.lock();
            if session.idle_expired(Instant::now()) {
                session.lock_vault();
                drop(session);
                let _ = app.emit(LOCKED_EVENT, "idle");
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            move_old_app_data(app.handle());
            let config_path = app.path().app_config_dir()?.join("config.json");
            // Копии лежат в локальной папке приложения: им не нужно синхронизироваться
            // между компьютерами, и они не засоряют папку с хранилищем.
            let backups_root = app.path().app_local_data_dir()?.join("backups");
            app.manage(AppState::new(Session::new(config_path, backups_root)));
            watch_idle(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Resized(_) = event
                && window.is_minimized().unwrap_or(false)
            {
                let state = window.state::<AppState>();
                let mut session = state.lock();
                if session.vault.is_some() && session.config.lock_on_minimize {
                    session.lock_vault();
                    drop(session);
                    let _ = window.emit(LOCKED_EVENT, "minimized");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::describe_path,
            commands::create_vault,
            commands::unlock,
            commands::lock,
            commands::touch,
            commands::reload_vault,
            commands::change_protection,
            settings::update_settings,
            backups::list_backups,
            backups::preview_backup,
            backups::restore_backup,
            backups::restore_damaged_vault,
            backups::export_backup,
            backups::backups_location,
            importing::analyze_import,
            importing::run_import,
            importing::delete_import_source,
            entries::entries,
            entries::reveal_field,
            entries::copy_field,
            entries::add_entry,
            entries::update_entry,
            entries::delete_entry,
            entries::generate_password,
            entries::move_entry,
            entries::move_section,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Секрет не должен остаться в буфере после выхода из приложения.
            if let RunEvent::Exit = event {
                #[cfg(desktop)]
                clipboard::clear_ours();
            }
        });
}
