//! higiri-cli on the sample data (the mock's day, 2026-11-19), in a work
//! folder: the read sub-commands against the screens' commands.

use std::path::PathBuf;

use chrono::NaiveDate;
use clap::Parser;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use super::args::Cli;
use super::error::{CliError, CliErrorKind};
use super::{IDENTIFIER, Reply, data_folder, run};
use crate::commands::projects::{self, ProjectInput};
use crate::commands::reports::{self, ReportKey};
use crate::commands::settings::{self, SettingsInput};
use crate::commands::todos::{self, TodoFields, TodoQuery};
use crate::commands::{dashboard, dependencies, search};
use crate::domain::listing::{DueFilter, TodoSort};
use crate::domain::model::{Priority, Status, Todo};
use crate::domain::report::ReportKind;
use crate::domain::settings::{DayCount, Settings};
use crate::domain::testing::{d, ts};
use crate::repo::db;

fn today() -> NaiveDate {
    d("2026-11-19")
}

/// The sample data in a work folder, as the app leaves it.
struct Sample {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    pool: SqlitePool,
}

async fn sample() -> Sample {
    let tmp = tempfile::tempdir().unwrap();
    let pool = db::open(&tmp.path().join(db::DB_FILE_NAME)).await.unwrap();
    crate::sample::seed_if_empty(&pool, today()).await.unwrap();
    Sample {
        dir: tmp.path().to_owned(),
        _tmp: tmp,
        pool,
    }
}

impl Sample {
    /// `higiri-cli <args>` as of the mock's day: its output parsed.
    async fn cli(&self, args: &[&str]) -> Result<Value, CliError> {
        let cli =
            Cli::try_parse_from(std::iter::once("higiri-cli").chain(args.iter().copied())).unwrap();
        let out = run(&cli.command, &self.dir, today()).await?;
        assert!(!out.contains('\n'), "one line: {out}");
        Ok(serde_json::from_str(&out).unwrap())
    }

    async fn ok(&self, args: &[&str]) -> Value {
        self.cli(args).await.unwrap()
    }

    async fn err(&self, args: &[&str]) -> CliError {
        self.cli(args).await.unwrap_err()
    }

    /// Every TODO (active projects).
    async fn todos(&self) -> Vec<Todo> {
        todos::list(&self.pool, &TodoQuery::default(), today())
            .await
            .unwrap()
            .rows
            .into_iter()
            .map(|r| r.item.todo)
            .collect()
    }

    async fn id(&self, title: &str) -> i64 {
        self.todos()
            .await
            .into_iter()
            .find(|t| t.title == title)
            .unwrap_or_else(|| panic!("{title}"))
            .id
    }

    async fn project(&self, name: &str) -> i64 {
        projects::list(&self.pool, true, today())
            .await
            .unwrap()
            .into_iter()
            .find(|s| s.project.name == name)
            .unwrap()
            .project
            .id
    }

    async fn hide(&self, ids: Vec<i64>) {
        let current = settings::get(&self.pool).await.unwrap();
        settings::update(
            &self.pool,
            SettingsInput {
                cli_hidden_projects: ids,
                ..SettingsInput::from(&current)
            },
        )
        .await
        .unwrap();
    }

