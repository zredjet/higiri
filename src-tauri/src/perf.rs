//! Performance check (非機能要件「性能」): 5,000 TODOs and 20,000 work logs.
//!
//! Every command is measured twice: with the default 暦日 setting, then with
//! 営業日 and a yearly custom holiday (年末年始).
//!
//! Not part of the normal test run. Run it optimized:
//!
//! ```text
//! cargo test --release --manifest-path src-tauri/Cargo.toml perf -- --ignored --nocapture
//! ```
//!
//! Set `HIGIRI_PERF_DB=/path/to/todo.db` to keep the generated database (for
//! trying it in the app); otherwise it is written to a temporary folder.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::{Days, NaiveDate, TimeZone, Utc};
use sqlx::SqlitePool;

use crate::commands::{dashboard, gantt, projects, reports, settings, todos};
use crate::domain::date::{format_date, format_timestamp};
use crate::domain::listing::{DueFilter, TodoSort};
use crate::domain::model::{Priority, Status};
use crate::domain::report::ReportKind;
use crate::domain::settings::{CustomHoliday, DayCount, Settings};
use crate::repo::db;

pub const PROJECTS: usize = 20;
pub const TODOS: usize = 5_000;
pub const WORK_LOGS: usize = 20_000;

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
    format!("{:>7.1} ms", d.as_secs_f64() * 1000.0)
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

    let runs = 7;
    vec![
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
    ]
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
    println!("open + migrations check      {}", ms(t.elapsed()));

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
        "{:<28} {:>10} {:>10}",
        "command (ms)", "calendar", "business"
    );
    for ((name, c), (_, b)) in calendar.iter().zip(&business) {
        println!("{name:<28} {} {}", ms(*c), ms(*b));
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
        items.items.len(),
        g.projects.iter().map(|p| p.rows.len()).sum::<usize>()
    );

    let t = Instant::now();
    crate::backup::run_daily(&pool, tmp.path(), today)
        .await
        .unwrap();
    println!("daily backup (VACUUM INTO)   {}", ms(t.elapsed()));
    pool.close().await;
}
