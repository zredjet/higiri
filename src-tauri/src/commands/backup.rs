//! バックアップ: list_backups, restore_backup, open_data_folder, and the
//! backup mirror (v0.4.0, C-3): choose_backup_mirror, clear_backup_mirror,
//! restore_backup_file.
//!
//! The folder and file dialogs open on the Rust side (off the main thread, as
//! for the JSON export); closing one without choosing returns null and changes
//! nothing. The commands that change the mirror or replace the data hand the
//! saved settings to the running app (`resident::apply`), so the resident
//! timer copies to the new mirror and stops copying to the old one.

use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::error::{AppError, CmdResult, ErrorKind};
use super::json_data::{pick, read_error};
use super::{clock, settings};
use crate::backup::{self, Backup, BackupError};
use crate::{repo, resident};

/// The folder holding todo.db and `backups/` (managed state).
pub struct DataDir(pub PathBuf);

/// バックアップの複製先 as chosen (v0.4.0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BackupMirror {
    /// The folder, as the dialog gave it.
    pub dir: String,
    /// The newest daily backup now on the mirror (null: there was none yet).
    pub last_ok: Option<NaiveDate>,
}

/// Backups in the data folder, newest first.
#[tauri::command]
#[specta::specta]
pub async fn list_backups(data_dir: State<'_, DataDir>) -> CmdResult<Vec<Backup>> {
    list(&data_dir.0)
}

/// Replaces all data with the chosen backup. The current data is copied to a
/// 「復元・取り込みの前の状態」 backup first, so the restore can be undone. The
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

/// 「ファイルを選んで復元」: asks for a backup file anywhere (on the mirror after
/// moving to a new PC, for one) and restores it as restore_backup does. The
/// screen has already said that all data will be replaced. Returns the file's
/// name, or null when the user closed the dialog.
#[tauri::command]
#[specta::specta]
pub async fn restore_backup_file(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    data_dir: State<'_, DataDir>,
) -> CmdResult<Option<String>> {
    let dialog = app
        .dialog()
        .file()
        .set_title("復元するバックアップのファイルを選ぶ")
        .add_filter("Higiriのバックアップ", &["db"]);
    let Some(picked) = pick(move || dialog.blocking_pick_file()).await? else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| read_error())?;
    restore_file(&pool, &data_dir.0, &path, clock::now()).await?;
    resident::apply(&app, &settings::get(&pool).await?);
    Ok(Some(file_name(&path)))
}

/// 「複製先を選ぶ」「複製先を変える」: asks for a folder, copies the daily
/// backups there at once and saves it as the mirror only when that worked.
/// Returns the mirror, or null when the user closed the dialog.
#[tauri::command]
#[specta::specta]
pub async fn choose_backup_mirror(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    data_dir: State<'_, DataDir>,
) -> CmdResult<Option<BackupMirror>> {
    let dialog = app.dialog().file().set_title("バックアップの複製先を選ぶ");
    let Some(picked) = pick(move || dialog.blocking_pick_folder()).await? else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| mirror_error())?;
    let chosen = choose_mirror(&pool, &data_dir.0, &path).await?;
    resident::apply(&app, &settings::get(&pool).await?);
    Ok(Some(chosen))
}

