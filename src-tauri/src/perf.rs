//! Performance check (非機能要件「性能」): 5,000 TODOs and 20,000 work logs.
//! 1,000 of the TODOs are children (S-1): 500 parents with 2 children each.
//! 30 TODO templates (T-10) with 5 children each, each child waiting for the
//! one before it. 300 repeating TODOs (T-12), open and parentless, of every
//! kind of rule. 1,000 dependencies (T-11), mostly between nearby TODOs (of
//! other projects, as projects take turns), never a parent and its child.
//!
//! Every command is measured twice: with the default 暦日 setting, then with
//! 営業日 and a yearly custom holiday (年末年始).
//!
//! The generated texts repeat, so the searches pick words that hit very
//! different numbers of TODOs (see `SEARCHES`).
//!
//! Not part of the normal test run. Run it optimized:
//!
//! ```text
//! cargo test --release --manifest-path src-tauri/Cargo.toml perf -- --ignored --nocapture
//! ```
//!
//! Set `HIGIRI_PERF_DB=/path/to/todo.db` to keep the generated database (for
//! trying it in the app); otherwise it is written to a temporary folder.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{Days, NaiveDate, TimeZone, Utc};
use sqlx::SqlitePool;

use crate::commands::{
    bulk, dashboard, dependencies, gantt, projects, repeats, reports, search, settings, templates,
    todos,
};
use crate::domain::bulk::BulkChange;
use crate::domain::date::{format_date, format_timestamp};
use crate::domain::listing::{DueFilter, TodoSort};
use crate::domain::model::{Priority, Status, Todo};
use crate::domain::repeat::{DayOfWeek, RepeatRule};
use crate::domain::report::ReportKind;
use crate::domain::settings::{CustomHoliday, DayCount, Settings};
use crate::domain::status::StatusChange;
use crate::domain::template::{TemplateBody, TemplateChild};
use crate::repo::db;

pub const PROJECTS: usize = 20;
pub const TODOS: usize = 5_000;
pub const WORK_LOGS: usize = 20_000;
/// Children among the TODOs (S-1).
pub const CHILDREN: usize = 1_000;
/// TODO templates (T-10), each with [`TEMPLATE_CHILDREN`] children.
pub const TEMPLATES: usize = 30;
pub const TEMPLATE_CHILDREN: usize = 5;
/// Repeating TODOs (T-12): the first open, parentless TODOs with a 日限.
pub const REPEATS: usize = 300;
/// Dependencies (T-11). The later TODO always has the larger id, so there is
/// no circle.
pub const DEPENDENCIES: usize = 1_000;

/// Searches measured (label, words). The seed's texts: titles
/// 「TODO 0001 の作業」, memos 「- 進め方のメモ…」, 待っている相手
/// 「相手の回答」 (about 10%), 中止の理由 「不要になったため」 (about 5%), work
/// logs 「- 作業ログ {n}: 手順を確認し、…」.
const SEARCHES: [(&str, &str); 6] = [
    // 2 characters, every title: the most results to order.
    ("search_todos (every TODO)", "作業"),
    // 2 characters in 待っている相手; the rest scan memo, reason and logs.
    ("search_todos (waiting_on)", "回答"),
    // 2 characters in 中止の理由.
    ("search_todos (close_reason)", "不要"),
    // Two words, a few TODOs: 「作業ログ」 only in logs, 「1234」 in one
    // title and a few logs.
    ("search_todos (2 words, few)", "作業ログ 1234"),
    // ASCII case ignored: 「TODO」 in every title, 「0042」 in one.
    ("search_todos (ASCII case)", "todo 0042"),
    // No hit: every field of every TODO is read.
    ("search_todos (no hit)", "該当なし"),
];

/// TODOs selected for 一括変更 (T-14).
pub const BULK: usize = 1_000;

fn bulk_status(status: Status) -> BulkChange {
    BulkChange::Status {
        status,
        reason: None,
    }
}

/// Small deterministic generator, so every run measures the same data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn days(&mut self, from: i64, to: i64) -> i64 {
        from + self.below((to - from + 1) as u64) as i64
    }
}

fn shift(today: NaiveDate, days: i64) -> NaiveDate {
    if days >= 0 {
        today + Days::new(days as u64)
    } else {
        today - Days::new((-days) as u64)
    }
}

fn at(day: NaiveDate, minute: u64) -> String {
    let t = Utc
        .from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
        .checked_add_signed(chrono::Duration::minutes((minute % 600) as i64 + 60))
        .unwrap();
    format_timestamp(t)
}

