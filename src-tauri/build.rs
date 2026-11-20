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
    "set_todo_parent",
    "set_todo_repeat",
    "list_todo_templates",
    "save_todo_template",
    "rename_todo_template",
    "delete_todo_template",
    "add_dependency",
    "remove_dependency",
    "list_dependencies",
    "list_timeline",
    "save_work_log",
    "delete_work_log",
    "get_settings",
    "update_settings",
    "get_dashboard",
    "get_startup_reminder",
    "ack_reminder",
    "get_gantt",
    "generate_report",
    "save_report",
    "get_report",
    "export_report_file",
    "list_backups",
    "restore_backup",
    "open_data_folder",
    "test_notification",
    "list_holidays",
    "search_todos",
    "export_data",
    "import_data",
];

fn main() {
    // `sqlx::migrate!` embeds the migration files at compile time.
    println!("cargo:rerun-if-changed=migrations");

    // Windows: embed the application manifest (Common Controls v6, which the
    // dialog plugin needs) into every binary of the package, test binaries
    // included. tauri-build's own manifest reaches only the app executable,
    // and `cargo test` then fails to start with STATUS_ENTRYPOINT_NOT_FOUND.
    // The file is tauri-build's default manifest.
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let manifest = std::path::Path::new(&dir).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(windows)
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
