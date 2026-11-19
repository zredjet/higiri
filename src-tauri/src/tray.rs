//! The tray icon (v1.1, D-7), made with Tauri's own `tray-icon` feature.
//!
//! - The menu has two items, 「Higiri を開く」 and 「終了」. No counts are shown
//!   (they would go stale while the window is closed).
//! - Windows: a left click on the icon opens the window too (the menu is on
//!   the right click). macOS: a click opens the menu, as menu bar items do.
//! - The icons come from `npm run icons`: a small color icon for Windows, a
//!   one-color template image for the macOS menu bar.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Runtime};

use crate::resident;

const TRAY_ID: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

#[cfg(target_os = "macos")]
const ICON: &[u8] = include_bytes!("../icons/tray-macos.png");
#[cfg(not(target_os = "macos"))]
const ICON: &[u8] = include_bytes!("../icons/tray-windows.png");

/// Creates the tray icon.
pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN, "Higiri を開く", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Higiri")
        .menu(&menu)
        .show_menu_on_left_click(!cfg!(windows))
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN => resident::show_main(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if cfg!(windows)
                && let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
            {
                resident::show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
