//! 日報と週報: generate_report, save_report, get_report, export_report_file.
//!
//! The weekly report (v0.3.0) goes through the same commands with
//! `kind: "weekly"`: `date` may be any day of the week and is moved to its
//! Monday here. It is saved in `reports` as `weekly`, from Monday to Sunday.
//! 月報 is refused as before.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::{SqliteConnection, SqlitePool};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::clock;
use super::error::{AppError, CmdResult, ErrorKind};
use crate::domain::report::{self, Report, ReportInput, ReportKind};
use crate::domain::report_template::Template;
use crate::domain::urgency::Rules;
use crate::domain::weekly_report::{self, WeeklyReportInput, week_end, week_start};
use crate::repo;
use crate::repo::todos::TodoFilter;

/// Which report: kind and a day of its period (for a weekly report, any day
/// of the week).
#[derive(Debug, Clone, Copy, Deserialize, specta::Type)]
pub struct ReportKey {
    pub kind: ReportKind,
    pub date: NaiveDate,
}

/// The period a key stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Period {
    kind: ReportKind,
    /// The day of a daily report; the Monday of a weekly one.
    start: NaiveDate,
    /// The same day; the Sunday.
    end: NaiveDate,
}

impl Period {
    fn of(key: ReportKey) -> CmdResult<Period> {
        match key.kind {
            ReportKind::Daily => Ok(Period {
                kind: key.kind,
                start: key.date,
                end: key.date,
            }),
            ReportKind::Weekly => {
                let start = week_start(key.date);
                Ok(Period {
                    kind: key.kind,
                    start,
                    end: week_end(start),
                })
            }
            ReportKind::Monthly => Err(AppError::validation(
                "月報はまだ作れません。日報か週報を選んでください。",
            )),
        }
    }

    /// 「日報」「週報」.
    fn name(self) -> &'static str {
        match self.kind {
            ReportKind::Weekly => "週報",
            _ => "日報",
        }
    }
}

/// The saved report of that day (or week), or null.
#[tauri::command]
#[specta::specta]
pub async fn get_report(pool: State<'_, SqlitePool>, key: ReportKey) -> CmdResult<Option<Report>> {
    get(&pool, key).await
}

