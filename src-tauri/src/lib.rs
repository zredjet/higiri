pub mod backup;
pub mod commands;
pub mod domain;
#[cfg(test)]
mod perf;
pub mod repo;
// Development sample data; also in test builds, which may be optimized.
#[cfg(any(debug_assertions, test))]
pub mod sample;

use std::path::PathBuf;

use tauri::Manager;

use crate::commands::backup::DataDir;
use crate::repo::db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta = commands::specta_builder();

    #[cfg(debug_assertions)]
    commands::export_bindings(&specta, commands::BINDINGS_PATH)
        .expect("failed to export TypeScript bindings");

    let builder = tauri::Builder::default();
    // A second launch brings the running window to the front and exits. Release
    // builds only: a development build may run next to the installed app (it
    // has its own database), and both share the identifier the lock uses.
    #[cfg(all(desktop, not(debug_assertions)))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
    }));

    builder
        .plugin(log_plugin())
        // Window position and size are restored at the next launch.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        // Markdown links open in the default browser (http/https only; see capabilities).
        .plugin(tauri_plugin_opener::init())
        // Save dialog for .md export (opened from Rust; no frontend permission).
        .plugin(tauri_plugin_dialog::init())
        // Copying a report (frontend permission: write text only).
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);
            log::info!("Higiri {} starting", app.package_info().version);

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

            // C-1: one backup a day, at the first launch of the day. A failed
            // backup is logged and does not stop the app.
            match tauri::async_runtime::block_on(backup::run_daily(
                &pool,
                &dir,
                commands::clock::today(),
            )) {
                Ok(Some(file)) => log::info!("backup written: {}", file.display()),
                Ok(None) => log::info!("backup for today already exists"),
                Err(e) => log::error!("daily backup failed: {e}"),
            }

            app.manage(pool);
            app.manage(DataDir(dir));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Operation log: `higiri.log` (or `higiri-dev.log` for development builds) in
/// the OS log folder, rotated at 1 MB with 5 old files kept. Only events and
/// errors are written, never TODO contents.
fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

    let file_name = if cfg!(debug_assertions) {
        "higiri-dev"
    } else {
        "higiri"
    };
    let mut builder = tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::LogDir {
            file_name: Some(file_name.into()),
        }))
        .level(log::LevelFilter::Info)
        .max_file_size(1_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                message
            ))
        });
    if cfg!(debug_assertions) {
        builder = builder.target(Target::new(TargetKind::Stdout));
    }
    builder.build()
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
