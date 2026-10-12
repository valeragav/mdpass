#[cfg(desktop)]
mod clipboard;
mod commands;
mod entries;
mod session;

use tauri::Manager;

use crate::session::{AppState, Session};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("config.json");
            app.manage(AppState::new(Session::new(config_path)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::describe_path,
            commands::create_vault,
            commands::unlock,
            commands::lock,
            entries::entries,
            entries::reveal_field,
            entries::copy_field,
            entries::add_entry,
            entries::update_entry,
            entries::delete_entry,
            entries::generate_password,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
