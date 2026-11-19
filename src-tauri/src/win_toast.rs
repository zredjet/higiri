//! Windows release builds: registers the app's identity for toasts (AUMID),
//! so that notifications show the name Higiri and its icon.
//!
//! The notification plugin sends toasts as the app identifier
//! (`com.redjet.higiri`) when the executable is not in `target\debug` or
//! `target\release`. An unpackaged app has to describe that identifier under
//! `HKCU\Software\Classes\AppUserModelId\<id>`: `DisplayName` and `IconUri`
//! (a PNG written to the data folder). Written at every launch.
//!
//! If toasts do not appear this way on a real PC, the fallback in the design
//! is a Start menu shortcut carrying the AUMID.

use std::path::Path;

/// The icon the toasts show (written next to todo.db).
const ICON_PNG: &[u8] = include_bytes!("../icons/128x128.png");
const ICON_FILE: &str = "toast-icon.png";

/// Writes the icon and the registry values for `app_id`.
pub fn register(app_id: &str, display_name: &str, data_dir: &Path) -> std::io::Result<()> {
    let icon = data_dir.join(ICON_FILE);
    if std::fs::read(&icon).ok().as_deref() != Some(ICON_PNG) {
        std::fs::write(&icon, ICON_PNG)?;
    }
    let (key, _) =
        winreg::HKCU.create_subkey(format!(r"Software\Classes\AppUserModelId\{app_id}"))?;
    key.set_value("DisplayName", &display_name)?;
    key.set_value("IconUri", &icon.display().to_string())?;
    Ok(())
}
