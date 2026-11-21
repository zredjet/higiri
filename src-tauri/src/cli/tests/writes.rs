//! The write sub-commands on the sample data: what they change against what
//! the screens' commands change, what they refuse, and the backup taken
//! before them.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;
use crate::commands::repeats::{self, RepeatInput};
use crate::commands::work_logs::{self, TimelineEntry, WorkLogInput};
use crate::domain::repeat::{DayOfWeek, RepeatRule};
use crate::domain::status::StatusChange;
use crate::domain::views::TodoItem;

impl Sample {
    /// `higiri-cli <args>` with `input` as standard input.
    async fn cli_reading(&self, args: &[&str], input: &str) -> Result<Value, CliError> {
        let mut cli =
            Cli::try_parse_from(std::iter::once("higiri-cli").chain(args.iter().copied())).unwrap();
        super::super::write::read_body(&mut cli.command, &mut input.as_bytes())?;
        let out = run(&cli.command, &self.dir, today(), now()).await?;
        Ok(serde_json::from_str(&out).unwrap())
    }

    async fn item(&self, id: i64) -> TodoItem {
        todos::get(&self.pool, id, today()).await.unwrap()
    }

    async fn timeline(&self, id: i64) -> Vec<TimelineEntry> {
        work_logs::timeline(&self.pool, id).await.unwrap()
    }

    /// Every TODO of every project (archived ones too), as items.
    async fn everything(&self) -> Vec<TodoItem> {
        let mut out = Vec::new();
        for p in projects::list(&self.pool, true, today()).await.unwrap() {
            let query = TodoQuery {
                project_id: Some(p.project.id),
                ..TodoQuery::default()
            };
            out.extend(
                todos::list(&self.pool, &query, today())
                    .await
                    .unwrap()
                    .rows
                    .into_iter()
                    .map(|r| r.item),
            );
        }
        out
    }

    /// The TODOs and every timeline: nothing may change when refused.
    async fn state(&self) -> (Vec<TodoItem>, Vec<Vec<TimelineEntry>>) {
        let all = self.everything().await;
        let mut lines = Vec::new();
        for i in &all {
            lines.push(self.timeline(i.todo.id).await);
        }
        (all, lines)
    }

    fn backups(&self) -> PathBuf {
        crate::backup::dir(&self.dir)
    }

    fn backup_files(&self) -> Vec<String> {
        match std::fs::read_dir(self.backups()) {
            Err(_) => Vec::new(),
            Ok(entries) => {
                let mut names: Vec<String> = entries
                    .map(|e| e.unwrap().file_name().into_string().unwrap())
                    .collect();
                names.sort();
                names
            }
        }
    }

    async fn set_memo(&self, id: i64, memo: &str) {
        let t = self.item(id).await.todo;
        todos::update(
            &self.pool,
            id,
            TodoFields {
                memo_md: memo.into(),
                ..fields_of(&t)
            },
            today(),
            now(),
        )
        .await
        .unwrap();
    }
}

fn fields_of(t: &Todo) -> TodoFields {
    TodoFields {
        title: t.title.clone(),
        priority: t.priority,
        due_date: t.due_date,
        planned_start: t.planned_start,
        planned_end: t.planned_end,
        actual_start: t.actual_start,
        actual_end: t.actual_end,
        waiting_on: t.waiting_on.clone(),
        follow_up_date: t.follow_up_date,
        close_reason: t.close_reason.clone(),
        memo_md: t.memo_md.clone(),
    }
}

/// A TODO without the time it was last written.
fn unstamped(mut t: Todo) -> Todo {
    t.updated_at = ts(2000, 1, 1, 0, 0);
    t
}

fn todo_count(state: &(Vec<TodoItem>, Vec<Vec<TimelineEntry>>)) -> usize {
    state.0.len()
}

// ---- add ---------------------------------------------------------------------------

