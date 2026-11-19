pub mod autostart;
pub mod backup;
pub mod commands;
pub mod domain;
pub mod json_data;
pub mod notifier;
#[cfg(test)]
mod perf;
pub mod repo;
pub mod resident;
// Development sample data; also in test builds, which may be optimized.
#[cfg(any(debug_assertions, test))]
pub mod sample;
pub mod tray;
#[cfg(all(windows, not(debug_assertions)))]
mod win_toast;

use std::path::PathBuf;

use tauri::Manager;
#[cfg(windows)]
use tauri_plugin_notification::NotificationExt;
// Registered in release builds only (see run()); referenced here so the
// dependencies do not count as unused in development builds.
#[cfg(all(desktop, debug_assertions))]
use tauri_plugin_autostart as _;
#[cfg(all(desktop, debug_assertions))]
use tauri_plugin_single_instance as _;
#[cfg(all(windows, debug_assertions))]
use winreg as _;

use crate::commands::backup::DataDir;
use crate::domain::settings::Settings;
use crate::repo::db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta = commands::specta_builder();

    #[cfg(debug_assertions)]
    commands::export_bindings(&specta, commands::BINDINGS_PATH)
        .expect("failed to export TypeScript bindings");

    let builder = tauri::Builder::default();
    // A second launch brings the running window to the front and exits (a
    // second launch at login leaves it hidden). Release builds only: a
    // development build may run next to the installed app (it has its own
    // database), and both share the identifier the lock uses.
    #[cfg(all(desktop, not(debug_assertions)))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        if !resident::started_at_login(&args) {
            resident::show_main(app);
        }
    }));
    // Start at login: release builds only (see autostart.rs).
    #[cfg(all(desktop, not(debug_assertions)))]
    let builder = builder.plugin(autostart::plugin());

    let app = builder
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
        // OS notifications (v1.1): shown from Rust only, no frontend permission.
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(specta.invoke_handler())
        // × hides the window to the tray (see resident.rs).
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                resident::on_close_requested(window, api);
            }
        })
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

            // C-1: one backup a day, at the first launch of the day (and when
            // the date changes while resident, see notifier.rs). A failed
            // backup is logged and does not stop the app.
            let today = commands::clock::today();
            match tauri::async_runtime::block_on(backup::run_daily(&pool, &dir, today)) {
                Ok(Some(file)) => log::info!("backup written: {}", file.display()),
                Ok(None) => log::info!("backup for today already exists"),
                Err(e) => log::error!("daily backup failed: {e}"),
            }

            let settings = tauri::async_runtime::block_on(commands::settings::get(&pool))
                .unwrap_or_else(|e| {
                    log::error!("settings could not be read: {}", e.message);
                    Settings::default()
                });

            // The tray first: without it, × quits instead of hiding.
            let tray_ready = match tray::create(app.handle()) {
                Ok(()) => true,
                Err(e) => {
                    log::error!("tray icon could not be created: {e}");
                    false
                }
            };
            app.manage(resident::CloseBehavior::new(&settings, tray_ready));

            // Windows: a click on a toast brings the window back (while the
            // toast is on screen). Not on macOS (the user's decision): there
            // the plugin keeps a thread and a main-thread timer for every
            // notification left in Notification Center, so a click only
            // activates the app, and the Dock or the menu bar opens the window.
            #[cfg(windows)]
            {
                let handle = app.handle().clone();
                app.notification().on_action(move |performed| {
                    if performed.action_id() == "tap" {
                        resident::show_main(&handle);
                    }
                })?;
            }

            autostart::sync_at_startup(app.handle(), settings.autostart);
            #[cfg(all(windows, not(debug_assertions)))]
            if let Err(e) =
                win_toast::register(&app.config().identifier, &app.package_info().name, &dir)
            {
                log::error!("toast registration failed: {e}");
            }

            let notifier = notifier::Notifier::new(&settings);
            notifier::spawn(
                app.handle().clone(),
                &notifier,
                pool.clone(),
                dir.clone(),
                today,
            );
            app.manage(notifier);

            app.manage(pool);
            app.manage(DataDir(dir));

            // The window is created hidden; a launch at login stays in the tray.
            if resident::started_at_login(std::env::args()) {
                log::info!("started at login: tray only");
            } else {
                resident::show_main(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| match event {
        // macOS: a click on the Dock icon brings the window back.
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => resident::show_main(_app),
        _ => {}
    });
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
