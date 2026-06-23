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
        .plugin(
            // Action + error logs to a per-OS file (and stdout in dev). Never
            // logs note content or passphrases — only actions, ids and errors.
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("slate".to_string()),
                    },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Storage mode: volatile in-memory in DEBUG builds (nothing is
            // persisted, everything is wiped on close), persistent SQLite in
            // RELEASE builds.
            #[cfg(debug_assertions)]
            {
                log::info!("Slate starting in DEBUG mode; storage is VOLATILE (in-memory, not persisted)");
                app.manage(AppState::memory()?);
            }
            #[cfg(not(debug_assertions))]
            {
                let app_data_dir = app.path().app_data_dir()?;
                std::fs::create_dir_all(&app_data_dir)?;
                let db_path = app_data_dir.join("notes.sqlite");
                log::info!("Slate starting in RELEASE mode; database at {}", db_path.display());
                app.manage(AppState::sqlite(db_path)?);
            }
            log::info!("application state ready");
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
            commands::vault::recovery_status,
            commands::vault::set_up_recovery,
            commands::vault::reveal_note,
            commands::vault::clear_active,
            commands::vault::protect_note,
            commands::vault::unprotect_note,
            commands::vault::recover_note,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