#[tokio::test]
async fn add_makes_a_todo_as_the_screen_does() {
    let s = sample().await;
    let screen = sample().await;
    let social = s.project("社内ツール整備").await;

    let out = s
        .ok(&[
            "add",
            "--project",
            "社内ツール整備",
            "--title",
            "  バックアップの手順を見直す  ",
            "--priority",
            "high",
            "--due",
            "2026-11-25",
            "--start",
            "2026-11-20",
            "--end",
            "2026-11-24",
        ])
        .await;
    let id = out["id"].as_i64().unwrap();
    assert_eq!(out["title"], "バックアップの手順を見直す");
    assert_eq!(
        out["project"],
        json!({"id": social, "name": "社内ツール整備"})
    );
    assert_eq!(out["status"], "todo");
    assert_eq!(out["status_label"], "未着手");
    assert_eq!(out["priority"], "high");
    assert_eq!(out["due_date"], "2026-11-25");
    assert_eq!(out["planned_start"], "2026-11-20");
    assert_eq!(out["planned_end"], "2026-11-24");
    assert_eq!(out["parent"], Value::Null);
    assert_eq!(out["updated_at"], "2026-11-19T01:30:00.000Z");

    // The screen's 「TODOを追加」 makes the same TODO and the same 「作成」.
    let made = todos::create(
        &screen.pool,
        todos::NewTodoInput {
            project_id: social,
            parent_id: None,
            template_id: None,
            title: "バックアップの手順を見直す".into(),
            priority: Some(Priority::High),
            due_date: Some(d("2026-11-25")),
            planned_start: Some(d("2026-11-20")),
            planned_end: Some(d("2026-11-24")),
            memo_md: None,
        },
        today(),
        now(),
    )
    .await
    .unwrap();
    assert_eq!(made.todo.id, id);
    assert_eq!(s.item(id).await, made);
    assert_eq!(s.timeline(id).await, screen.timeline(id).await);
    assert_eq!(s.item(id).await.todo.memo_md, "");

    // By id, with the priority left out: 中.
    let out = s
        .ok(&[
            "add",
            "--project",
            &social.to_string(),
            "--title",
            "-a で始まる",
        ])
        .await;
    assert_eq!(out["title"], "-a で始まる");
    assert_eq!(out["priority"], "medium");
    assert_eq!(out["due_date"], Value::Null);
}

#[tokio::test]
async fn add_makes_children_in_the_same_project_one_level_deep() {
    let s = sample().await;
    let parent = s.id("開発環境の標準化").await;
    let child = s.id("必要なツールの洗い出し").await;

    let out = s
        .ok(&[
            "add",
            "--project",
            "社内ツール整備",
            "--title",
            "手順書の読み合わせ",
            "--parent",
            &parent.to_string(),
        ])
        .await;
    assert_eq!(
        out["parent"],
        json!({"id": parent, "title": "開発環境の標準化"})
    );
    assert_eq!(
        s.item(parent).await.children.unwrap(),
        crate::domain::subtodo::ChildCounts {
            total: 4,
            done: 1,
            open: 3
        }
    );

    let before = s.state().await;
    // No grandchildren.
    let e = s
        .err(&[
            "add",
            "--project",
            "社内ツール整備",
            "--title",
            "孫",
            "--parent",
            &child.to_string(),
        ])
        .await;
    assert_eq!(
        e,
        CliError::validation(
            "子TODOは親にできません(親子は1段だけです)。親のないTODOを選んでください。"
        )
    );
    // The parent in the same project.
    let e = s
        .err(&[
            "add",
            "--project",
            "基幹システム更改",
            "--title",
            "別のプロジェクト",
            "--parent",
            &parent.to_string(),
        ])
        .await;
    assert_eq!(
        e,
        CliError::validation(
            "別のプロジェクトのTODOは親にできません。同じプロジェクトのTODOを選んでください。"
        )
    );
    let e = s
        .err(&[
            "add",
            "--project",
            "社内ツール整備",
            "--title",
            "親なし",
            "--parent",
            "9999",
        ])
        .await;
    assert_eq!(e, CliError::todo_not_found(9999));
    assert_eq!(s.state().await, before);
}

#[tokio::test]
async fn add_refuses_archived_unknown_and_twice_named_projects() {
    let s = sample().await;
    let recruiting = s.project("採用サイト改修").await;
    let before = s.state().await;

    let e = s
        .err(&["add", "--project", "社内ツール整備", "--title", "  "])
        .await;
    assert_eq!(e, CliError::validation("タイトルを入力してください。"));

    projects::archive(&s.pool, recruiting, true, now())
        .await
        .unwrap();
    let e = s
        .err(&["add", "--project", "採用サイト改修", "--title", "追加"])
        .await;
    assert_eq!(
        e,
        CliError::validation(
            "アーカイブしたプロジェクトにはTODOを追加できません。プロジェクトを元に戻してから追加してください。"
        )
    );

    let e = s
        .err(&["add", "--project", "ない", "--title", "追加"])
        .await;
    assert_eq!(e.kind, CliErrorKind::NotFound);

    projects::create(
        &s.pool,
        ProjectInput {
            name: "社内ツール整備".into(),
            color: None,
            description: String::new(),
        },
        now(),
    )
    .await
    .unwrap();
    let e = s
        .err(&["add", "--project", "社内ツール整備", "--title", "追加"])
        .await;
    assert_eq!(e.kind, CliErrorKind::Validation);
    assert!(e.message.contains("2つ以上あります"), "{}", e.message);

    // Nothing was added.
    assert_eq!(todo_count(&s.state().await), todo_count(&before));
}

// ---- update ------------------------------------------------------------------------

