//! Performance check (非機能要件「性能」): 5,000 TODOs and 20,000 work logs.
//! 1,000 of the TODOs are children (S-1): 500 parents with 2 children each.
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
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::{Days, NaiveDate, TimeZone, Utc};
use sqlx::SqlitePool;

use crate::commands::{dashboard, gantt, projects, reports, search, settings, todos};
use crate::domain::date::{format_date, format_timestamp};
use crate::domain::listing::{DueFilter, TodoSort};
use crate::domain::model::{Priority, Status, Todo};
use crate::domain::report::ReportKind;
use crate::domain::settings::{CustomHoliday, DayCount, Settings};
use crate::repo::db;

pub const PROJECTS: usize = 20;
pub const TODOS: usize = 5_000;
pub const WORK_LOGS: usize = 20_000;
/// Children among the TODOs (S-1).
pub const CHILDREN: usize = 1_000;

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
            median(runs, || reports::generate(pool, report, None)).await,
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
    pool.close().await;
}