/// Builds the report from that day's (or week's) records with its template.
/// The 所感・連絡事項 part is taken from `current_md` (the text being edited)
/// when given, otherwise from the saved report. Nothing is saved.
#[tauri::command]
#[specta::specta]
pub async fn generate_report(
    pool: State<'_, SqlitePool>,
    key: ReportKey,
    current_md: Option<String>,
) -> CmdResult<String> {
    generate(&pool, key, current_md.as_deref(), clock::today()).await
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
    let period = Period::of(key)?;
    let dialog = app
        .dialog()
        .file()
        .set_title(format!("{}を保存", period.name()))
        .set_file_name(file_name(period))
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

/// 「日報_2026-11-19.md」「週報_2026-11-16.md」 (the Monday).
fn file_name(period: Period) -> String {
    format!("{}_{}.md", period.name(), period.start)
}

fn write_error() -> AppError {
    AppError {
        kind: ErrorKind::Internal,
        message: "ファイルを保存できませんでした。保存先を変えて、もう一度保存してください。"
            .into(),
    }
}

pub async fn get(pool: &SqlitePool, key: ReportKey) -> CmdResult<Option<Report>> {
    let period = Period::of(key)?;
    let mut conn = pool.acquire().await?;
    Ok(repo::reports::get(&mut conn, period.kind, period.start).await?)
}

/// The 所感・連絡事項 to keep: from the text being edited, or the saved report.
async fn note_of<T: Template>(
    conn: &mut SqliteConnection,
    period: Period,
    current_md: Option<&str>,
    template: &T,
) -> CmdResult<String> {
    Ok(match current_md {
        Some(md) => report::split_note(md, template),
        None => repo::reports::get(conn, period.kind, period.start)
            .await?
            .map(|r| r.note_md)
            .unwrap_or_default(),
    })
}

/// `today` is the PC's today: a weekly report judges 待ち and 日限注意 on the
/// earlier of its Sunday and today (a daily report on its own day).
pub async fn generate(
    pool: &SqlitePool,
    key: ReportKey,
    current_md: Option<&str>,
    today: NaiveDate,
) -> CmdResult<String> {
    let period = Period::of(key)?;
    let mut conn = pool.acquire().await?;
    let settings = repo::settings::load(&mut conn).await?;
    let rules = Rules::from_settings(&settings);
    let projects = repo::projects::list(&mut conn, true).await?;
    let todos = repo::todos::list(
        &mut conn,
        &TodoFilter {
            include_archived_projects: true,
            ..TodoFilter::default()
        },
    )
    .await?;
    if period.kind == ReportKind::Weekly {
        let logs = repo::work_logs::list_between(&mut conn, period.start, period.end).await?;
        let events = repo::events::list_between(&mut conn, period.start, period.end).await?;
        let template = &settings.weekly_report_template;
        let note = note_of(&mut conn, period, current_md, template).await?;
        let body = weekly_report::generate_body(
            &WeeklyReportInput {
                date: period.start,
                today,
                rules: &rules,
                projects: &projects,
                todos: &todos,
                logs: &logs,
                events: &events,
            },
            template,
        );
        return Ok(report::compose(&body, &note, template));
    }
    let logs = repo::work_logs::list_by_date(&mut conn, period.start).await?;
    let events = repo::events::list_by_date(&mut conn, period.start).await?;
    let template = &settings.report_template;
    let note = note_of(&mut conn, period, current_md, template).await?;
    let body = report::generate_body(
        &ReportInput {
            date: period.start,
            rules: &rules,
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
    let period = Period::of(key)?;
    let mut conn = pool.acquire().await?;
    let settings = repo::settings::load(&mut conn).await?;
    let note_md = match period.kind {
        ReportKind::Weekly => report::split_note(body_md, &settings.weekly_report_template),
        _ => report::split_note(body_md, &settings.report_template),
    };
    Ok(repo::reports::upsert(
        &mut conn,
        &Report {
            kind: period.kind,
            period_start: period.start,
            period_end: period.end,
            body_md: body_md.to_owned(),
            note_md,
            updated_at: now,
        },
    )
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::SettingsInput;
    use crate::domain::report_template::{
        ReportTemplate, Section, SectionBody, SectionKind, WeeklyReportTemplate, WeeklySection,
        WeeklySectionBody, WeeklySectionKind,
    };
    use crate::domain::testing::{d, ts};
    use crate::repo::db;
    use crate::repo::work_logs::NewWorkLog;

    /// docs/requirements-design.md「日報の生成」の出力例(そのまま).
    const DESIGN_EXAMPLE: &str = include_str!("../../tests/fixtures/daily_report_2026-11-19.md");

    /// The sample's day (the mock's today).
    const TODAY: NaiveDate = NaiveDate::from_ymd_opt(2026, 11, 19).unwrap();

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
        let text = generate(&pool, key("2026-11-19"), Some(note), TODAY)
            .await
            .unwrap();
        assert_eq!(text, DESIGN_EXAMPLE);
    }

    #[tokio::test]
    async fn regenerating_keeps_the_saved_note() {
        let pool = seeded().await;
        let k = key("2026-11-19");
        let first = generate(&pool, k, None, TODAY).await.unwrap();
        assert!(first.ends_with("## 所感・連絡事項\n"));
        let edited = format!("{first}- 所感を書いた。\n").replace("## 待ち", "## 待ち(手で直した)");
        let saved = save(&pool, k, &edited, ts(2026, 11, 19, 10, 0))
            .await
            .unwrap();
        assert_eq!(saved.note_md, "- 所感を書いた。");
        assert_eq!(get(&pool, k).await.unwrap(), Some(saved));

        // Regenerating from the saved report: the note stays, hand edits above it do not.
        let again = generate(&pool, k, None, TODAY).await.unwrap();
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
        let text = generate(&pool, key("2026-11-19"), None, TODAY)
            .await
            .unwrap();
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
        let text = generate(&pool, key("2026-11-19"), None, TODAY)
            .await
            .unwrap();
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
        let first = generate(&pool, k, None, TODAY).await.unwrap();
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
        let second = generate(&pool, k, Some(&saved.body_md), TODAY)
            .await
            .unwrap();
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
        let third = generate(&pool, k, Some(&saved.body_md), TODAY)
            .await
            .unwrap();
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
        assert_eq!(generate(&pool, k, None, TODAY).await.unwrap(), third);
    }

    #[tokio::test]
    async fn other_days_and_kinds() {
        let pool = seeded().await;
        // No logs and no closures on 11/20, as seen from that day.
        let text = generate(&pool, key("2026-11-20"), None, TODAY)
            .await
            .unwrap();
        assert!(
            text.starts_with("# 日報 2026-11-20(金)\n\n## 本日の作業\n\n- なし\n"),
            "{text}"
        );
        assert!(text.contains("## 本日完了・中止\n- なし\n"));
        let monthly = ReportKey {
            kind: ReportKind::Monthly,
            date: d("2026-11-01"),
        };
        for e in [
            generate(&pool, monthly, None, TODAY).await.unwrap_err(),
            save(&pool, monthly, "# 月報", ts(2026, 11, 19, 10, 0))
                .await
                .unwrap_err(),
            get(&pool, monthly).await.unwrap_err(),
        ] {
            assert_eq!(e.kind, ErrorKind::Validation);
            assert_eq!(
                e.message,
                "月報はまだ作れません。日報か週報を選んでください。"
            );
        }
        assert_eq!(get(&pool, key("2026-11-21")).await.unwrap(), None);
    }

    // 週報 (R-6, v0.3.0)

    /// The sample's week seen on the sample's day, from the daily report's
    /// data and [`WEEK_LOGS`] (default template).
    const WEEKLY_FIXTURE: &str = include_str!("../../tests/fixtures/weekly_report_2026-11-16.md");

    const WEEKLY_NOTE: &str =
        "## 所感・連絡事項\n- 認証担当の回答が来週にずれ込む見込み。API設計書の日限を相談する。\n";

    fn weekly(date: &str) -> ReportKey {
        ReportKey {
            kind: ReportKind::Weekly,
            date: d(date),
        }
    }

    /// (TODO title, date, minutes, body): logs on the week's other days (one
    /// without a time, one of a child), and one on the Sunday before.
    const WEEK_LOGS: [(&str, &str, Option<i64>, &str); 5] = [
        (
            "移行手順書のドラフト",
            "2026-11-16",
            Some(120),
            "- 移行の対象と順番を洗い出した",
        ),
        (
            "移行手順書のドラフト",
            "2026-11-18",
            None,
            "- 切り戻し手順の章を書き始めた",
        ),
        (
            "必要なツールの洗い出し",
            "2026-11-17",
            Some(60),
            "- 各チームが使っているツールを聞き取った",
        ),
        (
            "募集要項ページの原稿確認",
            "2026-11-16",
            None,
            "- 原稿の初稿を人事部に送った",
        ),
        (
            "ライセンス棚卸し",
            "2026-11-15",
            Some(30),
            "- 契約中のライセンスを一覧にした",
        ),
    ];

    async fn week_seeded() -> SqlitePool {
        let pool = seeded().await;
        let mut c = pool.acquire().await.unwrap();
        let todos = repo::todos::list(
            &mut c,
            &TodoFilter {
                include_archived_projects: true,
                ..TodoFilter::default()
            },
        )
        .await
        .unwrap();
        for (title, date, minutes, body) in WEEK_LOGS {
            let todo = todos.iter().find(|t| t.title == title).unwrap();
            repo::work_logs::insert(
                &mut c,
                &NewWorkLog {
                    todo_id: todo.id,
                    log_date: d(date),
                    body_md: body.into(),
                    minutes,
                },
                ts(2026, 11, 19, 8, 0),
            )
            .await
            .unwrap();
        }
        drop(c);
        pool
    }

    /// Saves `template` as the weekly report template.
    async fn use_weekly_template(pool: &SqlitePool, template: WeeklyReportTemplate) {
        let current = crate::commands::settings::get(pool).await.unwrap();
        crate::commands::settings::update(
            pool,
            SettingsInput {
                weekly_report_template: template,
                ..SettingsInput::from(&current)
            },
        )
        .await
        .unwrap();
    }

    fn edit_weekly(
        t: &mut WeeklyReportTemplate,
        kind: WeeklySectionKind,
        f: impl FnOnce(&mut WeeklySection),
    ) {
        f(t.sections.iter_mut().find(|s| s.kind() == kind).unwrap());
    }

    #[tokio::test]
    async fn sample_week_reproduces_the_fixture_exactly() {
        let pool = week_seeded().await;
        let text = generate(&pool, weekly("2026-11-19"), Some(WEEKLY_NOTE), TODAY)
            .await
            .unwrap();
        assert_eq!(text, WEEKLY_FIXTURE);
        // The daily report of the day is still the design's example.
        let note = "## 所感・連絡事項\n- 移行手順書は今日が日限。レビュー結果を受けて、明日の午前に確定する。\n";
        assert_eq!(
            generate(&pool, key("2026-11-19"), Some(note), TODAY)
                .await
                .unwrap(),
            DESIGN_EXAMPLE
        );
    }

    #[tokio::test]
    async fn weekly_reports_are_kept_by_their_monday() {
        let pool = week_seeded().await;
        let first = generate(&pool, weekly("2026-11-18"), None, TODAY)
            .await
            .unwrap();
        assert!(
            first.starts_with("# 週報 2026-11-16〜2026-11-22\n\n## 今週の作業\n"),
            "{first}"
        );
        assert!(first.ends_with("\n\n## 所感・連絡事項\n"), "{first}");
        // Any day of the week opens the same report.
        for day in ["2026-11-16", "2026-11-22"] {
            assert_eq!(
                generate(&pool, weekly(day), None, TODAY).await.unwrap(),
                first
            );
        }
        let saved = save(
            &pool,
            weekly("2026-11-19"),
            &format!("{first}- 所感を書いた。\n"),
            ts(2026, 11, 19, 10, 0),
        )
        .await
        .unwrap();
        assert_eq!(
            (saved.kind, saved.period_start, saved.period_end),
            (ReportKind::Weekly, d("2026-11-16"), d("2026-11-22"))
        );
        assert_eq!(saved.note_md, "- 所感を書いた。");
        for day in ["2026-11-16", "2026-11-22"] {
            assert_eq!(get(&pool, weekly(day)).await.unwrap(), Some(saved.clone()));
        }
        // Not the daily report of the Monday, nor the next week's.
        assert_eq!(get(&pool, key("2026-11-16")).await.unwrap(), None);
        assert_eq!(get(&pool, weekly("2026-11-23")).await.unwrap(), None);
        // Regenerating keeps the note.
        let again = generate(&pool, weekly("2026-11-20"), None, TODAY)
            .await
            .unwrap();
        assert_eq!(again, format!("{first}- 所感を書いた。\n"));

        // 「週報_2026-11-16.md」, named by the Monday.
        assert_eq!(
            file_name(Period::of(weekly("2026-11-21")).unwrap()),
            "週報_2026-11-16.md"
        );
        assert_eq!(
            file_name(Period::of(key("2026-11-19")).unwrap()),
            "日報_2026-11-19.md"
        );
    }

    #[tokio::test]
    async fn a_past_week_is_seen_on_its_sunday() {
        let pool = week_seeded().await;
        // 11/9〜11/15, judged on 11/15.
        let text = generate(&pool, weekly("2026-11-12"), None, TODAY)
            .await
            .unwrap();
        for part in [
            "# 週報 2026-11-09〜2026-11-15\n",
            concat!(
                "## 今週の作業\n",
                "\n",
                "作業時間の合計 0.5h\n",
                "\n",
                "### 社内ツール整備\n",
                "- **ライセンス棚卸し**(未着手 / 日限 11/17 / 作業日 11/15)0.5h\n",
                "\n",
            ),
            "## 今週完了・中止\n- 【完了】基幹システム更改 / 要件定義書の改訂(11/13)\n\n",
            // Both started waiting after 11/15.
            "## 待ち\n- なし\n",
            "## 日限注意\n- 【間近】社内ツール整備 / ライセンス棚卸し(日限 11/17、あと2日)\n\n",
        ] {
            assert!(text.contains(part), "{part}\n---\n{text}");
        }
        // The week after: nothing done yet, judged on today.
        let next = generate(&pool, weekly("2026-11-25"), None, TODAY)
            .await
            .unwrap();
        assert!(
            next.starts_with("# 週報 2026-11-23〜2026-11-29\n\n## 今週の作業\n\n- なし\n\n## 今週完了・中止\n- なし\n"),
            "{next}"
        );
        assert!(
            next.contains("- 【本日】基幹システム更改 / 移行手順書のドラフト(日限 11/19)\n"),
            "{next}"
        );
    }

    #[tokio::test]
    async fn the_weekly_template_sets_order_headings_and_items() {
        let pool = week_seeded().await;
        let mut t = WeeklyReportTemplate {
            title: "{開始日}〜{終了日} 週次報告".into(),
            ..WeeklyReportTemplate::default()
        };
        for s in t.sections.iter_mut() {
            match &mut s.body {
                WeeklySectionBody::Work(o) => {
                    o.project_headings = false;
                    o.work_days = false;
                    o.total_time = false;
                    o.log_body = true;
                }
                WeeklySectionBody::Closed(o) => o.closed_date = false,
                WeeklySectionBody::NextWeek(o) => o.due_date = false,
                WeeklySectionBody::Note(_) => s.heading = "連絡事項".into(),
                _ => {}
            }
        }
        // 日限注意 first, 今週の作業 after 来週の予定; 待ち not shown.
        t.sections.swap(0, 4);
        edit_weekly(&mut t, WeeklySectionKind::Waiting, |s| s.enabled = false);
        edit_weekly(&mut t, WeeklySectionKind::Closed, |s| {
            s.heading = "完了・中止".into()
        });
        use_weekly_template(&pool, t).await;
        let text = generate(&pool, weekly("2026-11-19"), None, TODAY)
            .await
            .unwrap();
        assert!(
            text.starts_with(
                "# 2026-11-16〜2026-11-22 週次報告\n\n## 日限注意\n- 【超過】社内ツール整備 / ライセンス棚卸し(日限 11/17、2日超過)\n"
            ),
            "{text}"
        );
        assert!(!text.contains("## 待ち"), "{text}");
        assert!(
            text.contains(concat!(
                "## 完了・中止\n",
                "- 【完了】社内ツール整備 / 開発環境の標準化 / 必要なツールの洗い出し\n",
                "- 【完了】社内ツール整備 / デプロイ手順書の更新\n",
                "- 【中止】基幹システム更改 / 旧帳票の移行検証(理由:旧帳票の廃止が決まったため)\n",
            )),
            "{text}"
        );
        assert!(
            text.contains("## 来週の予定\n- 基幹システム更改 / 移行手順書のドラフト\n- 基幹システム更改 / 結合テスト計画の作成(予定 11/20〜11/27)\n"),
            "{text}"
        );
        assert!(
            text.contains(concat!(
                "## 今週の作業\n",
                "- 基幹システム更改 / **API設計書のレビュー対応**(待ち / 日限 11/20)3.5h\n",
                "  - 11/18\n",
                "    レビュー指摘の一覧を整理し、対応方針を決めた。\n",
                "\n",
                "    - 仕様の見直しが必要な指摘は3件\n",
                "    - 文言の修正だけで済む指摘は9件\n",
                "  - 11/19\n",
                "    - 指摘12件のうち9件を反映\n",
            )),
            "{text}"
        );
        assert!(
            text.contains(concat!(
                "- 採用サイト改修 / **募集要項ページの原稿確認**(待ち / 日限 11/27)\n",
                "  - 11/16\n",
                "    - 原稿の初稿を人事部に送った\n",
            )),
            "{text}"
        );
        assert!(text.ends_with("\n\n## 連絡事項\n"), "{text}");
        // The daily report keeps its own template.
        let daily = generate(&pool, key("2026-11-19"), None, TODAY)
            .await
            .unwrap();
        assert!(daily.starts_with("# 日報 2026-11-19(木)\n\n## 本日の作業\n"));
        assert!(daily.ends_with("\n\n## 所感・連絡事項\n"), "{daily}");
    }

    #[tokio::test]
    async fn the_weekly_note_survives_heading_changes() {
        let pool = week_seeded().await;
        let k = weekly("2026-11-19");
        let with_note = |heading: &str| {
            let mut t = WeeklyReportTemplate::default();
            edit_weekly(&mut t, WeeklySectionKind::Note, |s| {
                s.heading = heading.into()
            });
            t
        };
        let first = generate(&pool, k, None, TODAY).await.unwrap();
        save(
            &pool,
            k,
            &format!("{first}- 所感を書いた。\n"),
            ts(2026, 11, 19, 10, 0),
        )
        .await
        .unwrap();

        // 所感・連絡事項 → 連絡: found by the default heading.
        use_weekly_template(&pool, with_note("連絡")).await;
        let saved = get(&pool, k).await.unwrap().unwrap();
        let second = generate(&pool, k, Some(&saved.body_md), TODAY)
            .await
            .unwrap();
        assert!(
            second.ends_with("\n\n## 連絡\n- 所感を書いた。\n"),
            "{second}"
        );
        save(&pool, k, &second, ts(2026, 11, 19, 11, 0))
            .await
            .unwrap();

        // 連絡 → メモ: 「## 連絡」 is not a section's heading.
        use_weekly_template(&pool, with_note("メモ")).await;
        let saved = get(&pool, k).await.unwrap().unwrap();
        let third = generate(&pool, k, Some(&saved.body_md), TODAY)
            .await
            .unwrap();
        assert!(
            third.ends_with("\n\n## メモ\n- 所感を書いた。\n"),
            "{third}"
        );
        let saved = save(&pool, k, &saved.body_md, ts(2026, 11, 19, 12, 0))
            .await
            .unwrap();
        assert_eq!(saved.note_md, "- 所感を書いた。");
        assert_eq!(generate(&pool, k, None, TODAY).await.unwrap(), third);
        // The daily template's note heading is not the weekly one's.
        assert_eq!(
            crate::commands::settings::get(&pool)
                .await
                .unwrap()
                .report_template
                .note_heading(),
            "所感・連絡事項"
        );
    }
}
