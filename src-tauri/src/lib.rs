mod commands;
mod session;

use tauri::Manager;

use crate::session::{AppState, Session};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