#[tokio::test]
async fn update_changes_only_the_named_fields_as_the_screen_does() {
    let s = sample().await;
    let screen = sample().await;
    let plan = s.id("結合テスト計画の作成").await;
    s.set_memo(plan, "# 方針\n\n- 範囲を決める").await;
    screen.set_memo(plan, "# 方針\n\n- 範囲を決める").await;
    let before = s.item(plan).await.todo;

    let out = s
        .ok(&["update", &plan.to_string(), "--due", "2026-12-01"])
        .await;
    assert_eq!(out["due_date"], "2026-12-01");
    assert!(out.get("memo_md").is_none());
    let after = s.item(plan).await.todo;
    assert_eq!(
        unstamped(after.clone()),
        unstamped(Todo {
            due_date: Some(d("2026-12-01")),
            ..before.clone()
        })
    );
    assert_eq!(after.memo_md, "# 方針\n\n- 範囲を決める");

    // The screen's update_todo with the same field records the same.
    let t = screen.item(plan).await.todo;
    todos::update(
        &screen.pool,
        plan,
        TodoFields {
            due_date: Some(d("2026-12-01")),
            ..fields_of(&t)
        },
        today(),
        now(),
    )
    .await
    .unwrap();
    assert_eq!(s.item(plan).await, screen.item(plan).await);
    assert_eq!(s.timeline(plan).await, screen.timeline(plan).await);

    // Several at once; the others stay.
    let out = s
        .ok(&[
            "update",
            &plan.to_string(),
            "--title",
            " 結合テスト計画 ",
            "--priority",
            "low",
            "--no-start",
            "--end",
            "2026-11-28",
        ])
        .await;
    assert_eq!(out["title"], "結合テスト計画");
    assert_eq!(out["priority"], "low");
    assert_eq!(out["planned_start"], Value::Null);
    assert_eq!(out["planned_end"], "2026-11-28");
    assert_eq!(out["due_date"], "2026-12-01");
    assert_eq!(s.item(plan).await.todo.memo_md, "# 方針\n\n- 範囲を決める");
    let out = s
        .ok(&["update", &plan.to_string(), "--no-due", "--no-end"])
        .await;
    assert_eq!(out["due_date"], Value::Null);
    assert_eq!(out["planned_end"], Value::Null);
    assert_eq!(out["title"], "結合テスト計画");

    // The screen's checks, in its words.
    let unchanged = s.state().await;
    let e = s
        .err(&[
            "update",
            &plan.to_string(),
            "--start",
            "2026-11-28",
            "--end",
            "2026-11-27",
        ])
        .await;
    assert_eq!(
        e,
        CliError::validation("予定開始日が予定終了日より後になっています。日付を直してください。")
    );
    let e = s.err(&["update", &plan.to_string(), "--title", " "]).await;
    assert_eq!(e, CliError::validation("タイトルを入力してください。"));
    let repeating = s.id("社内ツールの更新確認").await;
    let e = s.err(&["update", &repeating.to_string(), "--no-due"]).await;
    assert_eq!(
        e,
        CliError::validation(
            "繰り返しのあるTODOは、日限を空にできません。繰り返しを外してから日限を消してください。"
        )
    );
    let e = s.err(&["update", "9999", "--due", "2026-12-01"]).await;
    assert_eq!(e, CliError::todo_not_found(9999));
    assert_eq!(s.state().await, unchanged);
}

#[tokio::test]
async fn the_follow_up_date_is_for_waiting_todos() {
    let s = sample().await;
    let api = s.id("API設計書のレビュー対応").await;
    let plan = s.id("結合テスト計画の作成").await;

    let before = s.state().await;
    let e = s
        .err(&["update", &plan.to_string(), "--follow-up", "2026-11-25"])
        .await;
    assert_eq!(e.kind, CliErrorKind::Validation);
    assert!(
        e.message
            .starts_with("確認日(--follow-up)は、待ちのTODOだけに入れられます。"),
        "{}",
        e.message
    );
    assert_eq!(s.state().await, before);

    let lines = s.timeline(api).await.len();
    let out = s
        .ok(&["update", &api.to_string(), "--follow-up", "2026-11-25"])
        .await;
    assert_eq!(out["waiting"]["follow_up_date"], "2026-11-25");
    assert_eq!(out["waiting"]["on"], "認証担当の回答");
    let timeline = s.timeline(api).await;
    assert_eq!(timeline.len(), lines + 1);
    let newest = timeline
        .iter()
        .filter_map(|e| match e {
            TimelineEntry::Event(e) => Some(e),
            TimelineEntry::WorkLog(_) => None,
        })
        .max_by_key(|e| e.event.id)
        .unwrap();
    assert!(newest.text.contains("確認日"), "{}", newest.text);
}

// ---- status ------------------------------------------------------------------------

