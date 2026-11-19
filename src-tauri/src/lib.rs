pub mod commands;
pub mod domain;
pub mod repo;
#[cfg(debug_assertions)]
pub mod sample;

use std::path::PathBuf;

use tauri::Manager;

use crate::repo::db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta = commands::specta_builder();

    #[cfg(debug_assertions)]
    commands::export_bindings(&specta, commands::BINDINGS_PATH)
        .expect("failed to export TypeScript bindings");

    tauri::Builder::default()
        // Markdown links open in the default browser (http/https only; see capabilities).
        .plugin(tauri_plugin_opener::init())
        // Save dialog for .md export (opened from Rust; no frontend permission).
        .plugin(tauri_plugin_dialog::init())
        // Copying a report (frontend permission: write text only).
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);

            // Open (or create) todo.db and apply migrations before any window
            // can call a command.
            let dir = data_dir(app.path().app_data_dir()?);
            std::fs::create_dir_all(&dir)?;
            let pool = tauri::async_runtime::block_on(db::open(&dir.join(db::DB_FILE_NAME)))?;

            #[cfg(debug_assertions)]
            if std::env::args().any(|a| a == "--sample-data") {
                let today = commands::clock::today();
                let seeded = tauri::async_runtime::block_on(sample::seed_if_empty(&pool, today))?;
                println!(
                    "sample data: {}",
                    if seeded {
                        "inserted"
                    } else {
                        "skipped (database is not empty)"
                    }
                );
            }

            app.manage(pool);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Folder holding todo.db. Debug builds use a `dev` subfolder so development
/// data never mixes with the data of an installed release build.
fn data_dir(app_data_dir: PathBuf) -> PathBuf {
    if cfg!(debug_assertions) {
        app_data_dir.join("dev")
    } else {
        app_data_dir
    }
}