/// 「複製をやめる」: no mirror from now on. The files already there stay.
#[tauri::command]
#[specta::specta]
pub async fn clear_backup_mirror(app: AppHandle, pool: State<'_, SqlitePool>) -> CmdResult<()> {
    clear_mirror(&pool).await?;
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

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The message for a refused or failed restore (of a listed backup or of a
/// chosen file).
fn restore_error(e: BackupError) -> AppError {
    match e {
        BackupError::NotFound(_) => AppError {
            kind: ErrorKind::NotFound,
            message: "選んだバックアップが見つかりません。一覧を開き直してから選んでください。"
                .into(),
        },
        BackupError::TooNew => AppError::validation(
            "このバックアップは新しい版のアプリで作られたため、復元できません。別のバックアップを選んでください。",
        ),
        BackupError::Damaged => AppError::validation(
            "このバックアップは壊れているため、復元できません。別のバックアップを選んでください。",
        ),
        BackupError::InUse => AppError::validation(
            "今使っているデータのファイルは選べません。バックアップのファイル(todo-2026-11-18.db など)を選んでください。",
        ),
        BackupError::Source(_) => read_error(),
        _ => AppError {
            kind: ErrorKind::Internal,
            message: "復元できませんでした。データは復元の前のままです。アプリを再起動してから、もう一度操作してください。".into(),
        },
    }
}

fn restore_logged(result: backup::Result<String>, what: &str) -> CmdResult<()> {
    match result {
        Ok(kept) => {
            log::info!("restored {what}; previous data kept as {kept}");
            Ok(())
        }
        Err(e) => {
            match &e {
                BackupError::NotFound(_)
                | BackupError::TooNew
                | BackupError::Damaged
                | BackupError::InUse => log::warn!("restore {what} refused: {e}"),
                _ => log::error!("restore {what} failed: {e}"),
            }
            Err(restore_error(e))
        }
    }
}

pub async fn restore(
    pool: &SqlitePool,
    data_dir: &Path,
    file_name: &str,
    now: DateTime<Utc>,
) -> CmdResult<()> {
    restore_logged(
        backup::restore(pool, data_dir, file_name, now).await,
        file_name,
    )
}

/// Restores the file at `path` (chosen anywhere). The log names the kind of
/// file only, not the path.
pub async fn restore_file(
    pool: &SqlitePool,
    data_dir: &Path,
    path: &Path,
    now: DateTime<Utc>,
) -> CmdResult<()> {
    restore_logged(
        backup::restore_file(pool, data_dir, path, now).await,
        "a chosen file",
    )
}

fn mirror_error() -> AppError {
    AppError {
        kind: ErrorKind::Internal,
        message: "選んだフォルダに複製できませんでした。書き込めるフォルダか確かめてから、もう一度選んでください。".into(),
    }
}

/// Makes `folder` the backup mirror: refuses the data folder and anything in
/// it, copies the daily backups there, and saves the folder (with the newest
/// daily backup it got) only when the copy worked.
pub async fn choose_mirror(
    pool: &SqlitePool,
    data_dir: &Path,
    folder: &Path,
) -> CmdResult<BackupMirror> {
    if backup::is_within(folder, data_dir) {
        return Err(AppError::validation(
            "アプリのデータフォルダの中は選べません。外付けのドライブやクラウドの同期フォルダなど、別の場所を選んでください。",
        ));
    }
    let dir = folder
        .to_str()
        .filter(|_| folder.is_absolute())
        .ok_or_else(mirror_error)?
        .to_owned();
    let last_ok = backup::mirror(data_dir.to_owned(), folder.to_owned())
        .await
        .map_err(|e| {
            log::warn!("backup mirror could not be chosen: {}", e.kind());
            mirror_error()
        })?;
    let mut conn = pool.acquire().await?;
    repo::settings::set_backup_mirror(&mut conn, Some(&dir), last_ok).await?;
    log::info!("backup mirror chosen; daily backups copied there");
    Ok(BackupMirror { dir, last_ok })
}

/// No backup mirror from now on (the files on it stay).
pub async fn clear_mirror(pool: &SqlitePool) -> CmdResult<()> {
    let mut conn = pool.acquire().await?;
    repo::settings::set_backup_mirror(&mut conn, None, None).await?;
    log::info!("backup mirror cleared");
    Ok(())
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

    async fn mirror_settings(pool: &SqlitePool) -> (Option<String>, Option<NaiveDate>) {
        let s = settings::get(pool).await.unwrap();
        (s.backup_mirror_dir, s.backup_mirror_last_ok)
    }

    #[tokio::test]
    async fn a_chosen_mirror_gets_the_backups_and_is_saved() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = db::open(&tmp.path().join(db::DB_FILE_NAME)).await.unwrap();
        backup::run_daily(&pool, tmp.path(), d("2026-11-18"))
            .await
            .unwrap();
        backup::run_daily(&pool, tmp.path(), d("2026-11-19"))
            .await
            .unwrap();
        let usb = tempfile::tempdir().unwrap();
        let chosen = choose_mirror(&pool, tmp.path(), usb.path()).await.unwrap();
        let dir = usb.path().display().to_string();
        assert_eq!(
            chosen,
            BackupMirror {
                dir: dir.clone(),
                last_ok: Some(d("2026-11-19"))
            }
        );
        assert!(usb.path().join("todo-2026-11-18.db").is_file());
        assert!(usb.path().join("todo-2026-11-19.db").is_file());
        assert_eq!(
            mirror_settings(&pool).await,
            (Some(dir), Some(d("2026-11-19")))
        );

        // Another one: copied there too; the first one's files stay.
        let other = tempfile::tempdir().unwrap();
        choose_mirror(&pool, tmp.path(), other.path())
            .await
            .unwrap();
        assert!(other.path().join("todo-2026-11-19.db").is_file());
        assert!(usb.path().join("todo-2026-11-19.db").is_file());

        // 「複製をやめる」: both keys null, the files stay.
        clear_mirror(&pool).await.unwrap();
        assert_eq!(mirror_settings(&pool).await, (None, None));
        assert!(other.path().join("todo-2026-11-19.db").is_file());
    }

    #[tokio::test]
    async fn the_data_folder_and_what_is_in_it_cannot_be_the_mirror() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = db::open(&tmp.path().join(db::DB_FILE_NAME)).await.unwrap();
        backup::run_daily(&pool, tmp.path(), d("2026-11-19"))
            .await
            .unwrap();
        let inside = tmp.path().join("copies");
        std::fs::create_dir(&inside).unwrap();
        for folder in [
            tmp.path().to_owned(),
            backup::dir(tmp.path()),
            inside.clone(),
        ] {
            let err = choose_mirror(&pool, tmp.path(), &folder).await.unwrap_err();
            assert_eq!(err.kind, ErrorKind::Validation);
            assert_eq!(
                err.message,
                "アプリのデータフォルダの中は選べません。外付けのドライブやクラウドの同期フォルダなど、別の場所を選んでください。"
            );
        }
        assert!(std::fs::read_dir(&inside).unwrap().next().is_none());
        assert_eq!(mirror_settings(&pool).await, (None, None));
    }

    #[tokio::test]
    async fn a_mirror_that_cannot_be_written_is_not_saved() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = db::open(&tmp.path().join(db::DB_FILE_NAME)).await.unwrap();
        backup::run_daily(&pool, tmp.path(), d("2026-11-19"))
            .await
            .unwrap();
        // The mirror in use stays as it was.
        let usb = tempfile::tempdir().unwrap();
        choose_mirror(&pool, tmp.path(), usb.path()).await.unwrap();
        let before = mirror_settings(&pool).await;

        let gone = tempfile::tempdir().unwrap();
        let err = choose_mirror(&pool, tmp.path(), &gone.path().join("USB"))
            .await
            .unwrap_err();
        assert_eq!(
            err.message,
            "選んだフォルダに複製できませんでした。書き込めるフォルダか確かめてから、もう一度選んでください。"
        );
        // A file that has a daily backup's name, as a folder in the way.
        let blocked = tempfile::tempdir().unwrap();
        std::fs::create_dir(blocked.path().join("todo-2026-11-19.db")).unwrap();
        assert!(
            choose_mirror(&pool, tmp.path(), blocked.path())
                .await
                .is_err()
        );
        assert_eq!(mirror_settings(&pool).await, before);
    }

    #[tokio::test]
    async fn a_chosen_file_is_restored_and_refusals_say_what_to_do() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = db::open(&tmp.path().join(db::DB_FILE_NAME)).await.unwrap();
        backup::run_daily(&pool, tmp.path(), d("2026-11-18"))
            .await
            .unwrap();
        let usb = tempfile::tempdir().unwrap();
        let file = usb.path().join("todo-2026-11-18.db");
        std::fs::copy(backup::dir(tmp.path()).join("todo-2026-11-18.db"), &file).unwrap();
        restore_file(&pool, tmp.path(), &file, ts(2026, 11, 19, 1, 0))
            .await
            .unwrap();
        assert_eq!(
            list(tmp.path())
                .unwrap()
                .iter()
                .filter(|b| b.kind == backup::BackupKind::BeforeRestore)
                .count(),
            1
        );

        let cases = [
            (usb.path().join("missing.db"), "ファイルを読めませんでした"),
            (
                tmp.path().join(db::DB_FILE_NAME),
                "今使っているデータのファイルは選べません",
            ),
        ];
        std::fs::write(usb.path().join("notes.db"), "not a database").unwrap();
        for (path, says) in cases.into_iter().chain([(
            usb.path().join("notes.db"),
            "このバックアップは壊れているため",
        )]) {
            let err = restore_file(&pool, tmp.path(), &path, ts(2026, 11, 19, 2, 0))
                .await
                .unwrap_err();
            assert!(err.message.contains(says), "{}", err.message);
        }
    }
}