/// The same change through the CLI and the screen's change_status, on two
/// copies of the sample: the same TODOs and the same history after it.
async fn same_as_the_screen(title: &str, args: &[&str], change: StatusChange) -> Value {
    let s = sample().await;
    let screen = sample().await;
    let id = s.id(title).await;
    let mut full = vec!["status".to_owned(), id.to_string()];
    full.extend(args.iter().map(|a| (*a).to_owned()));
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    let out = s.ok(&full).await;
    todos::change_status_impl(&screen.pool, id, &change, today(), now())
        .await
        .unwrap();
    assert_eq!(s.state().await, screen.state().await, "{title}");
    out
}

#[tokio::test]
async fn status_changes_as_the_screen_does() {
    // 作業中: 実績開始 is today.
    let out = same_as_the_screen(
        "応募フォームの項目見直し",
        &["doing"],
        StatusChange::to(Status::Doing),
    )
    .await;
    assert_eq!(out["todo"]["status"], "doing");
    assert_eq!(out["todo"]["actual_start"], "2026-11-19");
    assert_eq!(out["closed_children"], json!([]));
    assert_eq!(out["next_round"], Value::Null);
    assert_eq!(out["repeat_ended"], false);

    // 待ち: the 確認日 is the screen's 「待ちにする」 date unless given.
    let s = sample().await;
    let form = s.id("応募フォームの項目見直し").await;
    let offered = s.item(form).await.follow_up_default;
    let out = same_as_the_screen(
        "応募フォームの項目見直し",
        &["waiting", "--waiting-on", "人事部の回答"],
        StatusChange {
            to: Status::Waiting,
            waiting_on: Some("人事部の回答".into()),
            follow_up_date: Some(offered),
            reason: None,
        },
    )
    .await;
    assert_eq!(
        out["todo"]["waiting"],
        json!({"on": "人事部の回答", "since": "2026-11-19", "follow_up_date": offered, "day": 1})
    );
    let out = same_as_the_screen(
        "応募フォームの項目見直し",
        &[
            "waiting",
            "--waiting-on",
            "人事部の回答",
            "--follow-up",
            "2026-11-30",
        ],
        StatusChange {
            to: Status::Waiting,
            waiting_on: Some("人事部の回答".into()),
            follow_up_date: Some(d("2026-11-30")),
            reason: None,
        },
    )
    .await;
    assert_eq!(out["todo"]["waiting"]["follow_up_date"], "2026-11-30");

    // Out of 待ち: its three fields are emptied.
    let out = same_as_the_screen(
        "API設計書のレビュー対応",
        &["doing"],
        StatusChange::to(Status::Doing),
    )
    .await;
    assert_eq!(out["todo"]["waiting"], Value::Null);

    // 完了 and 中止 (with its reason).
    let out = same_as_the_screen(
        "移行手順書のドラフト",
        &["done"],
        StatusChange::to(Status::Done),
    )
    .await;
    assert_eq!(out["todo"]["actual_end"], "2026-11-19");
    let out = same_as_the_screen(
        "応募フォームの項目見直し",
        &["cancelled", "--reason", "要件が変わったため"],
        StatusChange {
            reason: Some("要件が変わったため".into()),
            ..StatusChange::to(Status::Cancelled)
        },
    )
    .await;
    assert_eq!(out["todo"]["close_reason"], "要件が変わったため");

    // 再開: from 完了 back to 作業中.
    same_as_the_screen(
        "要件定義書の改訂",
        &["doing"],
        StatusChange::to(Status::Doing),
    )
    .await;
}

#[tokio::test]
async fn status_refuses_what_the_screen_refuses() {
    let s = sample().await;
    let done = s.id("要件定義書の改訂").await;
    let form = s.id("応募フォームの項目見直し").await;
    let before = s.state().await;

    // 完了 cannot go 中止.
    let e = s
        .err(&["status", &done.to_string(), "cancelled", "--reason", "x"])
        .await;
    assert_eq!(e.kind, CliErrorKind::Validation);
    let e = s
        .err(&["status", &form.to_string(), "waiting", "--waiting-on", " "])
        .await;
    assert_eq!(e.kind, CliErrorKind::Validation);
    let e = s
        .err(&["status", &form.to_string(), "cancelled", "--reason", "  "])
        .await;
    assert_eq!(e, CliError::validation("中止の理由を入力してください。"));
    // Flags of another status.
    let e = s
        .err(&["status", &form.to_string(), "done", "--reason", "済んだ"])
        .await;
    assert_eq!(
        e,
        CliError::validation("--reason は、cancelled(中止)にするときだけ使えます。")
    );
    let e = s
        .err(&[
            "status",
            &form.to_string(),
            "doing",
            "--follow-up",
            "2026-11-30",
        ])
        .await;
    assert_eq!(
        e,
        CliError::validation(
            "--waiting-on と --follow-up は、waiting(待ち)にするときだけ使えます。"
        )
    );
    let e = s.err(&["status", "9999", "doing"]).await;
    assert_eq!(e, CliError::todo_not_found(9999));
    assert_eq!(s.state().await, before);
}

