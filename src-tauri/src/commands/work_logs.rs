//! 作業ログ: list_timeline, save_work_log, delete_work_log.

use std::cmp::Reverse;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use super::clock;
use super::error::{AppError, CmdResult};
use super::validate;
use crate::domain::events::TodoEvent;
use crate::domain::model::WorkLog;
use crate::domain::timeline::describe;
use crate::repo;
use crate::repo::work_logs::{NewWorkLog, WorkLogEdit};

/// Upper bound for 作業時間 (minutes in a day).
const MAX_MINUTES: i64 = 24 * 60;

/// One line of the detail panel's timeline: a work log or a recorded change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "type", content = "entry", rename_all = "snake_case")]
pub enum TimelineEntry {
    WorkLog(WorkLog),
    Event(EventEntry),
}

/// A recorded change with its text (「ステータスを「未着手」から「作業中」に変更。…」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct EventEntry {
    pub event: TodoEvent,
    pub text: String,
}

impl TimelineEntry {
    /// (day, time written): newest first is the display order.
    fn sort_key(&self) -> (NaiveDate, DateTime<Utc>, i64) {
        match self {
            TimelineEntry::WorkLog(l) => (l.log_date, l.created_at, l.id),
            TimelineEntry::Event(e) => (e.event.event_date, e.event.occurred_at, e.event.id),
        }
    }
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct WorkLogInput {
    /// `null` adds a new log; otherwise updates that log.
    pub id: Option<i64>,
    pub todo_id: i64,
    pub log_date: NaiveDate,
    pub body_md: String,
    /// 作業時間 in minutes (optional).
    pub minutes: Option<i64>,
}

// ---- tauri commands ----------------------------------------------------------

/// Work logs and recorded changes of one TODO, newest first.
#[tauri::command]
#[specta::specta]
pub async fn list_timeline(
    pool: State<'_, SqlitePool>,
    todo_id: i64,
) -> CmdResult<Vec<TimelineEntry>> {
    timeline(&pool, todo_id).await
}

/// Adds (`id = null`) or updates a work log.
#[tauri::command]
#[specta::specta]
pub async fn save_work_log(pool: State<'_, SqlitePool>, input: WorkLogInput) -> CmdResult<WorkLog> {
    save(&pool, input, clock::now()).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_work_log(pool: State<'_, SqlitePool>, id: i64) -> CmdResult<()> {
    delete(&pool, id).await
}

// ---- implementation ----------------------------------------------------------

pub async fn timeline(pool: &SqlitePool, todo_id: i64) -> CmdResult<Vec<TimelineEntry>> {
    let mut conn = pool.acquire().await?;
    repo::todos::get(&mut conn, todo_id).await?;
    let mut entries: Vec<TimelineEntry> = repo::work_logs::list_by_todo(&mut conn, todo_id)
        .await?
        .into_iter()
        .map(TimelineEntry::WorkLog)
        .chain(
            repo::events::list_by_todo(&mut conn, todo_id)
                .await?
                .into_iter()
                .map(|event| {
                    let text = describe(&event);
                    TimelineEntry::Event(EventEntry { event, text })
                }),
        )
        .collect();
    entries.sort_by_key(|e| Reverse(e.sort_key()));
    Ok(entries)
}

pub async fn save(
    pool: &SqlitePool,
    input: WorkLogInput,
    now: DateTime<Utc>,
) -> CmdResult<WorkLog> {
    let body_md = validate::required_text(&input.body_md, "作業ログの本文")?;
    if let Some(m) = input.minutes {
        validate::in_range(m, 0, MAX_MINUTES, "作業時間", "分")?;
    }

    let mut tx = pool.begin().await?;
    repo::todos::get(&mut tx, input.todo_id).await?;
    let saved = match input.id {
        None => {
            repo::work_logs::insert(
                &mut tx,
                &NewWorkLog {
                    todo_id: input.todo_id,
                    log_date: input.log_date,
                    body_md,
                    minutes: input.minutes,
                },
                now,
            )
            .await?
        }
        Some(id) => {
            let current = repo::work_logs::get(&mut tx, id).await?;
            if current.todo_id != input.todo_id {
                return Err(AppError::validation(
                    "別のTODOの作業ログは変更できません。TODOを開き直してください。",
                ));
            }
            repo::work_logs::update(
                &mut tx,
                id,
                &WorkLogEdit {
                    log_date: input.log_date,
                    body_md,
                    minutes: input.minutes,
                },
                now,
            )
            .await?
        }
    };
    tx.commit().await?;
    Ok(saved)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> CmdResult<()> {
    let mut conn = pool.acquire().await?;
    Ok(repo::work_logs::delete(&mut conn, id).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::commands::projects::{self, ProjectInput};
    use crate::commands::todos::{self, NewTodoInput};
    use crate::domain::model::Status;
    use crate::domain::status::StatusChange;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    async fn setup() -> (SqlitePool, i64) {
        let pool = db::open_in_memory().await.unwrap();
        let now = ts(2026, 11, 16, 0, 0);
        let p = projects::create(
            &pool,
            ProjectInput {
                name: "P".into(),
                color: None,
                description: String::new(),
            },
            now,
        )
        .await
        .unwrap();
        let t = todos::create(
            &pool,
            NewTodoInput {
                project_id: p.id,
                parent_id: None,
                title: "API設計書のレビュー対応".into(),
                priority: None,
                due_date: None,
                planned_start: None,
                planned_end: None,
                memo_md: None,
            },
            d("2026-11-16"),
            now,
        )
        .await
        .unwrap();
        (pool, t.todo.id)
    }

    fn log(todo_id: i64, date: &str, body: &str, minutes: Option<i64>) -> WorkLogInput {
        WorkLogInput {
            id: None,
            todo_id,
            log_date: d(date),
            body_md: body.into(),
            minutes,
        }
    }

    #[tokio::test]
    async fn timeline_mixes_logs_and_events_newest_first() {
        let (pool, tid) = setup().await;
        todos::change_status_impl(
            &pool,
            tid,
            &StatusChange::to(Status::Doing),
            d("2026-11-17"),
            ts(2026, 11, 17, 1, 0),
        )
        .await
        .unwrap();
        save(
            &pool,
            log(tid, "2026-11-18", "レビュー指摘を整理", Some(120)),
            ts(2026, 11, 18, 9, 0),
        )
        .await
        .unwrap();
        todos::change_status_impl(
            &pool,
            tid,
            &StatusChange::waiting("認証担当の回答", d("2026-11-20")),
            d("2026-11-19"),
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();

        let entries = timeline(&pool, tid).await.unwrap();
        let shape: Vec<String> = entries
            .iter()
            .map(|e| match e {
                TimelineEntry::WorkLog(l) => format!("log {}", l.log_date),
                TimelineEntry::Event(ev) => format!(
                    "{} {} {}",
                    ev.event.kind.as_str(),
                    ev.event.event_date,
                    ev.event.new_value.as_deref().unwrap_or("")
                ),
            })
            .collect();
        assert_eq!(
            shape,
            vec![
                "status 2026-11-19 waiting",
                "log 2026-11-18",
                "status 2026-11-17 doing",
                "created 2026-11-16 todo",
            ]
        );
        assert_eq!(
            timeline(&pool, 999).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn timeline_serializes_with_a_type_tag() {
        let (pool, tid) = setup().await;
        let entries = timeline(&pool, tid).await.unwrap();
        let json = serde_json::to_value(&entries[0]).unwrap();
        assert_eq!(json["type"], "event");
        assert_eq!(json["entry"]["event"]["kind"], "created");
        assert_eq!(json["entry"]["event"]["note"], serde_json::json!({}));
        assert_eq!(json["entry"]["text"], "TODOを作成");
    }

    #[tokio::test]
    async fn save_adds_then_updates() {
        let (pool, tid) = setup().await;
        let now = ts(2026, 11, 19, 9, 0);
        let added = save(
            &pool,
            log(tid, "2026-11-19", "  - 指摘を反映 ", Some(90)),
            now,
        )
        .await
        .unwrap();
        assert_eq!(added.body_md, "- 指摘を反映");

        let later = ts(2026, 11, 19, 10, 0);
        let updated = save(
            &pool,
            WorkLogInput {
                id: Some(added.id),
                minutes: None,
                ..log(tid, "2026-11-19", "- 指摘12件のうち9件を反映", None)
            },
            later,
        )
        .await
        .unwrap();
        assert_eq!(updated.id, added.id);
        assert_eq!(updated.minutes, None);
        assert_eq!(updated.updated_at, later);

        delete(&pool, added.id).await.unwrap();
        assert_eq!(
            delete(&pool, added.id).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn save_validates_input() {
        let (pool, tid) = setup().await;
        let now = ts(2026, 11, 19, 9, 0);
        assert_eq!(
            save(&pool, log(tid, "2026-11-19", "  ", None), now)
                .await
                .unwrap_err()
                .message,
            "作業ログの本文を入力してください。"
        );
        assert_eq!(
            save(&pool, log(tid, "2026-11-19", "x", Some(-5)), now)
                .await
                .unwrap_err()
                .message,
            "作業時間は0〜1440分の範囲で入力してください。"
        );
        assert_eq!(
            save(&pool, log(999, "2026-11-19", "x", None), now)
                .await
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn save_refuses_to_move_a_log_to_another_todo() {
        let (pool, tid) = setup().await;
        let now = ts(2026, 11, 19, 9, 0);
        let added = save(&pool, log(tid, "2026-11-19", "x", None), now)
            .await
            .unwrap();
        let mut c = pool.acquire().await.unwrap();
        let other = repo::todos::insert(
            &mut c,
            &repo::todos::NewTodo {
                project_id: 1,
                parent_id: None,
                title: "別".into(),
                priority: Default::default(),
                due_date: None,
                planned_start: None,
                planned_end: None,
                memo_md: String::new(),
            },
            now,
        )
        .await
        .unwrap();
        drop(c);
        let e = save(
            &pool,
            WorkLogInput {
                id: Some(added.id),
                ..log(other.id, "2026-11-19", "x", None)
            },
            now,
        )
        .await
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Validation);
    }
}
