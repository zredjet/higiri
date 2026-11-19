//! バックアップ: list_backups, restore_backup, open_data_folder.

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::error::{AppError, CmdResult, ErrorKind};
use super::{clock, settings};
use crate::backup::{self, Backup, BackupError};
use crate::resident;

/// The folder holding todo.db and `backups/` (managed state).
pub struct DataDir(pub PathBuf);

/// Backups in the data folder, newest first.
#[tauri::command]
#[specta::specta]
pub async fn list_backups(data_dir: State<'_, DataDir>) -> CmdResult<Vec<Backup>> {
    list(&data_dir.0)
}

/// Replaces all data with the chosen backup. The current data is copied to a
/// 「復元する前の状態」 backup first, so the restore can be undone. The
/// restored settings then apply to the running app (× and the notification
/// times); `autostart` is kept as it was, so the OS registration stays.
#[tauri::command]
#[specta::specta]
pub async fn restore_backup(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    data_dir: State<'_, DataDir>,
    file_name: String,
) -> CmdResult<()> {
    restore(&pool, &data_dir.0, &file_name, clock::now()).await?;
    resident::apply(&app, &settings::get(&pool).await?);
    Ok(())
}

/// Opens the data folder in Finder / Explorer.
#[tauri::command]
#[specta::specta]
pub async fn open_data_folder(app: AppHandle, data_dir: State<'_, DataDir>) -> CmdResult<()> {
    let path = data_dir.0.display().to_string();
    app.opener().open_path(&path, None::<&str>).map_err(|e| {
        log::error!("open data folder failed: {e}");
        AppError {
            kind: ErrorKind::Internal,
            message: format!(
                "データフォルダを開けませんでした。次の場所を手で開いてください: {path}"
            ),
        }
    })
}

pub fn list(data_dir: &Path) -> CmdResult<Vec<Backup>> {
    backup::list(data_dir).map_err(|e| {
        log::error!("list backups failed: {e}");
        AppError {
            kind: ErrorKind::Internal,
            message: "バックアップの一覧を読めませんでした。「データフォルダを開く」で backups フォルダを確かめてください。".into(),
        }
    })
}

pub async fn restore(
    pool: &SqlitePool,
    data_dir: &Path,
    file_name: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> CmdResult<()> {
    match backup::restore(pool, data_dir, file_name, now).await {
        Ok(kept) => {
            log::info!("restored {file_name}; previous data kept as {kept}");
            Ok(())
        }
        Err(BackupError::NotFound(_)) => Err(AppError {
            kind: ErrorKind::NotFound,
            message: "選んだバックアップが見つかりません。一覧を開き直してから選んでください。"
                .into(),
        }),
        Err(BackupError::TooNew) => Err(AppError::validation(
            "このバックアップは新しい版のアプリで作られたため、復元できません。別のバックアップを選んでください。",
        )),
        Err(BackupError::Damaged) => Err(AppError::validation(
            "このバックアップは壊れているため、復元できません。別のバックアップを選んでください。",
        )),
        Err(e) => {
            log::error!("restore {file_name} failed: {e}");
            Err(AppError {
                kind: ErrorKind::Internal,
                message: "復元できませんでした。データは復元の前のままです。アプリを再起動してから、もう一度操作してください。".into(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    #[tokio::test]
    async fn errors_are_shown_in_japanese_with_what_to_do() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = db::open(&tmp.path().join(db::DB_FILE_NAME)).await.unwrap();
        assert_eq!(list(tmp.path()).unwrap(), vec![]);

        let err = restore(
            &pool,
            tmp.path(),
            "todo-2026-11-19.db",
            ts(2026, 11, 19, 0, 0),
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::NotFound);

        backup::run_daily(&pool, tmp.path(), d("2026-11-19"))
            .await
            .unwrap();
        let listed = list(tmp.path()).unwrap();
        restore(
            &pool,
            tmp.path(),
            &listed[0].file_name,
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();
        assert_eq!(list(tmp.path()).unwrap().len(), 2);
    }
}