#[tokio::test]
async fn starting_a_blocked_todo_needs_confirmation() {
    let s = sample().await;
    let plan = s.id("結合テスト計画の作成").await;
    let before = s.state().await;

    let e = s.err(&["status", &plan.to_string(), "doing"]).await;
    assert_eq!(
        e,
        CliError::new(
            CliErrorKind::NeedsConfirmation,
            "次の2件の先行TODOが、まだ完了も中止もしていません(移行手順書のドラフト、性能試験の環境準備)。このまま作業中にするときは --despite-blocked を付けてください。"
        )
    );
    // Nothing changed, and no backup was taken.
    assert_eq!(s.state().await, before);
    assert!(s.backup_files().is_empty());

    // Other statuses do not ask.
    s.ok(&["status", &plan.to_string(), "hold"]).await;
    let out = s
        .ok(&["status", &plan.to_string(), "doing", "--despite-blocked"])
        .await;
    assert_eq!(out["todo"]["status"], "doing");
    assert_eq!(out["todo"]["blocked_by"].as_array().unwrap().len(), 2);

    // Reopening a closed TODO does not ask (it waits for nothing then).
    let done = s.id("要件定義書の改訂").await;
    dependencies::add(&s.pool, done, plan, today(), now())
        .await
        .unwrap();
    let out = s.ok(&["status", &done.to_string(), "doing"]).await;
    assert_eq!(out["todo"]["status"], "doing");

    // A TODO to finish first in a hidden project is counted, not named.
    let s = sample().await;
    let board = s.id("監視ダッシュボードの整理").await;
    let migration = s.id("移行手順書のドラフト").await;
    let child = s.id("必要なツールの洗い出し").await;
    dependencies::add(&s.pool, board, migration, today(), now())
        .await
        .unwrap();
    // 「親 / 子」 for a child (closed ones do not count).
    let setup = s.id("セットアップ手順書の作成").await;
    dependencies::add(&s.pool, board, setup, today(), now())
        .await
        .unwrap();
    dependencies::add(&s.pool, board, child, today(), now())
        .await
        .unwrap();
    s.hide(vec![s.project("基幹システム更改").await]).await;
    let e = s.err(&["status", &board.to_string(), "doing"]).await;
    assert_eq!(
        e.message,
        "次の2件の先行TODOが、まだ完了も中止もしていません(CLIに出さないプロジェクトのTODO、開発環境の標準化 / セットアップ手順書の作成)。このまま作業中にするときは --despite-blocked を付けてください。"
    );
}

#[tokio::test]
async fn closing_a_parent_with_open_children_needs_confirmation() {
    let s = sample().await;
    let parent = s.id("開発環境の標準化").await;
    let setup = s.id("セットアップ手順書の作成").await;
    let trial = s.id("新メンバーでの試行").await;
    let before = s.state().await;

    let e = s.err(&["status", &parent.to_string(), "done"]).await;
    assert_eq!(
        e,
        CliError::new(
            CliErrorKind::NeedsConfirmation,
            "開いている子TODOが2件あります(セットアップ手順書の作成、新メンバーでの試行)。子TODOも一緒に完了にするときは --with-children を付けてください。"
        )
    );
    let e = s
        .err(&[
            "status",
            &parent.to_string(),
            "cancelled",
            "--reason",
            "見送り",
        ])
        .await;
    assert_eq!(
        e.message,
        "開いている子TODOが2件あります(セットアップ手順書の作成、新メンバーでの試行)。子TODOも一緒に中止にするときは --with-children を付けてください。"
    );
    assert_eq!(s.state().await, before);
    assert!(s.backup_files().is_empty());

    // With the flag, as the screen's 「完了にする」 after its dialog.
    let screen = sample().await;
    let out = s
        .ok(&["status", &parent.to_string(), "done", "--with-children"])
        .await;
    todos::change_status_impl(
        &screen.pool,
        parent,
        &StatusChange::to(Status::Done),
        today(),
        now(),
    )
    .await
    .unwrap();
    assert_eq!(s.state().await, screen.state().await);
    assert_eq!(out["todo"]["status"], "done");
    assert_eq!(
        out["closed_children"],
        json!([
            {"id": setup, "title": "セットアップ手順書の作成"},
            {"id": trial, "title": "新メンバーでの試行"},
        ])
    );
    assert_eq!(s.item(setup).await.todo.status, Status::Done);

    // Reopened, the children stay closed; closing again asks nothing.
    s.ok(&["status", &parent.to_string(), "doing"]).await;
    let out = s.ok(&["status", &parent.to_string(), "done"]).await;
    assert_eq!(out["closed_children"], json!([]));
}

