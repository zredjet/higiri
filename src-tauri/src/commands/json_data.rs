//! JSON (C-2): export_data, import_data.
//!
//! Both dialogs open on the Rust side (off the main thread, as for the
//! report's .md). Closing a dialog without choosing returns null and changes
//! nothing.

use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use super::backup::DataDir;
use super::error::{AppError, CmdResult, ErrorKind};
use super::{clock, settings};
use crate::json_data::{self, ExportError, ImportError};
use crate::resident;

/// Asks where to save (default name `higiri-YYYY-MM-DD.json`) and writes all
/// data there. Returns the path, or null when the user closed the dialog.
#[tauri::command]
#[specta::specta]
pub async fn export_data(app: AppHandle, pool: State<'_, SqlitePool>) -> CmdResult<Option<String>> {
    let dialog = app
        .dialog()
        .file()
        .set_title("データをJSONで書き出す")
        .set_file_name(default_file_name(clock::today()))
        .add_filter("JSON", &["json"]);
    let Some(picked) = pick(move || dialog.blocking_save_file()).await? else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| write_error())?;
    let version = app.package_info().version.to_string();
    export(&pool, &path, &version, clock::now()).await?;
    Ok(Some(path.display().to_string()))
}

/// Asks for a file exported by Higiri and replaces all data with it, after
/// keeping the current data as a 「復元・取り込みの前の状態」 backup. The
/// screen has already asked the user. The imported settings then apply to the
/// running app, as after a restore. Returns the file's name, or null when the
/// user closed the dialog.
#[tauri::command]
#[specta::specta]
pub async fn import_data(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    data_dir: State<'_, DataDir>,
) -> CmdResult<Option<String>> {
    let dialog = app
        .dialog()
        .file()
        .set_title("取り込むJSONファイルを選ぶ")
        .add_filter("JSON", &["json"]);
    let Some(picked) = pick(move || dialog.blocking_pick_file()).await? else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| read_error())?;
    import(&pool, &data_dir.0, &path, clock::now()).await?;
    resident::apply(&app, &settings::get(&pool).await?);
    Ok(Some(file_name(&path)))
}

/// Runs a blocking dialog off the main thread.
pub(super) async fn pick(
    dialog: impl FnOnce() -> Option<FilePath> + Send + 'static,
) -> CmdResult<Option<FilePath>> {
    tauri::async_runtime::spawn_blocking(dialog)
        .await
        .map_err(|_| AppError::internal())
}

