mod app;
mod commands;
mod domain;
mod infra;
mod ports;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("notes.sqlite");
            app.manage(AppState::sqlite(db_path)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::notes::create_note,
            commands::notes::list_notes,
            commands::notes::search_notes,
            commands::notes::update_note,
            commands::notes::toggle_favorite,
            commands::notes::archive_note,
            commands::notes::delete_note,
            commands::notes::export_note,
            commands::vault::vault_status,
            commands::vault::create_vault,
            commands::vault::unlock_vault,
            commands::vault::lock_vault,
            commands::vault::protect_note,
            commands::vault::unprotect_note,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