/// Fills an empty database with the measured volume, with dates around `today`.
pub async fn seed(pool: &SqlitePool, today: NaiveDate) -> sqlx::Result<()> {
    let mut rng = Rng(19);
    let mut tx = pool.begin().await?;
    let created = at(shift(today, -300), 0);
    for p in 1..=PROJECTS {
        sqlx::query(
            "INSERT INTO projects (id, name, sort_order, created_at, updated_at) VALUES (?1, ?2, ?1, ?3, ?3)",
        )
        .bind(p as i64)
        .bind(format!("プロジェクト{p:02}"))
        .bind(&created)
        .execute(&mut *tx)
        .await?;
    }

    for id in 1..=TODOS as i64 {
        let project = (id - 1) % PROJECTS as i64 + 1;
        let status = match rng.below(100) {
            0..=39 => Status::Done,
            40..=44 => Status::Cancelled,
            45..=64 => Status::Todo,
            65..=79 => Status::Doing,
            80..=89 => Status::Waiting,
            _ => Status::Hold,
        };
        let created_day = shift(today, rng.days(-300, -1));
        let planned_start = shift(created_day, rng.days(0, 20));
        let planned_end = shift(planned_start, rng.days(0, 30));
        let due = (rng.below(10) < 8).then(|| shift(today, rng.days(-30, 60)));
        let started = !matches!(status, Status::Todo | Status::Cancelled);
        let actual_start = started.then(|| shift(planned_start, rng.days(0, 5)).min(today));
        let actual_end = (status == Status::Done).then(|| {
            let start = actual_start.unwrap_or(planned_start);
            shift(start, rng.days(0, 20)).min(today).max(start)
        });
        let closed_day = match status {
            Status::Done => actual_end,
            Status::Cancelled => Some(shift(created_day, rng.days(0, 30)).min(today)),
            _ => None,
        };
        let waiting = status == Status::Waiting;
        let waiting_since = waiting.then(|| shift(today, rng.days(-10, 0)));
        sqlx::query(
            "INSERT INTO todos (id, project_id, title, status, priority, due_date, planned_start, planned_end,
                actual_start, actual_end, waiting_on, waiting_since, follow_up_date, closed_at, close_reason,
                memo_md, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?1, ?17, ?17)",
        )
        .bind(id)
        .bind(project)
        .bind(format!("TODO {id:04} の作業"))
        .bind(status.as_str())
        .bind([Priority::High, Priority::Medium, Priority::Low][rng.below(3) as usize].as_i64())
        .bind(due.map(format_date))
        .bind(format_date(planned_start))
        .bind(format_date(planned_end))
        .bind(actual_start.map(format_date))
        .bind(actual_end.map(format_date))
        .bind(waiting.then_some("相手の回答"))
        .bind(waiting_since.map(format_date))
        .bind(waiting_since.map(|d| format_date(shift(d, 3))))
        .bind(closed_day.map(|d| at(d, id as u64)))
        .bind((status == Status::Cancelled).then_some("不要になったため"))
        .bind("- 進め方のメモ\n- 確認したいこと")
        .bind(at(created_day, id as u64))
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO todo_events (todo_id, event_date, kind, old_value, new_value, note, occurred_at)
             VALUES (?1, ?2, 'created', NULL, 'todo', '{}', ?3)",
        )
        .bind(id)
        .bind(format_date(created_day))
        .bind(at(created_day, id as u64))
        .execute(&mut *tx)
        .await?;
        if status != Status::Todo {
            let day = closed_day
                .or(waiting_since)
                .or(actual_start)
                .unwrap_or(today);
            sqlx::query(
                "INSERT INTO todo_events (todo_id, event_date, kind, old_value, new_value, note, occurred_at)
                 VALUES (?1, ?2, 'status', 'doing', ?3, '{\"auto\":{}}', ?4)",
            )
            .bind(id)
            .bind(format_date(day))
            .bind(status.as_str())
            .bind(at(day, id as u64 + 1))
            .execute(&mut *tx)
            .await?;
        }
    }

    // サブTODO: in each project, of every 10 TODOs in a row the 8th and 9th
    // become children of the 7th (same project, one level).
    sqlx::query(
        "UPDATE todos SET parent_id = id - ?1 * (((id - 1) / ?1) % 10 - 6)
         WHERE ((id - 1) / ?1) % 10 IN (7, 8)",
    )
    .bind(PROJECTS as i64)
    .execute(&mut *tx)
    .await?;

    for n in 0..WORK_LOGS {
        let todo = rng.below(TODOS as u64) as i64 + 1;
        let day = shift(today, rng.days(-365, 0));
        sqlx::query(
            "INSERT INTO work_logs (todo_id, log_date, body_md, minutes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        )
        .bind(todo)
        .bind(format_date(day))
        .bind(format!(
            "- 作業ログ {n}: 手順を確認し、気になった点をまとめた\n- 次は担当者に確認する"
        ))
        .bind((rng.below(4) > 0).then(|| (rng.below(8) as i64 + 1) * 15))
        .bind(at(day, n as u64))
        .execute(&mut *tx)
        .await?;
    }

    // TODOテンプレート: 「テンプレート 01」…, by name.
    for n in 1..=TEMPLATES {
        let child = |c: usize| TemplateChild {
            title: format!("手順 {c}"),
            priority: Priority::Medium,
            memo_md: "- 確認すること".into(),
            due_offset_days: Some(c as i64 - TEMPLATE_CHILDREN as i64),
            // Each child waits for the one before it (positions from 0).
            depends_on: if c > 1 { vec![c - 2] } else { Vec::new() },
        };
        let body = TemplateBody {
            title: format!("定例の作業 {n:02}"),
            priority: Priority::Medium,
            memo_md: "## 手順\n- 前回の記録を見る".into(),
            children: (1..=TEMPLATE_CHILDREN).map(child).collect(),
        };
        sqlx::query(
            "INSERT INTO todo_templates (name, body, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        )
        .bind(format!("テンプレート {n:02}"))
        .bind(body.to_json())
        .bind(&created)
        .execute(&mut *tx)
        .await?;
    }

    // 繰り返し: every kind of rule in turn.
    let rules = [
        RepeatRule::Daily,
        RepeatRule::Weekly {
            weekdays: vec![DayOfWeek::Mon, DayOfWeek::Thu],
        },
        RepeatRule::MonthlyDay { day: 31 },
        RepeatRule::MonthlyWeekday {
            week: 2,
            weekday: DayOfWeek::Tue,
        },
        RepeatRule::MonthlyLast,
        RepeatRule::Yearly { month: 2, day: 29 },
        RepeatRule::EveryDays { days: 10 },
    ];
    let ids: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM todos
         WHERE parent_id IS NULL AND due_date IS NOT NULL
           AND status IN ('todo', 'doing', 'waiting', 'hold')
         ORDER BY id LIMIT ?1",
    )
    .bind(REPEATS as i64)
    .fetch_all(&mut *tx)
    .await?;
    for (n, (id,)) in ids.into_iter().enumerate() {
        sqlx::query(
            "INSERT INTO todo_repeats (todo_id, rule, until_date, created_at, updated_at)
             VALUES (?1, ?2, NULL, ?3, ?3)",
        )
        .bind(id)
        .bind(rules[n % rules.len()].to_json())
        .bind(&created)
        .execute(&mut *tx)
        .await?;
    }

    // 依存関係: a later TODO waits for one up to 60 ids before it (now and
    // then much further). Pairs already there and a parent with its own
    // child are skipped.
    let mut joined = 0;
    while joined < DEPENDENCIES {
        let later = rng.below(TODOS as u64 - 1) as i64 + 2;
        let reach = if rng.below(10) == 0 {
            later - 1
        } else {
            (later - 1).min(60)
        };
        let first = later - 1 - rng.below(reach as u64) as i64;
        let done = sqlx::query(
            "INSERT OR IGNORE INTO todo_dependencies (todo_id, depends_on_id, created_at)
             SELECT ?1, ?2, ?3
             WHERE NOT EXISTS (SELECT 1 FROM todos
                               WHERE (id = ?1 AND parent_id = ?2) OR (id = ?2 AND parent_id = ?1))",
        )
        .bind(later)
        .bind(first)
        .bind(&created)
        .execute(&mut *tx)
        .await?;
        joined += done.rows_affected() as usize;
    }
    tx.commit().await
}

