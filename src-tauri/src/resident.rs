//! Staying resident (v1.1, D-7): the main window, the × button and the tray
//! hint.
//!
//! - The main window is created hidden (`tauri.conf.json`) and shown by the
//!   startup, unless the app was started at login (`--autostart`): then only
//!   the tray appears.
//! - × hides the window to the tray while `close_to_tray` is on, and quits
//!   otherwise. Without a tray (it could not be created) × always quits, since
//!   nothing could bring the window back.
//! - The first time × hides the window, a notification says so once
//!   (`tray_hint_shown`), even with the notifications turned off.
//! - 表示倍率 (v0.4.0): the main window's webview is zoomed as a whole
//!   (`set_zoom`), before the window is first shown, whenever the setting
//!   changes, and again after every page load (WebView2 keeps a zoom factor
//!   per site and may reset it when a page loads).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use sqlx::SqlitePool;
use tauri::{AppHandle, CloseRequestApi, Manager, Runtime, Webview, Window};

use crate::domain::notify;
use crate::domain::settings::{Settings, zoom_factor};
use crate::notifier::{self, Notifier};
use crate::repo;

/// Label of the main window (`tauri.conf.json`).
pub const MAIN_WINDOW: &str = "main";
/// Argument of the launch at login: start in the tray without the window.
pub const AUTOSTART_ARG: &str = "--autostart";

/// Whether the arguments are those of the launch at login.
pub fn started_at_login<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|a| a.as_ref() == AUTOSTART_ARG)
}

/// What × does, kept in step with the settings (managed state).
pub struct CloseBehavior {
    to_tray: AtomicBool,
    tray_ready: AtomicBool,
    hint_shown: AtomicBool,
}

impl CloseBehavior {
    pub fn new(settings: &Settings, tray_ready: bool) -> CloseBehavior {
        CloseBehavior {
            to_tray: AtomicBool::new(settings.close_to_tray),
            tray_ready: AtomicBool::new(tray_ready),
            hint_shown: AtomicBool::new(settings.tray_hint_shown),
        }
    }

    /// × hides the window (instead of quitting).
    pub fn hides(&self) -> bool {
        self.to_tray.load(Ordering::Relaxed) && self.tray_ready.load(Ordering::Relaxed)
    }
}

/// The 表示倍率 of the main window in percent, kept in step with the settings
/// (managed state), so that a page load can apply it again.
pub struct Zoom(AtomicU32);

impl Zoom {
    pub fn new(settings: &Settings) -> Zoom {
        Zoom(AtomicU32::new(settings.zoom))
    }
}

/// Zooms the main window's webview to the 表示倍率 in [`Zoom`].
pub fn apply_zoom<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        zoom_webview(app, w.as_ref());
    }
}

/// A page finished loading in a webview: the main one gets its zoom again.
pub fn on_page_loaded<R: Runtime>(webview: &Webview<R>) {
    if webview.label() == MAIN_WINDOW {
        zoom_webview(webview.app_handle(), webview);
    }
}

fn zoom_webview<R: Runtime>(app: &AppHandle<R>, webview: &Webview<R>) {
    let Some(zoom) = app.try_state::<Zoom>() else {
        return;
    };
    let percent = zoom.0.load(Ordering::Relaxed);
    if let Err(e) = webview.set_zoom(zoom_factor(percent)) {
        log::error!("zoom {percent}% could not be applied: {e}");
    }
}

/// Brings the main window back: restore from minimized, show, focus.
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Applies saved settings to the running app: what × does, the resident
/// timer's settings (it keeps them in memory and computes its wait again) and
/// the 表示倍率 (only when it changed). Called after update_settings and after
/// a restore replaced the settings (which keeps the 表示倍率).
pub fn apply<R: Runtime>(app: &AppHandle<R>, settings: &Settings) {
    if let Some(close) = app.try_state::<CloseBehavior>() {
        close
            .to_tray
            .store(settings.close_to_tray, Ordering::Relaxed);
    }
    if let Some(notifier) = app.try_state::<Notifier>() {
        notifier.update(settings);
    }
    if let Some(zoom) = app.try_state::<Zoom>()
        && zoom.0.swap(settings.zoom, Ordering::Relaxed) != settings.zoom
    {
        apply_zoom(app);
    }
}

/// × on the main window.
pub fn on_close_requested<R: Runtime>(window: &Window<R>, api: &CloseRequestApi) {
    if window.label() != MAIN_WINDOW {
        return;
    }
    let app = window.app_handle();
    let Some(close) = app.try_state::<CloseBehavior>() else {
        return;
    };
    if !close.hides() {
        return;
    }
    api.prevent_close();
    let _ = window.hide();
    if close.hint_shown.swap(true, Ordering::Relaxed) {
        return;
    }
    if let Err(e) = notifier::show(app, &notify::tray_hint()) {
        log::error!("tray hint notification failed: {e}");
    }
    let Some(pool) = app.try_state::<SqlitePool>() else {
        return;
    };
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn(async move {
        let saved = async {
            let mut conn = pool.acquire().await?;
            repo::settings::set_tray_hint_shown(&mut conn, true).await
        }
        .await;
        if let Err(e) = saved {
            log::error!("could not save tray_hint_shown: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_autostart_argument_counts() {
        assert!(started_at_login(["higiri", "--autostart"]));
        assert!(!started_at_login(["higiri"]));
        assert!(!started_at_login(["higiri", "--sample-data"]));
    }

    #[test]
    fn x_hides_only_with_the_setting_and_a_tray() {
        let on = Settings::default();
        let off = Settings {
            close_to_tray: false,
            ..Settings::default()
        };
        assert!(CloseBehavior::new(&on, true).hides());
        assert!(!CloseBehavior::new(&on, false).hides());
        assert!(!CloseBehavior::new(&off, true).hides());
    }
}
