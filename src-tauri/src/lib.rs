pub mod commands;
pub mod domain;
pub mod repo;

use tauri::Manager;

use crate::repo::db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Open (or create) todo.db in the app data folder and apply migrations
            // before any window can call a command.
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let pool = tauri::async_runtime::block_on(db::open(&dir.join(db::DB_FILE_NAME)))?;
            app.manage(pool);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