/// Median of several runs (the first run warms the page cache and is dropped).
async fn median<F, Fut, T>(runs: usize, mut f: F) -> Duration
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let _ = f().await;
    let mut times = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        let _ = f().await;
        times.push(t.elapsed());
    }
    times.sort();
    times[runs / 2]
}

fn ms(d: Duration) -> String {
    format!("{:>8.2} ms", d.as_secs_f64() * 1000.0)
}

/// The fields `update_todo` takes, as the screen's `fieldsOf` builds them.
fn fields_of(t: &Todo) -> todos::TodoFields {
    todos::TodoFields {
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

/// Label of the commands the first screen calls (see `measure`).
const FIRST_SCREEN: &str = "first screen (4 commands)";

/// `higiri-cli <args>` on the database `db` (v0.5.0): from opening it to
/// the JSON (the process start is not in it). Returns the JSON's length.
async fn cli(db: &Path, data_dir: &Path, args: &[&str], today: NaiveDate) -> usize {
    use clap::Parser;
    let parsed = crate::cli::args::Cli::try_parse_from(
        std::iter::once("higiri-cli").chain(args.iter().copied()),
    )
    .unwrap();
    let now = Utc.from_utc_datetime(&today.and_hms_opt(1, 30, 0).unwrap());
    crate::cli::run_on(&parsed.command, db, data_dir, today, now)
        .await
        .unwrap()
        .len()
}

/// The median time of higiri-cli's read sub-commands (0.5 s each at most),
/// then of `today` and `search` with one project hidden from the CLI.
async fn measure_cli(
    pool: &SqlitePool,
    db: &Path,
    data_dir: &Path,
    today: NaiveDate,
) -> Vec<(&'static str, Duration)> {
    let runs = 7;
    // A parent with children, work logs and dependencies.
    let (shown,): (i64,) = sqlx::query_as(
        "SELECT t.parent_id FROM todos t
         WHERE t.parent_id IS NOT NULL
           AND EXISTS (SELECT 1 FROM work_logs w WHERE w.todo_id = t.parent_id)
         ORDER BY t.parent_id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let show = shown.to_string();
    let cases: [(&'static str, Vec<&str>); 7] = [
        ("cli today", vec!["today"]),
        ("cli list (50 of all)", vec!["list"]),
        (
            "cli list (filtered, 1 project)",
            vec![
                "list",
                "--project",
                "3",
                "--status",
                "todo,doing",
                "--sort",
                "due_date",
            ],
        ),
        ("cli show (parent, 5 logs)", vec!["show", &show]),
        ("cli search (every TODO)", vec!["search", "作業"]),
        ("cli search (no hit)", vec!["search", "該当なし"]),
        ("cli projects --archived", vec!["projects", "--archived"]),
    ];
    let mut times = Vec::new();
    for (label, args) in &cases {
        times.push((
            *label,
            median(runs, || cli(db, data_dir, args, today)).await,
        ));
    }
    // One project hidden: its TODOs are read to hide the links to them.
    let base = settings::get(pool).await.unwrap();
    settings::update(
        pool,
        settings::SettingsInput {
            cli_hidden_projects: vec![2],
            ..settings::SettingsInput::from(&base)
        },
    )
    .await
    .unwrap();
    times.push((
        "cli today (1 project hidden)",
        median(runs, || cli(db, data_dir, &["today"], today)).await,
    ));
    times.push((
        "cli search (1 project hidden)",
        median(runs, || cli(db, data_dir, &["search", "作業"], today)).await,
    ));
    settings::update(pool, settings::SettingsInput::from(&base))
        .await
        .unwrap();
    times
}

/// The median time of higiri-cli's write sub-commands (0.5 s each at
/// most), on a copy of the database in a folder of its own (the counts
/// checked later stay). Today's daily backup is already there, as after the
/// first write or launch of the day; the first write's backup is timed once.
async fn measure_cli_writes(
    pool: &SqlitePool,
    tmp: &Path,
    today: NaiveDate,
) -> Vec<(&'static str, Duration)> {
    let runs = 7;
    let dir = tmp.join("cli-writes");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(db::DB_FILE_NAME);
    sqlx::query("VACUUM INTO ?1")
        .bind(file.display().to_string())
        .execute(pool)
        .await
        .unwrap();
    let copied = db::open(&file).await.unwrap();
    // An open TODO of its own: no parent, children, repetition or TODO to
    // finish first (no confirmation).
    let (plain,): (i64,) = sqlx::query_as(
        "SELECT t.id FROM todos t
         WHERE t.status = 'todo' AND t.parent_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM todos c WHERE c.parent_id = t.id)
           AND NOT EXISTS (SELECT 1 FROM todo_repeats r WHERE r.todo_id = t.id)
           AND NOT EXISTS (SELECT 1 FROM todo_dependencies d WHERE d.todo_id = t.id)
         ORDER BY t.id LIMIT 1",
    )
    .fetch_one(&copied)
    .await
    .unwrap();
    // Open parents with open children, one for each run.
    let parents: Vec<(i64,)> = sqlx::query_as(
        "SELECT p.id FROM todos p
         WHERE p.status NOT IN ('done', 'cancelled') AND p.parent_id IS NULL
           AND EXISTS (SELECT 1 FROM todos c
                       WHERE c.parent_id = p.id AND c.status NOT IN ('done', 'cancelled'))
         ORDER BY p.id LIMIT ?1",
    )
    .bind(runs as i64 + 1)
    .fetch_all(&copied)
    .await
    .unwrap();
    copied.close().await;
    assert_eq!(parents.len(), runs + 1);
    let (file, dir) = (&file, &dir);
    let plain = plain.to_string();
    let plain = plain.as_str();

    // The first write of the day takes the daily backup first.
    let first_log = ["log", plain, "--body", "計測の作業ログ"];
    let t = Instant::now();
    cli(file, dir, &first_log, today).await;
    let first = t.elapsed();
    assert!(
        crate::backup::dir(dir)
            .join(format!("todo-{today}.db"))
            .is_file()
    );
    let mut times = vec![("cli log (taking the backup)", first)];

    let add = [
        "add",
        "--project",
        "3",
        "--title",
        "計測で足すTODO",
        "--due",
        "2026-11-30",
    ];
    times.push((
        "cli add",
        median(runs, || cli(file, dir, &add, today)).await,
    ));
    let doing = Cell::new(false);
    times.push((
        "cli status (todo ↔ doing)",
        median(runs, || {
            doing.set(!doing.get());
            let to = if doing.get() { "doing" } else { "todo" };
            async move { cli(file, dir, &["status", plain, to], today).await }
        })
        .await,
    ));
    let next = Cell::new(0);
    let parent_ids: Vec<String> = parents.iter().map(|(id,)| id.to_string()).collect();
    times.push((
        "cli status (with children)",
        median(runs, || {
            let id = parent_ids[next.get()].as_str();
            next.set(next.get() + 1);
            async move { cli(file, dir, &["status", id, "done", "--with-children"], today).await }
        })
        .await,
    ));
    let log = ["log", plain, "--body", "計測の作業ログ", "--minutes", "30"];
    times.push((
        "cli log",
        median(runs, || cli(file, dir, &log, today)).await,
    ));
    let memo = ["append-memo", plain, "--body", "計測で書き足す"];
    times.push((
        "cli append-memo",
        median(runs, || cli(file, dir, &memo, today)).await,
    ));
    times
}

/// The median time of each measured command.
async fn measure(pool: &SqlitePool, today: NaiveDate) -> Vec<(&'static str, Duration)> {
    let all = todos::TodoQuery {
        project_id: None,
        statuses: None,
        priority: None,
        due: None,
        sort: None,
    };
    let filtered = todos::TodoQuery {
        project_id: Some(3),
        statuses: Some(vec![Status::Doing, Status::Waiting]),
        priority: Some(Priority::High),
        due: Some(DueFilter::WithinWeek),
        sort: Some(TodoSort::DueDate),
    };
    let until_today = todos::TodoQuery {
        due: Some(DueFilter::UntilToday),
        ..all.clone()
    };
    let gantt_all = gantt::GanttQuery {
        project_id: None,
        include_done: true,
        include_cancelled: false,
    };
    let gantt_one = gantt::GanttQuery {
        project_id: Some(3),
        ..gantt_all.clone()
    };
    let report = reports::ReportKey {
        kind: ReportKind::Daily,
        date: today,
    };
    let weekly_report = reports::ReportKey {
        kind: ReportKind::Weekly,
        date: today,
    };

    let runs = 7;
    let mut times = vec![
        (
            "get_dashboard",
            median(runs, || dashboard::get(pool, today)).await,
        ),
        (
            "get_startup_reminder",
            median(runs, || dashboard::reminder(pool, today)).await,
        ),
        (
            "list_projects",
            median(runs, || projects::list(pool, false, today)).await,
        ),
        (
            "list_todos (all 5,000)",
            median(runs, || todos::list(pool, &all, today)).await,
        ),
        (
            "list_todos (filtered)",
            median(runs, || todos::list(pool, &filtered, today)).await,
        ),
        (
            "list_todos (until today)",
            median(runs, || todos::list(pool, &until_today, today)).await,
        ),
        (
            "get_gantt (all)",
            median(runs, || gantt::get(pool, &gantt_all, today)).await,
        ),
        (
            "get_gantt (1 project)",
            median(runs, || gantt::get(pool, &gantt_one, today)).await,
        ),
        (
            "generate_report",
            median(runs, || reports::generate(pool, report, None, today)).await,
        ),
        (
            "generate_report (weekly)",
            median(runs, || reports::generate(pool, weekly_report, None, today)).await,
        ),
    ];
    for (label, words) in SEARCHES {
        times.push((
            label,
            median(runs, || search::search(pool, words, today)).await,
        ));
    }

    // Saving a TODO (since フェーズ13, useUpdateTodo in queries.ts): get_todo
    // reads the TODO as saved right now, then update_todo sends it with only
    // the caller's change. Measured on an open TODO, one call after the other.
    let (id,): (i64,) =
        sqlx::query_as("SELECT id FROM todos WHERE status = 'doing' ORDER BY id LIMIT 1")
            .fetch_one(pool)
            .await
            .unwrap();
    times.push((
        "get_todo",
        median(runs, || todos::get(pool, id, today)).await,
    ));
    // A Gantt drag: the planned span moves a day (back and forth), which
    // also records a `planned` event.
    let back = Cell::new(false);
    times.push((
        "get_todo + update_todo (drag)",
        median(runs, || {
            let days = if back.replace(!back.get()) { -1 } else { 1 };
            async move {
                let current = todos::get(pool, id, today).await.unwrap();
                let mut fields = fields_of(&current.todo);
                fields.planned_start = fields.planned_start.map(|d| shift(d, days));
                fields.planned_end = fields.planned_end.map(|d| shift(d, days));
                todos::update(pool, id, fields, today, Utc::now())
                    .await
                    .unwrap();
            }
        })
        .await,
    ));
    // The detail panel's autosave of the memo (no event).
    let n = Cell::new(0);
    times.push((
        "get_todo + update_todo (memo)",
        median(runs, || {
            n.set(n.get() + 1);
            let memo = format!("- 進め方のメモ\n- 確認したいこと {}", n.get());
            async move {
                let current = todos::get(pool, id, today).await.unwrap();
                let fields = todos::TodoFields {
                    memo_md: memo,
                    ..fields_of(&current.todo)
                };
                todos::update(pool, id, fields, today, Utc::now())
                    .await
                    .unwrap();
            }
        })
        .await,
    ));

    // TODOテンプレート (T-10): the list of the 「TODOを追加」 dialog, saving a
    // parent and its 2 children again under a name that is taken (replace),
    // and making a TODO with its 5 children from a template (deleted again,
    // so the counts stay).
    times.push((
        "list_todo_templates",
        median(runs, || templates::list(pool)).await,
    ));
    let (parent,): (i64,) = sqlx::query_as(
        "SELECT parent_id FROM todos WHERE parent_id IS NOT NULL ORDER BY id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    times.push((
        "save_todo_template (replace)",
        median(runs, || async {
            templates::save(pool, parent, "テンプレート 01", true, Utc::now())
                .await
                .unwrap();
        })
        .await,
    ));
    let (template_id,): (i64,) =
        sqlx::query_as("SELECT id FROM todo_templates WHERE name = 'テンプレート 02'")
            .fetch_one(pool)
            .await
            .unwrap();
    times.push((
        "create_todo (template) + delete",
        median(runs, || async {
            let made = todos::create(
                pool,
                todos::NewTodoInput {
                    project_id: 3,
                    parent_id: None,
                    template_id: Some(template_id),
                    title: "定例の作業".into(),
                    priority: None,
                    due_date: Some(shift(today, 14)),
                    planned_start: None,
                    planned_end: None,
                    memo_md: None,
                },
                today,
                Utc::now(),
            )
            .await
            .unwrap();
            todos::delete(pool, made.todo.id).await.unwrap();
        })
        .await,
    ));

    // 繰り返し (T-12): changing a rule (back and forth), and closing a
    // repeating TODO with 5 children, which makes the next round and its
    // children in the same transaction. Only the change_status is timed:
    // the TODO is made from a template before and both rounds are deleted
    // after, so the counts stay.
    let (repeating,): (i64,) =
        sqlx::query_as("SELECT todo_id FROM todo_repeats ORDER BY todo_id LIMIT 1")
            .fetch_one(pool)
            .await
            .unwrap();
    let flip = Cell::new(false);
    times.push((
        "set_todo_repeat",
        median(runs, || {
            let rule = if flip.replace(!flip.get()) {
                RepeatRule::MonthlyLast
            } else {
                RepeatRule::Weekly {
                    weekdays: vec![DayOfWeek::Mon],
                }
            };
            async move {
                let input = repeats::RepeatInput {
                    rule,
                    until_date: None,
                };
                repeats::set(pool, repeating, Some(input), today, Utc::now())
                    .await
                    .unwrap();
            }
        })
        .await,
    ));
    let mut closes = Vec::with_capacity(runs + 1);
    for _ in 0..=runs {
        let made = todos::create(
            pool,
            todos::NewTodoInput {
                project_id: 3,
                parent_id: None,
                template_id: Some(template_id),
                title: "定例の作業".into(),
                priority: None,
                due_date: Some(shift(today, 14)),
                planned_start: Some(shift(today, 10)),
                planned_end: Some(shift(today, 13)),
                memo_md: None,
            },
            today,
            Utc::now(),
        )
        .await
        .unwrap();
        let id = made.todo.id;
        let weekly = repeats::RepeatInput {
            rule: RepeatRule::Weekly {
                weekdays: vec![DayOfWeek::Mon],
            },
            until_date: None,
        };
        repeats::set(pool, id, Some(weekly), today, Utc::now())
            .await
            .unwrap();
        let t = Instant::now();
        todos::change_status_impl(pool, id, &StatusChange::to(Status::Done), today, Utc::now())
            .await
            .unwrap();
        closes.push(t.elapsed());
        let (next,): (i64,) = sqlx::query_as(
            "SELECT id FROM todos WHERE id > ?1 AND parent_id IS NULL ORDER BY id LIMIT 1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
        todos::delete(pool, next).await.unwrap();
        todos::delete(pool, id).await.unwrap();
    }
    closes.remove(0);
    closes.sort();
    times.push(("change_status (repeat round)", closes[runs / 2]));

    // 依存関係 (T-11): the detail panel's lists, and joining a pair (the
    // circle check reads every dependency) then removing it again, so the
    // count stays.
    let (busy,): (i64,) = sqlx::query_as(
        "SELECT todo_id FROM todo_dependencies
         WHERE todo_id IN (SELECT depends_on_id FROM todo_dependencies)
         ORDER BY todo_id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    times.push((
        "list_dependencies",
        median(runs, || dependencies::list(pool, busy, today)).await,
    ));
    let (later, first): (i64, i64) = sqlx::query_as(
        "SELECT a.id, b.id FROM todos a JOIN todos b ON b.id < a.id
         WHERE a.parent_id IS NULL AND b.parent_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM todo_dependencies d
                           WHERE d.todo_id = a.id AND d.depends_on_id = b.id)
         ORDER BY a.id DESC, b.id DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    times.push((
        "add_dependency + remove",
        median(runs, || async {
            dependencies::add(pool, later, first, today, Utc::now())
                .await
                .unwrap();
            dependencies::remove(pool, later, first, today, Utc::now())
                .await
                .unwrap();
        })
        .await,
    ));

    // 後続TODOのずらし (G-8): the TODO with the most TODOs after it (後続の後続
    // too), its plan dragged to end a year later, so that every open one with
    // a 予定開始 is pushed. Nothing is written.
    let all_deps = {
        let mut c = pool.acquire().await.unwrap();
        crate::repo::dependencies::list(&mut c).await.unwrap()
    };
    let (head, after) = all_deps
        .iter()
        .map(|d| d.depends_on_id)
        .map(|id| (id, crate::domain::shift::reachable(id, &all_deps).len()))
        .max_by_key(|&(id, n)| (n, std::cmp::Reverse(id)))
        .unwrap();
    let head_start = todos::get(pool, head, today)
        .await
        .unwrap()
        .todo
        .planned_start
        .unwrap();
    let far = shift(today, 365);
    let moved = crate::commands::shift::preview(pool, head, head_start, far)
        .await
        .unwrap();
    let direct: Vec<i64> = all_deps
        .iter()
        .filter(|d| d.depends_on_id == head)
        .map(|d| d.todo_id)
        .collect();
    assert!(
        moved.iter().any(|s| !direct.contains(&s.id)),
        "the later TODOs of later TODOs move too"
    );
    println!(
        "dependent shift: TODO {head}, {after} TODOs after it, {} moved",
        moved.len()
    );
    times.push((
        "preview_dependent_shift",
        median(runs, || {
            crate::commands::shift::preview(pool, head, head_start, far)
        })
        .await,
    ));

    // 一括変更 (T-14): 1,000 TODOs selected (the first 1,000: every status,
    // parents with their children, repeating TODOs). The confirmations write
    // nothing. The saves move the 日限 by +7 and back, and put 1,000 open
    // TODOs on 保留 and back, so nothing stays changed (closing is measured
    // on a copy, in `perf_5000_todos_20000_logs`). The history rows they add
    // are deleted after, so the later measures see the same data.
    let (events_before,): (i64,) = sqlx::query_as("SELECT MAX(id) FROM todo_events")
        .fetch_one(pool)
        .await
        .unwrap();
    let first: Vec<i64> = (1..=BULK as i64).collect();
    for (label, change) in [
        (
            "preview_bulk_change (due)",
            BulkChange::ShiftDue { days: 7 },
        ),
        ("preview_bulk_change (done)", bulk_status(Status::Done)),
        ("preview_bulk_change (doing)", bulk_status(Status::Doing)),
    ] {
        times.push((
            label,
            median(runs, || bulk::preview(pool, &first, change.clone(), today)).await,
        ));
    }
    let mut moves = Vec::with_capacity(runs + 1);
    for _ in 0..=runs {
        let t = Instant::now();
        let done = bulk::apply(
            pool,
            &first,
            BulkChange::ShiftDue { days: 7 },
            today,
            Utc::now(),
        )
        .await
        .unwrap();
        moves.push(t.elapsed());
        assert!(done.changed.len() > 300, "{} moved", done.changed.len());
        bulk::apply(
            pool,
            &first,
            BulkChange::ShiftDue { days: -7 },
            today,
            Utc::now(),
        )
        .await
        .unwrap();
    }
    moves.remove(0);
    moves.sort();
    times.push(("apply_bulk_change (due)", moves[runs / 2]));
    let open: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, status FROM todos WHERE status IN ('todo', 'doing') ORDER BY id LIMIT ?1",
    )
    .bind(BULK as i64)
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(open.len(), BULK);
    let open_ids: Vec<i64> = open.iter().map(|(id, _)| *id).collect();
    let back = |status: &str| -> Vec<i64> {
        open.iter()
            .filter(|(_, s)| s == status)
            .map(|(id, _)| *id)
            .collect()
    };
    let (was_todo, was_doing) = (back("todo"), back("doing"));
    let mut holds = Vec::with_capacity(runs + 1);
    for _ in 0..=runs {
        let t = Instant::now();
        let done = bulk::apply(
            pool,
            &open_ids,
            bulk_status(Status::Hold),
            today,
            Utc::now(),
        )
        .await
        .unwrap();
        holds.push(t.elapsed());
        assert_eq!(done.changed.len(), BULK);
        bulk::apply(
            pool,
            &was_todo,
            bulk_status(Status::Todo),
            today,
            Utc::now(),
        )
        .await
        .unwrap();
        bulk::apply(
            pool,
            &was_doing,
            bulk_status(Status::Doing),
            today,
            Utc::now(),
        )
        .await
        .unwrap();
    }
    holds.remove(0);
    holds.sort();
    times.push(("apply_bulk_change (hold)", holds[runs / 2]));
    sqlx::query("DELETE FROM todo_events WHERE id > ?1")
        .bind(events_before)
        .execute(pool)
        .await
        .unwrap();

    // What the first screen asks for (App.tsx: settings, projects, the
    // dashboard and its badge, the startup reminder). The screen sends them
    // at once; here they run one after the other.
    times.push((
        FIRST_SCREEN,
        median(runs, || async {
            settings::get(pool).await.unwrap();
            projects::list(pool, false, today).await.unwrap();
            dashboard::get(pool, today).await.unwrap();
            dashboard::reminder(pool, today).await.unwrap();
        })
        .await,
    ));
    times
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "performance check; run with --release -- --ignored --nocapture"]
async fn perf_5000_todos_20000_logs() {
    let today = NaiveDate::from_ymd_opt(2026, 11, 19).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let path = std::env::var_os("HIGIRI_PERF_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| tmp.path().join(db::DB_FILE_NAME));
    assert!(!path.exists(), "{} already exists", path.display());

    let pool = db::open(&path).await.unwrap();
    let t = Instant::now();
    seed(&pool, today).await.unwrap();
    println!(
        "seed {} TODOs / {} logs: {}",
        TODOS,
        WORK_LOGS,
        ms(t.elapsed())
    );
    pool.close().await;

    let t = Instant::now();
    let pool = db::open(&path).await.unwrap();
    let opened = t.elapsed();
    println!("{:<30} {}", "open + migrations check", ms(opened));

    let calendar = measure(&pool, today).await;

    // 営業日, with 年末年始 (12/29〜1/3) every year.
    let base = settings::get(&pool).await.unwrap();
    settings::update(
        &pool,
        settings::SettingsInput {
            day_count: DayCount::Business,
            custom_holidays: vec![CustomHoliday {
                name: "年末年始".into(),
                start: NaiveDate::from_ymd_opt(2026, 12, 29).unwrap(),
                end: NaiveDate::from_ymd_opt(2027, 1, 3).unwrap(),
                yearly: true,
            }],
            ..settings::SettingsInput::from(&base)
        },
    )
    .await
    .unwrap();
    let business = measure(&pool, today).await;

    let cli_business = measure_cli(&pool, &path, tmp.path(), today).await;

    println!(
        "{:<30} {:>11} {:>11}",
        "command (ms)", "calendar", "business"
    );
    for ((name, c), (_, b)) in calendar.iter().zip(&business) {
        println!("{name:<30} {} {}", ms(*c), ms(*b));
    }
    settings::update(&pool, settings::SettingsInput::from(&Settings::default()))
        .await
        .unwrap();
    let cli_calendar = measure_cli(&pool, &path, tmp.path(), today).await;
    for ((name, c), (_, b)) in cli_calendar.iter().zip(&cli_business) {
        println!("{name:<30} {} {}", ms(*c), ms(*b));
        assert!(
            *c < Duration::from_millis(500) && *b < Duration::from_millis(500),
            "{name}: 0.5 s at most"
        );
    }
    for (name, t) in measure_cli_writes(&pool, tmp.path(), today).await {
        println!("{name:<30} {}", ms(t));
        assert!(t < Duration::from_millis(500), "{name}: 0.5 s at most");
    }

    let all = todos::TodoQuery {
        project_id: None,
        statuses: None,
        priority: None,
        due: None,
        sort: None,
    };
    let gantt_all = gantt::GanttQuery {
        project_id: None,
        include_done: true,
        include_cancelled: false,
    };
    let items = todos::list(&pool, &all, today).await.unwrap();
    let g = gantt::get(&pool, &gantt_all, today).await.unwrap();
    println!(
        "rows: list {} / gantt {}",
        items.rows.len(),
        g.projects.iter().map(|p| p.rows.len()).sum::<usize>()
    );
    for (label, words) in SEARCHES {
        let found = search::search(&pool, words, today).await.unwrap();
        println!(
            "{label:<30} 「{words}」 total {} / returned {}",
            found.total,
            found.hits.len()
        );
    }

    let t = Instant::now();
    crate::backup::run_daily(&pool, tmp.path(), today)
        .await
        .unwrap();
    let backed_up = t.elapsed();
    println!("{:<30} {}", "daily backup (VACUUM INTO)", ms(backed_up));

    // C-3: the 7 daily backups to an empty mirror folder, then the
    // dashboard with the mirror's notice (it lists the backups folder).
    let backups = crate::backup::dir(tmp.path());
    let newest = backups.join(format!("todo-{today}.db"));
    for days_ago in 1..7 {
        let day = shift(today, -days_ago);
        std::fs::copy(&newest, backups.join(format!("todo-{day}.db"))).unwrap();
    }
    let mirror = tmp.path().join("mirror");
    std::fs::create_dir(&mirror).unwrap();
    let t = Instant::now();
    crate::backup::sync_mirror(tmp.path(), &mirror).unwrap();
    println!(
        "{:<30} {}",
        format!(
            "backup mirror (7 x {:.1} MB)",
            std::fs::metadata(&newest).unwrap().len() as f64 / 1_000_000.0
        ),
        ms(t.elapsed())
    );
    {
        let mut c = pool.acquire().await.unwrap();
        crate::repo::settings::set_backup_mirror(
            &mut c,
            Some(&mirror.display().to_string()),
            Some(shift(today, -7)),
        )
        .await
        .unwrap();
    }
    let with_notice = median(7, || dashboard::with_backup_alert(&pool, tmp.path(), today)).await;
    assert!(
        dashboard::with_backup_alert(&pool, tmp.path(), today)
            .await
            .unwrap()
            .backup_mirror_alert
            .is_some()
    );
    println!(
        "{:<30} {}",
        "get_dashboard (mirror notice)",
        ms(with_notice)
    );
    {
        let mut c = pool.acquire().await.unwrap();
        crate::repo::settings::set_backup_mirror(&mut c, None, None)
            .await
            .unwrap();
    }

    // 起動2秒 (Rust side): what setup does before the window (open the
    // database, the first launch of the day's backup) and the first screen's
    // commands. The WebView's own loading is not in this number.
    let first_screen = |times: &[(&str, Duration)]| {
        times
            .iter()
            .find(|(label, _)| *label == FIRST_SCREEN)
            .map(|(_, d)| *d)
            .unwrap()
    };
    println!(
        "{:<30} {} {}",
        "startup, Rust side",
        ms(opened + backed_up + first_screen(&calendar)),
        ms(opened + backed_up + first_screen(&business))
    );

    // C-2: export everything to JSON, then import it back (into the same
    // database: the work database, the checks, the kept copy and the swap).
    let file = tmp.path().join("higiri-export.json");
    let now = Utc::now();
    let t = Instant::now();
    crate::json_data::export_to(&pool, &file, "perf", now)
        .await
        .unwrap();
    let size = std::fs::metadata(&file).unwrap().len();
    println!(
        "{:<30} {}",
        format!("export_data (JSON, {:.1} MB)", size as f64 / 1_000_000.0),
        ms(t.elapsed())
    );
    let t = Instant::now();
    crate::json_data::import(&pool, tmp.path(), &file, now)
        .await
        .unwrap();
    println!("{:<30} {}", "import_data (JSON)", ms(t.elapsed()));
    // 一括変更 (T-14): closing 1,000 open TODOs (their open children close
    // with them, the repeating ones make their next round), on a copy of the
    // database, so the counts below stay.
    let copy = tmp.path().join("bulk-close.db");
    sqlx::query("VACUUM INTO ?1")
        .bind(copy.display().to_string())
        .execute(&pool)
        .await
        .unwrap();
    {
        let copied = db::open(&copy).await.unwrap();
        let open: Vec<(i64,)> = sqlx::query_as(
            "SELECT id FROM todos WHERE status NOT IN ('done', 'cancelled') ORDER BY id LIMIT ?1",
        )
        .bind(BULK as i64)
        .fetch_all(&copied)
        .await
        .unwrap();
        let ids: Vec<i64> = open.into_iter().map(|(id,)| id).collect();
        let preview = bulk::preview(&copied, &ids, bulk_status(Status::Done), today)
            .await
            .unwrap();
        let t = Instant::now();
        bulk::apply(&copied, &ids, bulk_status(Status::Done), today, Utc::now())
            .await
            .unwrap();
        println!(
            "{:<30} {}",
            format!(
                "apply_bulk_change (done: {} + {} children, {} rounds)",
                preview.targets.len(),
                preview.closing_children,
                preview.next_rounds
            ),
            ms(t.elapsed())
        );
        copied.close().await;
    }

    let counts: (i64, i64) =
        sqlx::query_as("SELECT (SELECT COUNT(*) FROM todos), (SELECT COUNT(*) FROM work_logs)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(counts, (TODOS as i64, WORK_LOGS as i64));
    let (children,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM todos WHERE parent_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(children, CHILDREN as i64);
    let (template_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM todo_templates")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(template_count, TEMPLATES as i64);
    let (repeat_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM todo_repeats")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(repeat_count, REPEATS as i64);
    let (dependency_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM todo_dependencies")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dependency_count, DEPENDENCIES as i64);
    pool.close().await;
}
