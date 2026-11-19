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

/// Builds the report from that day's records with the report template. The
/// 所感・連絡事項 part is taken from `current_md` (the text being edited) when
/// given, otherwise from the saved report. Nothing is saved.
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
    let template = &settings.report_template;
    let note = match current_md {
        Some(md) => report::split_note(md, template),
        None => repo::reports::get(&mut conn, key.kind, key.date)
            .await?
            .map(|r| r.note_md)
            .unwrap_or_default(),
    };
    let body = report::generate_body(
        &ReportInput {
            date: key.date,
            rules: &Rules::from_settings(&settings),
            projects: &projects,
            todos: &todos,
            logs: &logs,
            events: &events,
        },
        template,
    );
    Ok(report::compose(&body, &note, template))
}

pub async fn save(
    pool: &SqlitePool,
    key: ReportKey,
    body_md: &str,
    now: DateTime<Utc>,
) -> CmdResult<Report> {
    daily_only(key)?;
    let mut conn = pool.acquire().await?;
    let template = repo::settings::load(&mut conn).await?.report_template;
    Ok(repo::reports::upsert(
        &mut conn,
        &Report {
            kind: key.kind,
            period_start: key.date,
            period_end: key.date,
            body_md: body_md.to_owned(),
            note_md: report::split_note(body_md, &template),
            updated_at: now,
        },
    )
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::SettingsInput;
    use crate::domain::report_template::{ReportTemplate, Section, SectionBody, SectionKind};
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

    /// Saves `template` as the report template.
    async fn use_template(pool: &SqlitePool, template: ReportTemplate) {
        let current = crate::commands::settings::get(pool).await.unwrap();
        crate::commands::settings::update(
            pool,
            SettingsInput {
                report_template: template,
                ..SettingsInput::from(&current)
            },
        )
        .await
        .unwrap();
    }

    fn edit(t: &mut ReportTemplate, kind: SectionKind, f: impl FnOnce(&mut Section)) {
        f(t.sections.iter_mut().find(|s| s.kind() == kind).unwrap());
    }

    #[tokio::test]
    async fn the_template_sets_order_headings_and_items() {
        let pool = seeded().await;
        let mut t = ReportTemplate {
            title: "{日付}({曜日}) 業務報告".into(),
            ..ReportTemplate::default()
        };
        for s in t.sections.iter_mut() {
            match &mut s.body {
                SectionBody::Work(o) => {
                    o.project_headings = false;
                    o.status = false;
                }
                SectionBody::Closed(o) => o.close_reason = false,
                SectionBody::Waiting(o) => {
                    o.waiting_on = false;
                    o.waiting_days = false;
                    o.follow_up_date = false;
                }
                SectionBody::Tomorrow(o) => o.planned_period = false,
                SectionBody::Attention(o) => {
                    o.days = false;
                    o.waiting_mark = false;
                    o.follow_up_and_late_start = true;
                }
                SectionBody::Note(_) => s.heading = "連絡事項".into(),
            }
        }
        // 日限注意 first, 本日の作業 last; 待ち not shown.
        t.sections.swap(0, 4);
        edit(&mut t, SectionKind::Waiting, |s| s.enabled = false);
        edit(&mut t, SectionKind::Closed, |s| {
            s.heading = "完了・中止".into()
        });
        use_template(&pool, t).await;
        let text = generate(&pool, key("2026-11-19"), None).await.unwrap();
        assert_eq!(
            text,
            concat!(
                "# 2026-11-19(木) 業務報告\n",
                "\n",
                "## 日限注意\n",
                "- 【超過】社内ツール整備 / ライセンス棚卸し(日限 11/17)\n",
                "- 【本日】基幹システム更改 / 移行手順書のドラフト(日限 11/19)\n",
                "- 【間近】基幹システム更改 / API設計書のレビュー対応(日限 11/20)\n",
                "- 【要確認】採用サイト改修 / 募集要項ページの原稿確認(確認日 11/19)\n",
                "- 【着手遅れ】基幹システム更改 / 性能試験の環境準備(予定開始 11/17)\n",
                "\n",
                "## 完了・中止\n",
                "- 【完了】社内ツール整備 / デプロイ手順書の更新\n",
                "- 【中止】基幹システム更改 / 旧帳票の移行検証\n",
                "\n",
                "## 明日の予定\n",
                "- 基幹システム更改 / 移行手順書のドラフト\n",
                "- 基幹システム更改 / 結合テスト計画の作成\n",
                "- 基幹システム更改 / 性能試験の環境準備\n",
                "- 社内ツール整備 / ライセンス棚卸し\n",
                "\n",
                "## 本日の作業\n",
                "- 基幹システム更改 / **API設計書のレビュー対応**(日限 11/20)1.5h\n",
                "  - 指摘12件のうち9件を反映\n",
                "  - 残り3件は認証まわり。認証担当に質問を送り、回答待ち\n",
                "  - [ ] 指摘10:トークン更新の手順を追記\n",
                "- 基幹システム更改 / **移行手順書のドラフト**(日限 11/19)2.0h\n",
                "  - 切り戻し手順の章を書き終え、レビューを依頼した\n",
                "- 社内ツール整備 / **デプロイ手順書の更新** 0.5h\n",
                "  - 新しい承認フローに合わせて手順4〜6を書き換えた\n",
                "\n",
                "## 連絡事項\n",
            )
        );
    }

    #[tokio::test]
    async fn items_can_be_left_out_one_by_one() {
        let pool = seeded().await;
        let mut t = ReportTemplate::default();
        for s in t.sections.iter_mut() {
            match &mut s.body {
                SectionBody::Work(o) => {
                    o.status = false;
                    o.due_date = false;
                }
                SectionBody::Waiting(o) => o.waiting_days = false,
                SectionBody::Attention(o) => o.follow_up_and_late_start = true,
                _ => {}
            }
        }
        use_template(&pool, t).await;
        let text = generate(&pool, key("2026-11-19"), None).await.unwrap();
        // Project headings stay; no parentheses without status and due date.
        assert!(
            text.contains(concat!(
                "## 本日の作業\n",
                "\n",
                "### 基幹システム更改\n",
                "- **API設計書のレビュー対応** 1.5h\n",
            )),
            "{text}"
        );
        assert!(
            text.contains(concat!(
                "## 待ち\n",
                "- 採用サイト改修 / 募集要項ページの原稿確認(人事部の原稿確認待ち、確認日 11/19)\n",
                "- 基幹システム更改 / API設計書のレビュー対応(認証担当の回答待ち、確認日 11/20)\n",
            )),
            "{text}"
        );
        assert!(
            text.contains(concat!(
                "- 【間近】基幹システム更改 / API設計書のレビュー対応(日限 11/20、あと1日、待ち)\n",
                "- 【要確認】採用サイト改修 / 募集要項ページの原稿確認(確認日 11/19、待ち3日目)\n",
                "- 【着手遅れ】基幹システム更改 / 性能試験の環境準備(予定開始 11/17、2日経過)\n",
            )),
            "{text}"
        );
    }

    #[tokio::test]
    async fn the_note_survives_heading_changes() {
        let pool = seeded().await;
        let k = key("2026-11-19");
        let with_note = |heading: &str| {
            let mut t = ReportTemplate::default();
            edit(&mut t, SectionKind::Note, |s| s.heading = heading.into());
            t
        };
        let first = generate(&pool, k, None).await.unwrap();
        save(
            &pool,
            k,
            &format!("{first}- 所感を書いた。\n"),
            ts(2026, 11, 19, 10, 0),
        )
        .await
        .unwrap();

        // 所感・連絡事項 → 連絡: found by the default heading.
        use_template(&pool, with_note("連絡")).await;
        let saved = get(&pool, k).await.unwrap().unwrap();
        assert_eq!(saved.note_md, "- 所感を書いた。");
        let second = generate(&pool, k, Some(&saved.body_md)).await.unwrap();
        assert!(
            second.ends_with("\n\n## 連絡\n- 所感を書いた。\n"),
            "{second}"
        );
        save(&pool, k, &second, ts(2026, 11, 19, 11, 0))
            .await
            .unwrap();

        // 連絡 → メモ: neither the heading nor the default is in the text,
        // but 「## 連絡」 is not a section's heading.
        use_template(&pool, with_note("メモ")).await;
        let saved = get(&pool, k).await.unwrap().unwrap();
        let third = generate(&pool, k, Some(&saved.body_md)).await.unwrap();
        assert!(
            third.ends_with("\n\n## メモ\n- 所感を書いた。\n"),
            "{third}"
        );
        // Saving with the heading not found keeps the note apart as well.
        let saved = save(&pool, k, &saved.body_md, ts(2026, 11, 19, 12, 0))
            .await
            .unwrap();
        assert_eq!(saved.note_md, "- 所感を書いた。");
        // And from the saved report, too.
        assert_eq!(generate(&pool, k, None).await.unwrap(), third);
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
