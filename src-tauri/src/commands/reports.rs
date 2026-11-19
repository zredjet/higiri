//! 日報: generate_report, save_report, get_report, export_report_file.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::clock;
use super::error::{AppError, CmdResult, ErrorKind};
use crate::domain::report::{self, Report, ReportInput, ReportKind};
use crate::domain::urgency::Rules;
use crate::repo;
use crate::repo::todos::TodoFilter;

/// Which report: kind and the first day of its period.
#[derive(Debug, Clone, Copy, Deserialize, specta::Type)]
pub struct ReportKey {
    pub kind: ReportKind,
    pub date: NaiveDate,
}

fn daily_only(key: ReportKey) -> CmdResult<()> {
    if key.kind != ReportKind::Daily {
        return Err(AppError::validation(
            "週報と月報はまだ作れません。日報を選んでください。",
        ));
    }
    Ok(())
}

/// The saved report of that day, or null.
#[tauri::command]
#[specta::specta]
pub async fn get_report(pool: State<'_, SqlitePool>, key: ReportKey) -> CmdResult<Option<Report>> {
    get(&pool, key).await
}

/// Builds the report from that day's records. The 所感・連絡事項 part is taken
/// from `current_md` (the text being edited) when given, otherwise from the
/// saved report. Nothing is saved.
#[tauri::command]
#[specta::specta]
pub async fn generate_report(
    pool: State<'_, SqlitePool>,
    key: ReportKey,
    current_md: Option<String>,
) -> CmdResult<String> {
    generate(&pool, key, current_md.as_deref()).await
}

/// Saves the full text; its 所感・連絡事項 part is stored apart as well.
#[tauri::command]
#[specta::specta]
pub async fn save_report(
    pool: State<'_, SqlitePool>,
    key: ReportKey,
    body_md: String,
) -> CmdResult<Report> {
    save(&pool, key, &body_md, clock::now()).await
}

/// Asks where to save (the save dialog opens on the Rust side) and writes the
/// text as a .md file. Returns the path, or null when the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn export_report_file(
    app: AppHandle,
    key: ReportKey,
    body_md: String,
) -> CmdResult<Option<String>> {
    daily_only(key)?;
    let dialog = app
        .dialog()
        .file()
        .set_title("日報を保存")
        .set_file_name(format!("日報_{}.md", key.date))
        .add_filter("Markdown", &["md"]);
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file())
        .await
        .map_err(|_| AppError::internal())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|_| write_error())?;
    std::fs::write(&path, body_md).map_err(|e| {
        log::error!("report export failed: {e}");
        write_error()
    })?;
    Ok(Some(path.display().to_string()))
}

fn write_error() -> AppError {
    AppError {
        kind: ErrorKind::Internal,
        message: "ファイルを保存できませんでした。保存先を変えて、もう一度保存してください。"
            .into(),
    }
}

pub async fn get(pool: &SqlitePool, key: ReportKey) -> CmdResult<Option<Report>> {
    daily_only(key)?;
    let mut conn = pool.acquire().await?;
    Ok(repo::reports::get(&mut conn, key.kind, key.date).await?)
}

pub async fn generate(
    pool: &SqlitePool,
    key: ReportKey,
    current_md: Option<&str>,
) -> CmdResult<String> {
    daily_only(key)?;
    let mut conn = pool.acquire().await?;
    let settings = repo::settings::load(&mut conn).await?;
    let projects = repo::projects::list(&mut conn, true).await?;
    let todos = repo::todos::list(
        &mut conn,
        &TodoFilter {
            include_archived_projects: true,
            ..TodoFilter::default()
        },
    )
    .await?;
    let logs = repo::work_logs::list_by_date(&mut conn, key.date).await?;
    let events = repo::events::list_by_date(&mut conn, key.date).await?;
    let note = match current_md {
        Some(md) => report::split_note(md),
        None => repo::reports::get(&mut conn, key.kind, key.date)
            .await?
            .map(|r| r.note_md)
            .unwrap_or_default(),
    };
    let body = report::generate_body(&ReportInput {
        date: key.date,
        rules: &Rules::from_settings(&settings),
        projects: &projects,
        todos: &todos,
        logs: &logs,
        events: &events,
    });
    Ok(report::compose(&body, &note))
}

pub async fn save(
    pool: &SqlitePool,
    key: ReportKey,
    body_md: &str,
    now: DateTime<Utc>,
) -> CmdResult<Report> {
    daily_only(key)?;
    let mut conn = pool.acquire().await?;
    Ok(repo::reports::upsert(
        &mut conn,
        &Report {
            kind: key.kind,
            period_start: key.date,
            period_end: key.date,
            body_md: body_md.to_owned(),
            note_md: report::split_note(body_md),
            updated_at: now,
        },
    )
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    /// docs/requirements-design.md「日報の生成」の出力例(そのまま).
    const DESIGN_EXAMPLE: &str = include_str!("../../tests/fixtures/daily_report_2026-11-19.md");

    fn key(date: &str) -> ReportKey {
        ReportKey {
            kind: ReportKind::Daily,
            date: d(date),
        }
    }

    async fn seeded() -> SqlitePool {
        let pool = db::open_in_memory().await.unwrap();
        crate::sample::seed_if_empty(&pool, d("2026-11-19"))
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn sample_day_reproduces_the_design_example_exactly() {
        let pool = seeded().await;
        let note = "## 所感・連絡事項\n- 移行手順書は今日が日限。レビュー結果を受けて、明日の午前に確定する。\n";
        let text = generate(&pool, key("2026-11-19"), Some(note))
            .await
            .unwrap();
        assert_eq!(text, DESIGN_EXAMPLE);
    }

    #[tokio::test]
    async fn regenerating_keeps_the_saved_note() {
        let pool = seeded().await;
        let k = key("2026-11-19");
        let first = generate(&pool, k, None).await.unwrap();
        assert!(first.ends_with("## 所感・連絡事項\n"));
        let edited = format!("{first}- 所感を書いた。\n").replace("## 待ち", "## 待ち(手で直した)");
        let saved = save(&pool, k, &edited, ts(2026, 11, 19, 10, 0))
            .await
            .unwrap();
        assert_eq!(saved.note_md, "- 所感を書いた。");
        assert_eq!(get(&pool, k).await.unwrap(), Some(saved));

        // Regenerating from the saved report: the note stays, hand edits above it do not.
        let again = generate(&pool, k, None).await.unwrap();
        assert!(
            again.ends_with("## 所感・連絡事項\n- 所感を書いた。\n"),
            "{again}"
        );
        assert!(again.contains("\n## 待ち\n"));
    }

    #[tokio::test]
    async fn other_days_and_kinds() {
        let pool = seeded().await;
        // No logs and no closures on 11/20, as seen from that day.
        let text = generate(&pool, key("2026-11-20"), None).await.unwrap();
        assert!(
            text.starts_with("# 日報 2026-11-20(金)\n\n## 本日の作業\n\n- なし\n"),
            "{text}"
        );
        assert!(text.contains("## 本日完了・中止\n- なし\n"));
        let weekly = ReportKey {
            kind: ReportKind::Weekly,
            date: d("2026-11-16"),
        };
        assert_eq!(
            generate(&pool, weekly, None).await.unwrap_err().kind,
            ErrorKind::Validation
        );
        assert_eq!(get(&pool, key("2026-11-21")).await.unwrap(), None);
    }
}