#[tokio::test]
async fn closing_a_repeating_todo_names_the_next_round() {
    let s = sample().await;
    let screen = sample().await;
    let weekly = s.id("社内ツールの更新確認").await;

    // Not closing: nothing is made.
    let out = s.ok(&["status", &weekly.to_string(), "hold"]).await;
    assert_eq!(out["next_round"], Value::Null);
    assert_eq!(out["repeat_ended"], false);
    assert!(out["todo"]["repeat"].is_object());
    s.ok(&["status", &weekly.to_string(), "todo"]).await;

    let out = s.ok(&["status", &weekly.to_string(), "done"]).await;
    let next = out["next_round"]["id"].as_i64().unwrap();
    assert_eq!(out["next_round"]["due_date"], "2026-12-03");
    assert_eq!(out["repeat_ended"], false);
    assert_eq!(out["todo"]["repeat"], Value::Null);
    let round = s.item(next).await;
    assert_eq!(round.todo.title, "社内ツールの更新確認");
    assert_eq!(round.todo.status, Status::Todo);
    assert!(round.repeat.is_some());
    // As the screen makes it.
    todos::change_status_impl(
        &screen.pool,
        weekly,
        &StatusChange::to(Status::Hold),
        today(),
        now(),
    )
    .await
    .unwrap();
    todos::change_status_impl(
        &screen.pool,
        weekly,
        &StatusChange::to(Status::Todo),
        today(),
        now(),
    )
    .await
    .unwrap();
    todos::change_status_impl(
        &screen.pool,
        weekly,
        &StatusChange::to(Status::Done),
        today(),
        now(),
    )
    .await
    .unwrap();
    assert_eq!(s.state().await, screen.state().await);

    // Past the end date: no next round.
    repeats::set(
        &s.pool,
        next,
        Some(RepeatInput {
            rule: RepeatRule::Weekly {
                weekdays: vec![DayOfWeek::Thu],
            },
            until_date: Some(d("2026-12-03")),
        }),
        today(),
        now(),
    )
    .await
    .unwrap();
    let todos_before = s.everything().await.len();
    let out = s
        .ok(&[
            "status",
            &next.to_string(),
            "cancelled",
            "--reason",
            "不要になった",
        ])
        .await;
    assert_eq!(out["next_round"], Value::Null);
    assert_eq!(out["repeat_ended"], true);
    assert_eq!(out["todo"]["repeat"], Value::Null);
    assert_eq!(s.everything().await.len(), todos_before);
}

// ---- log ---------------------------------------------------------------------------

#[tokio::test]
async fn log_adds_a_work_log_as_the_screen_does() {
    let s = sample().await;
    let screen = sample().await;
    let plan = s.id("結合テスト計画の作成").await;

    let out = s
        .ok(&["log", &plan.to_string(), "--body", "- 範囲の案を作った\n"])
        .await;
    assert_eq!(out["todo_id"], plan);
    assert_eq!(out["log_date"], "2026-11-19");
    assert_eq!(out["minutes"], Value::Null);
    assert_eq!(out["body_md"], "- 範囲の案を作った");
    let saved = work_logs::save(
        &screen.pool,
        WorkLogInput {
            id: None,
            todo_id: plan,
            log_date: today(),
            body_md: "- 範囲の案を作った\n".into(),
            minutes: None,
        },
        now(),
    )
    .await
    .unwrap();
    assert_eq!(out["id"], saved.id);
    assert_eq!(s.state().await, screen.state().await);

    let out = s
        .ok(&[
            "log",
            &plan.to_string(),
            "--body",
            "見直した",
            "--date",
            "2026-11-18",
            "--minutes",
            "90",
        ])
        .await;
    assert_eq!(out["log_date"], "2026-11-18");
    assert_eq!(out["minutes"], 90);

    // Several lines from standard input, Windows line breaks too.
    let out = s
        .cli_reading(
            &["log", &plan.to_string(), "--body", "-"],
            "## 今日\r\n\r\n- 1行目\r\n- 2行目\r\n",
        )
        .await
        .unwrap();
    assert_eq!(out["body_md"], "## 今日\n\n- 1行目\n- 2行目");
    let shown = s.ok(&["show", &plan.to_string()]).await;
    assert_eq!(shown["work_logs_total"], 3);
    assert_eq!(
        shown["work_logs"][0]["body_md"],
        "## 今日\n\n- 1行目\n- 2行目"
    );

    let before = s.state().await;
    let e = s.err(&["log", &plan.to_string(), "--body", " \n "]).await;
    assert_eq!(
        e,
        CliError::validation("作業ログの本文を入力してください。")
    );
    for minutes in ["-1", "1441"] {
        let e = s
            .err(&[
                "log",
                &plan.to_string(),
                "--body",
                "x",
                "--minutes",
                minutes,
            ])
            .await;
        assert_eq!(
            e,
            CliError::validation("作業時間は0〜1440分の範囲で入力してください。"),
            "{minutes}"
        );
    }
    let e = s.err(&["log", "9999", "--body", "x"]).await;
    assert_eq!(e, CliError::todo_not_found(9999));
    assert_eq!(s.state().await, before);
}

