//! 通知: test_notification.

use tauri::AppHandle;

use super::error::{AppError, CmdResult, ErrorKind};
use crate::domain::notify;
use crate::notifier;

/// 「通知を試す」: shows a test notification, whether or not the
/// notifications are turned on (it checks that the OS shows them at all).
#[tauri::command]
#[specta::specta]
pub async fn test_notification(app: AppHandle) -> CmdResult<()> {
    notifier::show(&app, &notify::test_message()).map_err(|e| {
        log::error!("test notification failed: {e}");
        AppError {
            kind: ErrorKind::Internal,
            message: "通知を出せませんでした。OSの設定で Higiri の通知が許可されているかを確かめてください。".into(),
        }
    })
}