    async fn business_days(&self) {
        settings::update(
            &self.pool,
            SettingsInput {
                day_count: DayCount::Business,
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
    }
}

fn ids(todos: &Value) -> Vec<i64> {
    todos
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_i64().unwrap())
        .collect()
}

fn titles(todos: &Value) -> Vec<String> {
    todos
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

// ---- the data folder, the database and the reply ---------------------------------

#[test]
fn output_goes_to_stdout_with_0_and_errors_to_stderr_with_1() {
    let out = Reply::of(Ok(r#"{"hits":[],"total":0}"#.to_owned()));
    assert_eq!(out, Reply::Out(r#"{"hits":[],"total":0}"#.to_owned()));
    assert_eq!(out.code(), 0);
    let err = Reply::of(Err(CliError::hidden_todo()));
    assert_eq!(err.code(), 1);
    let Reply::Error(line) = err else {
        panic!("standard error");
    };
    let parsed: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(parsed["error"]["kind"], "hidden");
    assert_eq!(parsed["error"]["message"], CliError::hidden_todo().message);
}

#[test]
fn the_data_folder_is_the_apps() {
    let conf: Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
    assert_eq!(conf["identifier"], IDENTIFIER);
    // `--version` is the app's version.
    assert_eq!(conf["version"], env!("CARGO_PKG_VERSION"));

    let app = data_folder(false).unwrap();
    assert!(app.ends_with(IDENTIFIER), "{}", app.display());
    assert_eq!(data_folder(true).unwrap(), app.join("dev"));
    #[cfg(target_os = "macos")]
    assert!(
        app.ends_with("Library/Application Support/com.redjet.higiri"),
        "{}",
        app.display()
    );
}

#[tokio::test]
async fn no_database_is_refused_and_nothing_is_made() {
    let tmp = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from(["higiri-cli", "today"]).unwrap();
    let e = run(&cli.command, tmp.path(), today()).await.unwrap_err();
    assert_eq!(e.kind, CliErrorKind::NotFound);
    assert_eq!(
        e.message,
        "Higiri のデータが見つかりません。先に Higiri を一度起動してください。"
    );
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    // A folder that is not there is not made either.
    let missing = tmp.path().join("missing");
    assert!(run(&cli.command, &missing, today()).await.is_err());
    assert!(!missing.exists());
}

#[tokio::test]
async fn a_database_of_another_version_is_refused() {
    let s = sample().await;
    db::record_later_migration(&s.pool).await;
    let e = s.err(&["projects"]).await;
    assert_eq!(e.kind, CliErrorKind::Validation);
    assert!(
        e.message
            .starts_with("higiri-cli が Higiri より古い版です。")
    );

    let tmp = tempfile::tempdir().unwrap();
    db::migrated_to(&tmp.path().join(db::DB_FILE_NAME), 1).await;
    let cli = Cli::try_parse_from(["higiri-cli", "projects"]).unwrap();
    let e = run(&cli.command, tmp.path(), today()).await.unwrap_err();
    assert_eq!(e.kind, CliErrorKind::Validation);
    assert!(e.message.starts_with("Higiri のデータが古い版のままです。"));
}

// ---- today -------------------------------------------------------------------

/// (title, kind, days_until_due, text) of `today`'s 日限注意.
fn attention(out: &Value) -> Vec<(String, String, Option<i64>, String)> {
    out["attention"]["todos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["title"].as_str().unwrap().to_owned(),
                t["urgency"]["kind"].as_str().unwrap().to_owned(),
                t["days_until_due"].as_i64(),
                t["urgency"]["text"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[tokio::test]
async fn today_is_the_dashboard_with_the_daily_reports_words() {
    let s = sample().await;
    for business in [false, true] {
        if business {
            s.business_days().await;
        }
        let out = s.ok(&["today"]).await;
        let dash = dashboard::get(&s.pool, today()).await.unwrap();
        assert_eq!(out["today"], "2026-11-19");
        assert_eq!(
            out["day_count"],
            if business { "business" } else { "calendar" }
        );
        assert_eq!(
            out["day_count_label"],
            if business { "営業日" } else { "暦日" }
        );
        assert_eq!(out["soon_days"], 3);

        // The dashboard's items, categories, day counts and order.
        assert_eq!(out["attention"]["total"], dash.attention_total);
        assert_eq!(
            out["attention"]["counts"],
            serde_json::to_value(dash.counts).unwrap()
        );
        let expected: Vec<(String, String, Option<i64>)> = dash
            .attention
            .iter()
            .map(|i| {
                (
                    i.todo.title.clone(),
                    serde_json::to_value(i.assessment.urgency)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned(),
                    i.assessment.days_until_due,
                )
            })
            .collect();
        let got = attention(&out);
        assert_eq!(
            got.iter()
                .map(|(t, k, n, _)| (t.clone(), k.clone(), *n))
                .collect::<Vec<_>>(),
            expected
        );
        let unit = if business { "営業日" } else { "日" };
        assert_eq!(
            got.iter().map(|g| g.3.as_str()).collect::<Vec<_>>(),
            [
                format!("日限 11/17、2{unit}超過"),
                "日限 11/19".to_owned(),
                format!("日限 11/20、あと1{unit}、待ち"),
                "確認日 11/19、待ち3日目".to_owned(),
                format!("予定開始 11/17、2{unit}経過"),
            ]
        );
        for t in out["attention"]["todos"].as_array().unwrap() {
            assert_eq!(t["business_days"], business);
        }

        // 超過・本日・間近 read exactly as the daily report's lines.
        let report = reports::generate(
            &s.pool,
            ReportKey {
                kind: ReportKind::Daily,
                date: today(),
            },
            None,
            today(),
        )
        .await
        .unwrap();
        for (title, kind, _, text) in &got {
            if ["overdue", "due_today", "due_soon"].contains(&kind.as_str()) {
                assert!(
                    report.contains(&format!(" / {title}({text})\n")),
                    "{title}({text})\n{report}"
                );
            }
        }

        assert_eq!(
            titles(&out["today_work"]),
            dash.today_work
                .iter()
                .map(|i| i.todo.title.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            titles(&out["waiting"]),
            dash.waiting
                .iter()
                .map(|i| i.todo.title.clone())
                .collect::<Vec<_>>()
        );
        let waiting = &out["waiting"][0];
        assert_eq!(waiting["title"], "募集要項ページの原稿確認");
        assert_eq!(
            waiting["waiting"],
            json!({"on": "人事部の原稿確認", "since": "2026-11-17", "follow_up_date": "2026-11-19", "day": 3})
        );
        assert_eq!(
            out["projects"],
            json!([
                {"id": 1, "name": "基幹システム更改", "progress_done": 1, "progress_total": 5, "attention_count": 3},
                {"id": 2, "name": "社内ツール整備", "progress_done": 2, "progress_total": 8, "attention_count": 1},
                {"id": 3, "name": "採用サイト改修", "progress_done": 0, "progress_total": 2, "attention_count": 1},
            ])
        );
        assert_eq!(out["backup_mirror_alert"], Value::Null);
    }
}

#[tokio::test]
async fn the_todo_form_has_the_screens_judgement() {
    let s = sample().await;
    let out = s.ok(&["today"]).await;
    let api = out["attention"]["todos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["title"] == "API設計書のレビュー対応")
        .unwrap()
        .clone();
    let id = s.id("API設計書のレビュー対応").await;
    let item = todos::get(&s.pool, id, today()).await.unwrap();
    assert_eq!(
        api,
        json!({
            "id": id,
            "title": "API設計書のレビュー対応",
            "parent": null,
            "project": {"id": 1, "name": "基幹システム更改"},
            "status": "waiting",
            "status_label": "待ち",
            "priority": "high",
            "priority_label": "高",
            "due_date": "2026-11-20",
            "planned_start": "2026-11-16",
            "planned_end": "2026-11-19",
            "actual_start": "2026-11-17",
            "actual_end": null,
            "urgency": {"kind": "due_soon", "label": "間近", "text": "日限 11/20、あと1日、待ち"},
            "days_until_due": 1,
            "business_days": false,
            "waiting": {"on": "認証担当の回答", "since": "2026-11-19", "follow_up_date": "2026-11-20", "day": 1},
            "close_reason": null,
            "warnings": [],
            "blocked_by": [],
            "children": null,
            "repeat": null,
            "updated_at": crate::domain::date::format_timestamp(item.todo.updated_at),
        })
    );
    // The keys come in the form's order (not sorted).
    let text = run(
        &Cli::try_parse_from(["higiri-cli", "show", &id.to_string()])
            .unwrap()
            .command,
        &s.dir,
        today(),
    )
    .await
    .unwrap();
    assert!(text.starts_with(&format!(
        r#"{{"id":{id},"title":"API設計書のレビュー対応","parent":null,"#
    )));

    // A child, a parent, a TODO that waits, a repeating one, warnings.
    let list = s.ok(&["list", "--limit", "100"]).await;
    let by_title = |title: &str| {
        list["todos"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["title"] == title)
            .unwrap()
            .clone()
    };
    let parent = s.id("開発環境の標準化").await;
    assert_eq!(
        by_title("セットアップ手順書の作成")["parent"],
        json!({"id": parent, "title": "開発環境の標準化"})
    );
    assert_eq!(
        by_title("開発環境の標準化")["children"],
        json!({"total": 3, "done": 1, "open": 2})
    );
    let first = s.id("移行手順書のドラフト").await;
    let second = s.id("性能試験の環境準備").await;
    assert_eq!(
        by_title("結合テスト計画の作成")["blocked_by"],
        json!([
            {"id": first, "title": "移行手順書のドラフト"},
            {"id": second, "title": "性能試験の環境準備"},
        ])
    );
    let late = by_title("性能試験の環境準備");
    assert_eq!(
        late["warnings"],
        json!([{"kind": "starts_before_dependency", "label": "先行TODOより前に始まる予定"}])
    );
    assert_eq!(late["urgency"]["kind"], "late_start");
    assert_eq!(
        by_title("移行手順書のドラフト")["warnings"],
        json!([{"kind": "plan_overrun", "label": "予定超過"}])
    );
    assert_eq!(
        by_title("社内ツールの更新確認")["repeat"],
        json!({"rule": {"kind": "weekly", "weekdays": ["thu"]}, "until_date": null, "next_due": "2026-12-03"})
    );
    let cancelled = by_title("旧帳票の移行検証");
    assert_eq!(cancelled["close_reason"], "旧帳票の廃止が決まったため");
    assert_eq!(cancelled["urgency"], Value::Null);
    assert_eq!(cancelled["days_until_due"], Value::Null);
    // The memo and the work logs are not in the form.
    for t in list["todos"].as_array().unwrap() {
        assert!(t.get("memo_md").is_none());
        assert!(t.get("work_logs").is_none());
    }
}

// ---- list --------------------------------------------------------------------

#[tokio::test]
async fn list_filters_and_orders_as_the_screen() {
    let s = sample().await;
    let social = s.project("社内ツール整備").await;
    let queries = [
        (vec!["list"], TodoQuery::default()),
        (
            vec!["list", "--status", "todo,doing", "--sort", "due_date"],
            TodoQuery {
                statuses: Some(vec![Status::Todo, Status::Doing]),
                sort: Some(TodoSort::DueDate),
                ..TodoQuery::default()
            },
        ),
        (
            vec!["list", "--due", "attention", "--sort", "priority"],
            TodoQuery {
                due: Some(DueFilter::Attention),
                sort: Some(TodoSort::Priority),
                ..TodoQuery::default()
            },
        ),
        (
            vec!["list", "--due", "within_week", "--priority", "medium"],
            TodoQuery {
                due: Some(DueFilter::WithinWeek),
                priority: Some(Priority::Medium),
                ..TodoQuery::default()
            },
        ),
        (
            vec!["list", "--project", "社内ツール整備", "--status", "done"],
            TodoQuery {
                project_id: Some(social),
                statuses: Some(vec![Status::Done]),
                ..TodoQuery::default()
            },
        ),
    ];
    for (args, query) in queries {
        let out = s.ok(&args).await;
        let screen = todos::list(&s.pool, &query, today()).await.unwrap();
        let expected: Vec<i64> = screen
            .rows
            .iter()
            .filter(|r| !r.bridge)
            .map(|r| r.item.todo.id)
            .collect();
        assert_eq!(ids(&out["todos"]), expected, "{args:?}");
        assert_eq!(out["total"], expected.len() as i64, "{args:?}");
    }

    // 完了 in 社内ツール整備: the child done without its parent comes alone,
    // with its parent named (the screen's dimmed row is not listed).
    let done = s
        .ok(&["list", "--project", &social.to_string(), "--status", "done"])
        .await;
    assert_eq!(
        titles(&done["todos"]),
        ["デプロイ手順書の更新", "必要なツールの洗い出し"]
    );
    assert_eq!(done["todos"][1]["parent"]["title"], "開発環境の標準化");

    // --limit cuts the list; total counts them all.
    let all = s.ok(&["list"]).await;
    assert_eq!(all["total"], 16);
    let three = s.ok(&["list", "--limit", "3"]).await;
    assert_eq!(three["total"], 16);
    assert_eq!(ids(&three["todos"]), ids(&all["todos"])[..3]);
}

#[tokio::test]
async fn archived_projects_are_listed_only_when_named() {
    let s = sample().await;
    let hiring = s.project("採用サイト改修").await;
    projects::archive(&s.pool, hiring, true, ts(2026, 11, 19, 0, 0))
        .await
        .unwrap();
    let all = s.ok(&["list"]).await;
    assert_eq!(all["total"], 14);
    assert!(
        all["todos"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["project"]["id"] != hiring)
    );
    for named in ["採用サイト改修".to_owned(), hiring.to_string()] {
        let out = s.ok(&["list", "--project", &named]).await;
        assert_eq!(
            titles(&out["todos"]),
            ["募集要項ページの原稿確認", "応募フォームの項目見直し"]
        );
    }
    // projects: --archived adds it.
    let shown = s.ok(&["projects"]).await;
    assert_eq!(shown["projects"].as_array().unwrap().len(), 2);
    let every = s.ok(&["projects", "--archived"]).await;
    assert_eq!(
        every["projects"][2],
        json!({"id": hiring, "name": "採用サイト改修", "description": "", "archived": true,
               "progress_done": 0, "progress_total": 2, "attention_count": 1})
    );
}

#[tokio::test]
async fn projects_are_named_by_id_or_exact_name() {
    let s = sample().await;
    let e = s.err(&["list", "--project", "社内"]).await;
    assert_eq!(e.kind, CliErrorKind::NotFound);
    assert_eq!(
        e.message,
        "プロジェクト「社内」が見つかりません。higiri-cli projects で名前かidを確かめてください。"
    );
    let e = s.err(&["list", "--project", "99"]).await;
    assert_eq!(e.kind, CliErrorKind::NotFound);

    // A project named like an id, and two projects of the same name.
    let numbered = projects::create(
        &s.pool,
        ProjectInput {
            name: "2026".into(),
            color: None,
            description: String::new(),
        },
        ts(2026, 11, 19, 0, 0),
    )
    .await
    .unwrap();
    let out = s.ok(&["list", "--project", "2026"]).await;
    assert_eq!(out["total"], 0);
    assert!(numbered.id != 2026);
    projects::create(
        &s.pool,
        ProjectInput {
            name: "社内ツール整備".into(),
            color: None,
            description: String::new(),
        },
        ts(2026, 11, 19, 0, 0),
    )
    .await
    .unwrap();
    let e = s.err(&["list", "--project", "社内ツール整備"]).await;
    assert_eq!(e.kind, CliErrorKind::Validation);
    assert_eq!(
        e.message,
        "「社内ツール整備」という名前のプロジェクトが2つ以上あります。higiri-cli projects でidを確かめて、idで指してください。"
    );
    // By id it is one of them.
    let out = s.ok(&["list", "--project", "2"]).await;
    assert_eq!(out["total"], 8);
}

// ---- show --------------------------------------------------------------------

#[tokio::test]
async fn show_adds_the_memo_the_newest_logs_and_the_todos_around() {
    let s = sample().await;
    let api = s.id("API設計書のレビュー対応").await;
    let item = todos::get(&s.pool, api, today()).await.unwrap();
    todos::update(
        &s.pool,
        api,
        TodoFields {
            title: item.todo.title.clone(),
            priority: item.todo.priority,
            due_date: item.todo.due_date,
            planned_start: item.todo.planned_start,
            planned_end: item.todo.planned_end,
            actual_start: item.todo.actual_start,
            actual_end: item.todo.actual_end,
            waiting_on: item.todo.waiting_on.clone(),
            follow_up_date: item.todo.follow_up_date,
            close_reason: item.todo.close_reason.clone(),
            memo_md: "## 方針\n- 認証まわりは別途相談".into(),
        },
        today(),
        ts(2026, 11, 19, 2, 0),
    )
    .await
    .unwrap();

    let out = s.ok(&["show", &api.to_string()]).await;
    assert_eq!(out["id"], api);
    assert_eq!(out["urgency"]["text"], "日限 11/20、あと1日、待ち");
    assert_eq!(out["memo_md"], "## 方針\n- 認証まわりは別途相談");
    assert_eq!(out["work_logs_total"], 2);
    assert_eq!(
        out["work_logs"],
        json!([
            {"log_date": "2026-11-19", "minutes": 90, "body_md": "- 指摘12件のうち9件を反映\n- 残り3件は認証まわり。認証担当に質問を送り、回答待ち\n- [ ] 指摘10:トークン更新の手順を追記"},
            {"log_date": "2026-11-18", "minutes": 120, "body_md": "レビュー指摘の一覧を整理し、対応方針を決めた。\n\n- 仕様の見直しが必要な指摘は3件\n- 文言の修正だけで済む指摘は9件"},
        ])
    );
    assert_eq!(out["child_todos"], json!([]));
    assert_eq!(out["depends_on"], json!([]));
    let later = s.id("性能試験の環境準備").await;
    assert_eq!(
        out["dependents"],
        json!([{"id": later, "title": "性能試験の環境準備", "status": "todo", "status_label": "未着手"}])
    );
    let one = s.ok(&["show", &api.to_string(), "--logs", "1"]).await;
    assert_eq!(one["work_logs"].as_array().unwrap().len(), 1);
    assert_eq!(one["work_logs"][0]["log_date"], "2026-11-19");
    assert_eq!(one["work_logs_total"], 2);
    let none = s.ok(&["show", &api.to_string(), "--logs", "0"]).await;
    assert_eq!(none["work_logs"], json!([]));

    // A parent's children, in the list's order.
    let parent = s.id("開発環境の標準化").await;
    let out = s.ok(&["show", &parent.to_string()]).await;
    assert_eq!(out["children"], json!({"total": 3, "done": 1, "open": 2}));
    let child_todos = out["child_todos"].as_array().unwrap();
    assert_eq!(
        child_todos
            .iter()
            .map(|c| (
                c["title"].as_str().unwrap(),
                c["status"].as_str().unwrap(),
                c["status_label"].as_str().unwrap(),
                c["due_date"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            ("必要なツールの洗い出し", "done", "完了", "2026-11-18"),
            ("セットアップ手順書の作成", "todo", "未着手", "2026-12-04"),
            ("新メンバーでの試行", "todo", "未着手", "2026-12-10"),
        ]
    );

    // 先行TODO in the order they were added, finished ones too.
    let plan = s.id("結合テスト計画の作成").await;
    let out = s.ok(&["show", &plan.to_string()]).await;
    assert_eq!(
        out["depends_on"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["移行手順書のドラフト", "性能試験の環境準備"]
    );
    assert_eq!(out["dependents"], json!([]));

    let e = s.err(&["show", "99999"]).await;
    assert_eq!(e.kind, CliErrorKind::NotFound);
    assert_eq!(
        e.message,
        "TODO(id 99999)が見つかりません。削除された可能性があります。higiri-cli list か search でidを確かめてください。"
    );
}

// ---- search ------------------------------------------------------------------

#[tokio::test]
async fn search_names_where_it_found_the_words_without_the_text() {
    let s = sample().await;
    // 「認証」: 待っている相手 and the work logs of API設計書のレビュー対応.
    let out = s.ok(&["search", "認証"]).await;
    assert_eq!(out["total"], 1);
    assert_eq!(out["hits"][0]["todo"]["title"], "API設計書のレビュー対応");
    assert_eq!(out["hits"][0]["matched"], json!(["waiting_on", "work_log"]));
    let text = serde_json::to_string(&out).unwrap();
    assert!(!text.contains("トークン更新"), "{text}");
    assert!(!text.contains("回答待ち"), "{text}");

    // Two words, in the screen's order and count.
    let out = s.ok(&["search", "手順", "書"]).await;
    let screen = search::search(&s.pool, "手順 書", today()).await.unwrap();
    assert_eq!(out["total"], screen.total);
    assert_eq!(
        out["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["todo"]["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        screen
            .hits
            .iter()
            .map(|h| h.item.todo.id)
            .collect::<Vec<_>>()
    );
    let first = &out["hits"][0];
    assert_eq!(first["matched"][0], "title");
    for h in out["hits"].as_array().unwrap() {
        assert!(h.get("segments").is_none());
        assert!(h["todo"].get("memo_md").is_none());
    }

    // A memo hit names the memo only.
    let id = s.id("ライセンス棚卸し").await;
    let item = todos::get(&s.pool, id, today()).await.unwrap();
    todos::update(
        &s.pool,
        id,
        TodoFields {
            title: item.todo.title.clone(),
            priority: item.todo.priority,
            due_date: item.todo.due_date,
            planned_start: item.todo.planned_start,
            planned_end: item.todo.planned_end,
            actual_start: item.todo.actual_start,
            actual_end: item.todo.actual_end,
            waiting_on: None,
            follow_up_date: None,
            close_reason: None,
            memo_md: "契約更新は経理の山田さんに確認".into(),
        },
        today(),
        ts(2026, 11, 19, 2, 0),
    )
    .await
    .unwrap();
    let out = s.ok(&["search", "経理"]).await;
    assert_eq!(out["hits"][0]["matched"], json!(["memo"]));
    assert!(!serde_json::to_string(&out).unwrap().contains("山田"));

    let none = s.ok(&["search", "該当なし"]).await;
    assert_eq!(none, json!({"hits": [], "total": 0}));
}

// ---- projects the CLI does not show ---------------------------------------------

#[tokio::test]
async fn hidden_projects_are_nowhere() {
    let s = sample().await;
    let core = s.project("基幹システム更改").await;
    let social = s.project("社内ツール整備").await;
    // 結合テスト計画の作成 also waits for ライセンス棚卸し (社内ツール整備), and
    // 監視ダッシュボードの整理 (社内ツール整備) waits for 移行手順書のドラフト.
    let plan = s.id("結合テスト計画の作成").await;
    let licences = s.id("ライセンス棚卸し").await;
    let migration = s.id("移行手順書のドラフト").await;
    let dashboard_todo = s.id("監視ダッシュボードの整理").await;
    dependencies::add(&s.pool, plan, licences, today(), ts(2026, 11, 19, 2, 0))
        .await
        .unwrap();
    dependencies::add(
        &s.pool,
        dashboard_todo,
        migration,
        today(),
        ts(2026, 11, 19, 2, 0),
    )
    .await
    .unwrap();
    let before = s.ok(&["today"]).await;
    let all_search = search::search(&s.pool, "手順", today()).await.unwrap();
    s.hide(vec![social]).await;

    // today: counted without it.
    let out = s.ok(&["today"]).await;
    let shown: Vec<&Value> = before["attention"]["todos"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["project"]["id"] != social)
        .collect();
    assert_eq!(out["attention"]["total"], shown.len() as i64);
    assert_eq!(out["attention"]["total"], 4);
    assert_eq!(out["attention"]["counts"]["overdue"], 0);
    assert_eq!(
        ids(&out["attention"]["todos"]),
        shown
            .iter()
            .map(|t| t["id"].as_i64().unwrap())
            .collect::<Vec<_>>()
    );
    for part in ["today_work", "waiting"] {
        assert!(
            out[part]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["project"]["id"] != social),
            "{part}"
        );
    }
    assert_eq!(
        out["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["基幹システム更改", "採用サイト改修"]
    );
    let text = serde_json::to_string(&out).unwrap();
    assert!(!text.contains("ライセンス棚卸し"), "{text}");
    assert!(!text.contains("社内ツール整備"), "{text}");

    // list: left out, and refused when named.
    let out = s.ok(&["list", "--limit", "100"]).await;
    assert_eq!(out["total"], 8);
    assert!(
        out["todos"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["project"]["id"] != social)
    );
    for named in ["社内ツール整備".to_owned(), social.to_string()] {
        let e = s.err(&["list", "--project", &named]).await;
        assert_eq!(e, CliError::hidden_project());
        assert_eq!(e.kind, CliErrorKind::Hidden);
        assert_eq!(
            e.message,
            "このプロジェクトは、CLIに出さない設定です。CLIで扱うときは、Higiri の設定画面の「CLIに出すプロジェクト」で変えてください。"
        );
    }
    // The links to it: { hidden: true }.
    let plan_out = out["todos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == plan)
        .unwrap();
    let second = s.id("性能試験の環境準備").await;
    assert_eq!(
        plan_out["blocked_by"],
        json!([
            {"id": migration, "title": "移行手順書のドラフト"},
            {"id": second, "title": "性能試験の環境準備"},
            {"hidden": true},
        ])
    );

    // show: refused for its TODOs; its TODOs around another one are hidden.
    let e = s.err(&["show", &licences.to_string()]).await;
    assert_eq!(e.kind, CliErrorKind::Hidden);
    assert_eq!(
        e.message,
        "このTODOは、CLIに出さない設定のプロジェクトにあります。CLIで扱うときは、Higiri の設定画面の「CLIに出すプロジェクト」で変えてください。"
    );
    let out = s.ok(&["show", &plan.to_string()]).await;
    assert_eq!(out["depends_on"][2], json!({"hidden": true}));
    assert_eq!(out["blocked_by"][2], json!({"hidden": true}));
    let out = s.ok(&["show", &migration.to_string()]).await;
    assert_eq!(
        out["dependents"].as_array().unwrap().last().unwrap(),
        &json!({"hidden": true})
    );
    assert!(
        !serde_json::to_string(&out)
            .unwrap()
            .contains("監視ダッシュボード")
    );

    // search: left out before counting and cutting.
    let out = s.ok(&["search", "手順"]).await;
    let shown_hits: Vec<i64> = all_search
        .hits
        .iter()
        .filter(|h| h.item.todo.project_id != social)
        .map(|h| h.item.todo.id)
        .collect();
    assert!(shown_hits.len() < all_search.hits.len());
    assert_eq!(out["total"], shown_hits.len() as i64);
    assert_eq!(
        out["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["todo"]["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        shown_hits
    );

    // projects: counted, not listed.
    let out = s.ok(&["projects"]).await;
    assert_eq!(out["hidden_projects"], 1);
    assert_eq!(
        out["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        [core, s.project("採用サイト改修").await]
    );

    // Shown again.
    s.hide(vec![]).await;
    assert_eq!(s.ok(&["projects"]).await["hidden_projects"], 0);
    assert_eq!(s.ok(&["today"]).await["attention"]["total"], 5);
}