// ---- append-memo -------------------------------------------------------------------

#[tokio::test]
async fn append_memo_adds_after_one_empty_line_and_changes_nothing_else() {
    let s = sample().await;
    let plan = s.id("結合テスト計画の作成").await;
    let form = s.id("応募フォームの項目見直し").await;

    // An empty memo: the text only.
    let out = s
        .ok(&[
            "append-memo",
            &plan.to_string(),
            "--body",
            "  決めたこと  \n",
        ])
        .await;
    assert_eq!(out["memo_md"], "決めたこと");
    assert_eq!(out["id"], plan);
    assert_eq!(out["title"], "結合テスト計画の作成");

    // Blanks at its end go; the start stays.
    s.set_memo(form, "\n  字下げ\n\n- 項目\n\n \n").await;
    let before = s.item(form).await.todo;
    let lines = s.timeline(form).await;
    let out = s
        .cli_reading(
            &["append-memo", &form.to_string(), "--body", "-"],
            "## 追記\n\n- 確認した\n",
        )
        .await
        .unwrap();
    let memo = "\n  字下げ\n\n- 項目\n\n## 追記\n\n- 確認した";
    assert_eq!(out["memo_md"], memo);
    let after = s.item(form).await.todo;
    assert_eq!(
        unstamped(after.clone()),
        unstamped(Todo {
            memo_md: memo.into(),
            ..before.clone()
        })
    );
    assert_eq!(after.updated_at, now());
    // A memo is not recorded.
    assert_eq!(s.timeline(form).await, lines);

    // A 待ち TODO keeps its 待ち fields.
    let api = s.id("API設計書のレビュー対応").await;
    let waiting = s.item(api).await.todo;
    s.ok(&[
        "append-memo",
        &api.to_string(),
        "--body",
        "回答の期限を聞く",
    ])
    .await;
    assert_eq!(
        unstamped(s.item(api).await.todo),
        unstamped(Todo {
            memo_md: "回答の期限を聞く".into(),
            ..waiting
        })
    );

    let unchanged = s.state().await;
    let e = s
        .err(&["append-memo", &form.to_string(), "--body", " \r\n "])
        .await;
    assert_eq!(e, CliError::validation("書き足す本文を入力してください。"));
    let e = s.err(&["append-memo", "9999", "--body", "x"]).await;
    assert_eq!(e, CliError::todo_not_found(9999));
    assert_eq!(s.state().await, unchanged);
}

// ---- projects the CLI does not show, and the backup ----------------------------------

#[tokio::test]
async fn hidden_projects_refuse_every_write_and_take_no_backup() {
    let s = sample().await;
    let social = s.project("社内ツール整備").await;
    let board = s.id("監視ダッシュボードの整理").await;
    let parent = s.id("開発環境の標準化").await;
    s.hide(vec![social]).await;
    let before = s.state().await;

    let id = board.to_string();
    let p = parent.to_string();
    let social_id = social.to_string();
    let cases: [&[&str]; 9] = [
        &["add", "--project", "社内ツール整備", "--title", "追加"],
        &["add", "--project", &social_id, "--title", "追加"],
        &["update", &id, "--due", "2026-12-24"],
        &["status", &id, "doing"],
        &["status", &p, "done", "--with-children"],
        &["log", &id, "--body", "作業した"],
        &["append-memo", &id, "--body", "追記"],
        // A parent in it, named for another project's TODO.
        &[
            "add",
            "--project",
            "基幹システム更改",
            "--title",
            "子",
            "--parent",
            &p,
        ],
        &["log", &p, "--body", "作業した", "--minutes", "30"],
    ];
    for args in cases {
        let e = s.err(args).await;
        assert_eq!(e.kind, CliErrorKind::Hidden, "{args:?}");
        let expected = if args[0] == "add" && args.len() == 5 {
            CliError::hidden_project()
        } else {
            CliError::hidden_todo()
        };
        assert_eq!(e, expected, "{args:?}");
    }
    assert_eq!(s.state().await, before);
    assert!(s.backup_files().is_empty());
}

