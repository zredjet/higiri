/// App commands that get `allow-*` / `deny-*` permissions. Once this list is
/// set, the screens can call only the commands a capability allows.
/// Keep it in step with `commands::specta_builder()` and
/// `capabilities/default.json` (a test checks the capability file).
const APP_COMMANDS: &[&str] = &[
    "list_projects",
    "create_project",
    "update_project",
    "archive_project",
    "delete_project",
    "list_todos",
    "get_todo",
    "create_todo",
    "update_todo",
    "change_status",
    "delete_todo",
    "list_timeline",
    "save_work_log",
    "delete_work_log",
    "get_settings",
    "update_settings",
    "get_dashboard",
    "get_startup_reminder",
    "ack_reminder",
    "get_gantt",
];

fn main() {
    // `sqlx::migrate!` embeds the migration files at compile time.
    println!("cargo:rerun-if-changed=migrations");
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
