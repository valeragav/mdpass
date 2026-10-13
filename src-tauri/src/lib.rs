mod backups;
#[cfg(desktop)]
mod clipboard;
mod commands;
mod entries;
mod importing;
mod session;
mod settings;

use std::time::{Duration, SystemTime};

use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use crate::session::{AppState, Session};

/// Событие для интерфейса: хранилище закрыто без его участия. В данных — причина:
/// `idle` (бездействие) или `minimized` (окно свернули).
const LOCKED_EVENT: &str = "vault-locked";

/// Как часто проверять бездействие.
const IDLE_CHECK: Duration = Duration::from_secs(5);

/// Сбой приложения попадает в журнал, а не только в консоль, которой у
/// установленного приложения нет.
fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("сбой: {info}");
        default(info);
    }));
}

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
            log::warn!("не удалось перенести {} в {}: {err}", from.display(), to.display());
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
            if session.idle_expired(SystemTime::now()) {
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
        // Журнал ошибок в папке журналов приложения: до трёх файлов по мегабайту.
        // Значения записей и пароли сюда не пишутся, только пути и системные ошибки.
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .targets([
                    Target::new(TargetKind::LogDir { file_name: Some("mdpass".into()) }),
                    Target::new(TargetKind::Stderr),
                ])
                .level(log::LevelFilter::Info)
                .max_file_size(1_000_000)
                .rotation_strategy(RotationStrategy::KeepSome(3))
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            log_panics();
            log::info!("mdpass {} запущен", app.package_info().version);
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
                // Сессию может держать запись файла (около секунды): ждём её не в потоке
                // окна, иначе окно зависнет.
                let window = window.clone();
                std::thread::spawn(move || {
                    let state = window.state::<AppState>();
                    let mut session = state.lock();
                    if session.vault.is_some() && session.config.lock_on_minimize {
                        session.lock_vault();
                        drop(session);
                        let _ = window.emit(LOCKED_EVENT, "minimized");
                    }
                });
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
            commands::rename_vault,
            commands::change_protection,
            settings::update_settings,
            backups::list_backups,
            backups::preview_backup,
            backups::restore_backup,
            backups::reveal_backup_field,
            backups::copy_backup_field,
            backups::restore_backup_entry,
            backups::close_backup,
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
            entries::set_favorite,
            entries::order_favorites,
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