#[tokio::test]
async fn todays_backup_is_taken_before_the_first_write() {
    let s = sample().await;
    let plan = s.id("結合テスト計画の作成").await;
    let id = plan.to_string();

    // Reading takes none.
    for args in [
        &["today"][..],
        &["list"],
        &["show", &id],
        &["search", "手順"],
        &["projects", "--archived"],
    ] {
        s.ok(args).await;
    }
    assert!(!s.backups().exists());

    // A refused write takes none either (checked before the backup).
    s.err(&["append-memo", &id, "--body", " "]).await;
    assert!(!s.backups().exists());

    s.ok(&["log", &id, "--body", "作業した"]).await;
    assert_eq!(s.backup_files(), ["todo-2026-11-19.db"]);
    let file = s.backups().join("todo-2026-11-19.db");
    // The backup is the data before the write: no work log yet.
    let copy = db::open_existing(&file).await.unwrap();
    assert_eq!(
        work_logs::timeline(&copy, plan)
            .await
            .unwrap()
            .iter()
            .filter(|e| matches!(e, TimelineEntry::WorkLog(_)))
            .count(),
        0
    );
    copy.close().await;
    let files = s.backup_files();
    let taken = std::fs::metadata(&file).unwrap().modified().unwrap();

    // Today's is there: the next writes take none.
    s.ok(&["status", &id, "hold"]).await;
    s.ok(&["append-memo", &id, "--body", "追記"]).await;
    assert_eq!(s.backup_files(), files);
    assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), taken);
}

#[tokio::test]
async fn nothing_is_written_without_the_backup() {
    let s = sample().await;
    let plan = s.id("結合テスト計画の作成").await;
    // The backups folder cannot be made: a file has its name.
    std::fs::write(s.backups(), b"").unwrap();
    let before = s.state().await;
    let id = plan.to_string();
    for args in [
        &["add", "--project", "基幹システム更改", "--title", "追加"][..],
        &["update", &id, "--due", "2026-12-24"],
        &["status", &id, "hold"],
        &["log", &id, "--body", "作業した"],
        &["append-memo", &id, "--body", "追記"],
    ] {
        let e = s.err(args).await;
        assert_eq!(e, CliError::no_backup(), "{args:?}");
        assert_eq!(
            e.message,
            "バックアップを作れなかったため、書き込みをやめました。Higiri を起動して、バックアップが作られるか確かめてください。"
        );
    }
    assert_eq!(s.state().await, before);
}

// ---- the app writing at the same time ------------------------------------------------

#[tokio::test]
async fn a_write_waits_while_the_app_writes() {
    let s = sample().await;
    let plan = s.id("結合テスト計画の作成").await;
    let form = s.id("応募フォームの項目見直し").await;
    // The app holds the write lock for a moment (a save of the detail panel).
    let app = db::open(&s.dir.join(db::DB_FILE_NAME)).await.unwrap();
    let held = db::begin_write(&app).await.unwrap();
    let ends = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        held.commit().await.unwrap();
    });
    let start = std::time::Instant::now();
    let out = s
        .ok(&["log", &plan.to_string(), "--body", "待ってから書いた"])
        .await;
    assert!(start.elapsed() >= std::time::Duration::from_millis(250));
    assert_eq!(out["body_md"], "待ってから書いた");
    ends.await.unwrap();

    // And the other way round: the screen's save waits for the CLI.
    let held = db::begin_write(&app).await.unwrap();
    let ends = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        held.commit().await.unwrap();
    });
    s.set_memo(form, "画面で保存した").await;
    ends.await.unwrap();
    assert_eq!(s.item(form).await.todo.memo_md, "画面で保存した");
    app.close().await;
}

#[tokio::test]
async fn a_write_still_locked_after_the_wait_is_refused() {
    let s = sample().await;
    let plan = s.id("結合テスト計画の作成").await;
    let file = s.dir.join(db::DB_FILE_NAME);
    let app = db::open(&file).await.unwrap();
    // The CLI waits less here than its 5 seconds.
    let short = db::open_existing_waiting(&file, std::time::Duration::from_millis(100))
        .await
        .unwrap();
    let held = db::begin_write(&app).await.unwrap();
    let before = s.state().await;
    let id = plan.to_string();
    for args in [
        &["add", "--project", "基幹システム更改", "--title", "追加"][..],
        &["update", &id, "--due", "2026-12-24"],
        &["status", &id, "hold"],
        &["log", &id, "--body", "作業した"],
        &["append-memo", &id, "--body", "追記"],
    ] {
        let cli =
            Cli::try_parse_from(std::iter::once("higiri-cli").chain(args.iter().copied())).unwrap();
        let e = super::super::dispatch(&short, &cli.command, &s.dir, today(), now())
            .await
            .unwrap_err();
        assert_eq!(e, CliError::busy(), "{args:?}");
        assert_eq!(
            e.message,
            "Higiri がデータを書き換えている最中です。少し待ってから、もう一度実行してください。"
        );
    }
    held.rollback().await.unwrap();
    assert_eq!(s.state().await, before);
    short.close().await;
    app.close().await;
}
