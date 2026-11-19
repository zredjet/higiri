//! Start at login (v1.1, D-7), through tauri-plugin-autostart from Rust only
//! (the screens get no permission for it).
//!
//! - Registered with `--autostart`, so the app starts in the tray without its
//!   window. macOS uses a LaunchAgent, which can pass the argument.
//! - Release builds register again at every launch (`sync_at_startup`): moving
//!   the unpacked ZIP changes the executable's path.
//! - Development builds never touch the OS (the plugin is not even loaded).
//! - macOS: an app running from App Translocation (downloaded and opened in
//!   place) is not registered, since that path goes away.

use std::path::Path;

use tauri::{AppHandle, Runtime};

use crate::commands::error::CmdResult;

pub const TRANSLOCATED: &str = "自動起動を登録できませんでした。Higiri.app を「アプリケーション」フォルダに移してから、もう一度オンにしてください。";
#[cfg_attr(debug_assertions, allow(dead_code))]
const ENABLE_FAILED: &str =
    "自動起動を登録できませんでした。アプリを再起動してから、もう一度オンにしてください。";
#[cfg_attr(debug_assertions, allow(dead_code))]
const DISABLE_FAILED: &str =
    "自動起動を解除できませんでした。アプリを再起動してから、もう一度オフにしてください。";

/// The plugin (release builds only).
#[cfg(not(debug_assertions))]
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec![crate::resident::AUTOSTART_ARG]),
    )
}

/// Whether `exe` runs from macOS App Translocation
/// (`/private/var/folders/…/AppTranslocation/<id>/d/Higiri.app/…`).
pub fn is_translocated(exe: &Path) -> bool {
    exe.components()
        .any(|c| c.as_os_str() == "AppTranslocation")
}

/// Registers (`on`) or removes the start at login. Does nothing in
/// development builds.
pub fn apply<R: Runtime>(app: &AppHandle<R>, on: bool) -> CmdResult<()> {
    #[cfg(debug_assertions)]
    {
        let _ = (app, on);
        Ok(())
    }
    #[cfg(not(debug_assertions))]
    {
        register(app, on)
    }
}

#[cfg(not(debug_assertions))]
fn register<R: Runtime>(app: &AppHandle<R>, on: bool) -> CmdResult<()> {
    use crate::commands::error::{AppError, ErrorKind};
    use tauri_plugin_autostart::ManagerExt;

    let internal = |message: &str| AppError {
        kind: ErrorKind::Internal,
        message: message.into(),
    };
    let manager = app.autolaunch();
    if on {
        if std::env::current_exe().is_ok_and(|exe| is_translocated(&exe)) {
            return Err(AppError::validation(TRANSLOCATED));
        }
        manager.enable().map_err(|e| {
            log::error!("autostart enable failed: {e}");
            internal(ENABLE_FAILED)
        })
    } else {
        manager.disable().map_err(|e| {
            log::error!("autostart disable failed: {e}");
            internal(DISABLE_FAILED)
        })
    }
}

/// At startup (release builds): registers again, so the registration follows
/// the executable when it moved, or removes a registration left behind.
pub fn sync_at_startup<R: Runtime>(app: &AppHandle<R>, on: bool) {
    if cfg!(debug_assertions) {
        return;
    }
    match apply(app, on) {
        Ok(()) if on => log::info!("autostart registered"),
        Ok(()) => {}
        Err(e) => log::warn!("autostart not updated: {}", e.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translocated_paths_are_recognized() {
        assert!(is_translocated(Path::new(
            "/private/var/folders/xy/abc/T/AppTranslocation/0F1E/d/Higiri.app/Contents/MacOS/higiri"
        )));
        assert!(!is_translocated(Path::new(
            "/Applications/Higiri.app/Contents/MacOS/higiri"
        )));
        assert!(!is_translocated(Path::new(
            "/Users/me/Downloads/AppTranslocationNotes/Higiri.app/Contents/MacOS/higiri"
        )));
    }
}