/// `higiri-2026-11-19.json`.
pub fn default_file_name(today: NaiveDate) -> String {
    format!("higiri-{today}.json")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn write_error() -> AppError {
    AppError {
        kind: ErrorKind::Internal,
        message: "ファイルを書き出せませんでした。保存先を変えて、もう一度書き出してください。"
            .into(),
    }
}

pub(super) fn read_error() -> AppError {
    AppError {
        kind: ErrorKind::Internal,
        message: "ファイルを読めませんでした。ファイルの場所を確かめて、もう一度選んでください。"
            .into(),
    }
}

pub async fn export(
    pool: &SqlitePool,
    path: &Path,
    app_version: &str,
    now: DateTime<Utc>,
) -> CmdResult<()> {
    match json_data::export_to(pool, path, app_version, now).await {
        Ok(()) => {
            log::info!("all data exported to JSON");
            Ok(())
        }
        Err(ExportError::Io(e)) => {
            log::error!("JSON export could not be written: {e}");
            Err(write_error())
        }
        Err(e) => {
            log::error!("JSON export failed: {e}");
            Err(AppError::internal())
        }
    }
}

/// The message for a refused or failed import. Every refusal says that the
/// data was not changed.
fn import_error(e: &ImportError) -> AppError {
    let refused = |what: &str, todo: &str| {
        AppError::validation(format!(
            "{what}ため、取り込めませんでした。今のデータは変わっていません。{todo}"
        ))
    };
    const PICK_EXPORT: &str = "Higiriの「JSONで書き出す」で作ったファイルを選んでください。";
    const EXPORT_AGAIN: &str =
        "書き出したあとに手で直した所があれば元に戻すか、書き出し直したファイルを選んでください。";
    match e {
        ImportError::Read(_) => read_error(),
        ImportError::NotJson => refused("このファイルはJSONとして読めない", PICK_EXPORT),
        ImportError::NotOurs => refused("このファイルはHigiriで書き出したJSONではない", PICK_EXPORT),
        ImportError::TooNew => refused(
            "このファイルは新しい版のアプリで書き出された",
            "アプリを新しい版にしてから取り込んでください。",
        ),
        ImportError::Malformed => refused("ファイルの中身に読めない所がある", EXPORT_AGAIN),
        ImportError::Constraint => refused(
            "ファイルの中身につじつまの合わない所がある(TODOのプロジェクトが見つからない、日付の前後が逆、など)",
            EXPORT_AGAIN,
        ),
        ImportError::Unreadable { .. } => refused(
            "ファイルの中身に、アプリが読めない値がある(日付の書き方が違う、など)",
            EXPORT_AGAIN,
        ),
        ImportError::Replace(_)
        | ImportError::Db(_)
        | ImportError::Migrate(_)
        | ImportError::Io(_) => AppError {
            kind: ErrorKind::Internal,
            message: "取り込めませんでした。データは取り込む前のままです。アプリを再起動してから、もう一度操作してください。".into(),
        },
    }
}

pub async fn import(
    pool: &SqlitePool,
    data_dir: &Path,
    path: &Path,
    now: DateTime<Utc>,
) -> CmdResult<()> {
    match json_data::import(pool, data_dir, path, now).await {
        Ok(kept) => {
            log::info!("all data imported from JSON; previous data kept as {kept}");
            Ok(())
        }
        Err(e) => {
            match &e {
                ImportError::Replace(_)
                | ImportError::Db(_)
                | ImportError::Migrate(_)
                | ImportError::Io(_)
                | ImportError::Read(_) => log::error!("JSON import failed: {e}"),
                // The file was refused; its contents are not logged.
                _ => log::warn!("JSON import refused: {e}"),
            }
            Err(import_error(&e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{self, BackupKind};
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    /// Where a test writes an export (kept apart from the data folder).
    fn export_path(dir: &Path) -> std::path::PathBuf {
        dir.join(default_file_name(d("2026-11-19")))
    }

    async fn file_db(dir: &Path) -> SqlitePool {
        db::open(&dir.join(db::DB_FILE_NAME)).await.unwrap()
    }

    #[test]
    fn default_name_has_the_date() {
        assert_eq!(default_file_name(d("2026-11-19")), "higiri-2026-11-19.json");
    }

    #[tokio::test]
    async fn export_then_import_and_undo_from_the_kept_copy() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = file_db(src_dir.path()).await;
        crate::sample::seed_if_empty(&src, d("2026-11-19"))
            .await
            .unwrap();
        let out = tempfile::tempdir().unwrap();
        let path = export_path(out.path());
        export(&src, &path, "0.1.0", ts(2026, 11, 19, 1, 0))
            .await
            .unwrap();
        // Only the file: no .part left behind.
        assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 1);

        let dst_dir = tempfile::tempdir().unwrap();
        let dst = file_db(dst_dir.path()).await;
        import(&dst, dst_dir.path(), &path, ts(2026, 11, 19, 2, 0))
            .await
            .unwrap();
        let count = |pool: SqlitePool| async move {
            let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM todos")
                .fetch_one(&pool)
                .await
                .unwrap();
            n.0
        };
        assert_eq!(count(dst.clone()).await, count(src.clone()).await);

        // The data before the import is listed and brings back the empty database.
        let kept: Vec<_> = backup::list(dst_dir.path())
            .unwrap()
            .into_iter()
            .filter(|b| b.kind == BackupKind::BeforeRestore)
            .collect();
        assert_eq!(kept.len(), 1);
        super::super::backup::restore(
            &dst,
            dst_dir.path(),
            &kept[0].file_name,
            ts(2026, 11, 19, 3, 0),
        )
        .await
        .unwrap();
        assert_eq!(count(dst).await, 0);
    }

    #[tokio::test]
    async fn refusals_are_explained_in_japanese_and_change_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        crate::sample::seed_if_empty(&pool, d("2026-11-19"))
            .await
            .unwrap();
        let files = tempfile::tempdir().unwrap();
        let cases: [(&str, &str, &str); 6] = [
            (
                "broken.json",
                "{\"format\": \"higiri-",
                "JSONとして読めない",
            ),
            (
                "other.json",
                "{\"name\": \"x\"}",
                "Higiriで書き出したJSONではない",
            ),
            (
                "newer.json",
                "{\"format\":\"higiri-export\",\"format_version\":2}",
                "新しい版",
            ),
            (
                "shape.json",
                "{\"format\":\"higiri-export\",\"format_version\":1,\"schema_version\":1,\"projects\":{}}",
                "読めない所",
            ),
            (
                "fk.json",
                r#"{"format":"higiri-export","format_version":1,"schema_version":1,
                    "projects":[],"work_logs":[],"todo_events":[],"reports":[],"settings":{},
                    "todos":[{"id":1,"project_id":9,"title":"T","status":"todo","priority":2,
                              "memo_md":"","sort_order":0,
                              "created_at":"2026-11-19T00:00:00.000Z","updated_at":"2026-11-19T00:00:00.000Z"}]}"#,
                "つじつまの合わない所",
            ),
            (
                "date.json",
                r#"{"format":"higiri-export","format_version":1,"schema_version":1,
                    "todos":[],"work_logs":[],"todo_events":[],"reports":[],"settings":{},
                    "projects":[{"id":1,"name":"P","created_at":"2026/11/19","updated_at":"2026-11-19T00:00:00.000Z"}]}"#,
                "アプリが読めない値",
            ),
        ];
        for (name, body, says) in cases {
            let path = files.path().join(name);
            std::fs::write(&path, body).unwrap();
            let err = import(&pool, tmp.path(), &path, ts(2026, 11, 19, 2, 0))
                .await
                .unwrap_err();
            assert_eq!(err.kind, ErrorKind::Validation, "{name}");
            assert!(err.message.contains(says), "{name}: {}", err.message);
            assert!(
                err.message.contains("今のデータは変わっていません"),
                "{name}"
            );
        }
        let missing = import(
            &pool,
            tmp.path(),
            &files.path().join("missing.json"),
            ts(2026, 11, 19, 2, 0),
        )
        .await
        .unwrap_err();
        assert!(missing.message.contains("ファイルを読めませんでした"));

        // No copy of the current data was taken.
        assert!(backup::list(tmp.path()).unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failed_write_leaves_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        let folder = tmp.path().join("no-such-folder");
        let err = export(
            &pool,
            &folder.join("higiri.json"),
            "0.1.0",
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap_err();
        assert!(err.message.contains("書き出せませんでした"));
        assert!(!folder.exists());
    }
}
