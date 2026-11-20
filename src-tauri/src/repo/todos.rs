//! `todos` table.
//!
//! Status columns are written only through [`write_status`] (used by
//! `change_status`); [`update_fields`] never touches `status`, `waiting_since`
//! or `closed_at`.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, SqliteConnection};

use super::{
    RepoError, Result, date_param, opt_ts_param, read_date, read_opt_ts, read_ts, ts_param,
};
use crate::domain::model::{Priority, Status, Todo};
use crate::domain::status::StatusFields;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTodo {
    pub project_id: i64,
    /// The parent (S-1). The caller checks the family rules
    /// (`domain::subtodo`).
    pub parent_id: Option<i64>,
    pub title: String,
    pub priority: Priority,
    pub due_date: Option<NaiveDate>,
    pub planned_start: Option<NaiveDate>,
    pub planned_end: Option<NaiveDate>,
    pub memo_md: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TodoFilter {
    /// Only this project.
    pub project_id: Option<i64>,
    /// Only these statuses (`None` = all).
    pub statuses: Option<Vec<Status>>,
    /// Also include TODOs of archived projects.
    pub include_archived_projects: bool,
}

/// Number of TODOs per project and status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusCount {
    pub project_id: i64,
    pub status: Status,
    pub count: i64,
}

#[derive(FromRow)]
struct TodoRow {
    id: i64,
    project_id: i64,
    parent_id: Option<i64>,
    title: String,
    status: String,
    priority: i64,
    due_date: Option<String>,
    planned_start: Option<String>,
    planned_end: Option<String>,
    actual_start: Option<String>,
    actual_end: Option<String>,
    waiting_on: Option<String>,
    waiting_since: Option<String>,
    follow_up_date: Option<String>,
    closed_at: Option<String>,
    close_reason: Option<String>,
    memo_md: String,
    sort_order: i64,
    created_at: String,
    updated_at: String,
}

impl TodoRow {
    fn into_todo(self) -> Result<Todo> {
        let status = Status::parse(&self.status).ok_or(RepoError::Corrupt {
            column: "todos.status",
            value: self.status.clone(),
        })?;
        let priority = Priority::from_i64(self.priority).ok_or(RepoError::Corrupt {
            column: "todos.priority",
            value: self.priority.to_string(),
        })?;
        Ok(Todo {
            id: self.id,
            project_id: self.project_id,
            parent_id: self.parent_id,
            title: self.title,
            status,
            priority,
            due_date: read_date("todos.due_date", self.due_date)?,
            planned_start: read_date("todos.planned_start", self.planned_start)?,
            planned_end: read_date("todos.planned_end", self.planned_end)?,
            actual_start: read_date("todos.actual_start", self.actual_start)?,
            actual_end: read_date("todos.actual_end", self.actual_end)?,
            waiting_on: self.waiting_on,
            waiting_since: read_date("todos.waiting_since", self.waiting_since)?,
            follow_up_date: read_date("todos.follow_up_date", self.follow_up_date)?,
            closed_at: read_opt_ts("todos.closed_at", self.closed_at)?,
            close_reason: self.close_reason,
            memo_md: self.memo_md,
            sort_order: self.sort_order,
            created_at: read_ts("todos.created_at", self.created_at)?,
            updated_at: read_ts("todos.updated_at", self.updated_at)?,
        })
    }
}

fn not_found(id: i64) -> RepoError {
    RepoError::NotFound { entity: "todo", id }
}

/// Adds a TODO (status 未着手) at the end of its project.
pub async fn insert(
    conn: &mut SqliteConnection,
    new: &NewTodo,
    now: DateTime<Utc>,
) -> Result<Todo> {
    sqlx::query_as::<_, TodoRow>(
        "INSERT INTO todos
           (project_id, title, priority, due_date, planned_start, planned_end, memo_md,
            sort_order, created_at, updated_at, parent_id)
         VALUES
           (?1, ?2, ?3, ?4, ?5, ?6, ?7,
            (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM todos WHERE project_id = ?1), ?8, ?8, ?9)
         RETURNING *",
    )
    .bind(new.project_id)
    .bind(&new.title)
    .bind(new.priority.as_i64())
    .bind(date_param(new.due_date))
    .bind(date_param(new.planned_start))
    .bind(date_param(new.planned_end))
    .bind(&new.memo_md)
    .bind(ts_param(now))
    .bind(new.parent_id)
    .fetch_one(&mut *conn)
    .await?
    .into_todo()
}

pub async fn get(conn: &mut SqliteConnection, id: i64) -> Result<Todo> {
    sqlx::query_as::<_, TodoRow>("SELECT * FROM todos WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| not_found(id))?
        .into_todo()
}

/// TODOs matching `filter`, in project order and then TODO order.
pub async fn list(conn: &mut SqliteConnection, filter: &TodoFilter) -> Result<Vec<Todo>> {
    let statuses = filter.statuses.as_ref().map(|list| {
        let names: Vec<&str> = list.iter().map(|s| s.as_str()).collect();
        serde_json::Value::from(names).to_string()
    });
    sqlx::query_as::<_, TodoRow>(
        "SELECT t.* FROM todos t
         JOIN projects p ON p.id = t.project_id
         WHERE (?1 IS NULL OR t.project_id = ?1)
           AND (?2 IS NULL OR t.status IN (SELECT value FROM json_each(?2)))
           AND (?3 OR p.archived_at IS NULL)
         ORDER BY p.sort_order, p.id, t.sort_order, t.id",
    )
    .bind(filter.project_id)
    .bind(statuses)
    .bind(filter.include_archived_projects)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(TodoRow::into_todo)
    .collect()
}

/// The TODOs with these ids, in the order of `ids`. Missing ids are skipped.
pub async fn list_by_ids(conn: &mut SqliteConnection, ids: &[i64]) -> Result<Vec<Todo>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut rows: HashMap<i64, TodoRow> = sqlx::query_as::<_, TodoRow>(
        "SELECT * FROM todos WHERE id IN (SELECT value FROM json_each(?1))",
    )
    .bind(serde_json::Value::from(ids.to_vec()).to_string())
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|r| (r.id, r))
    .collect();
    ids.iter()
        .filter_map(|id| rows.remove(id))
        .map(TodoRow::into_todo)
        .collect()
}

/// The children of these parents (S-1), in TODO order.
pub async fn list_children(conn: &mut SqliteConnection, parent_ids: &[i64]) -> Result<Vec<Todo>> {
    if parent_ids.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, TodoRow>(
        "SELECT * FROM todos WHERE parent_id IN (SELECT value FROM json_each(?1))
         ORDER BY sort_order, id",
    )
    .bind(serde_json::Value::from(parent_ids.to_vec()).to_string())
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(TodoRow::into_todo)
    .collect()
}

/// Sets (`Some`) or clears (`None`) the parent (S-1). The caller checks the
/// family rules (`domain::subtodo`).
pub async fn set_parent(
    conn: &mut SqliteConnection,
    id: i64,
    parent_id: Option<i64>,
    now: DateTime<Utc>,
) -> Result<Todo> {
    sqlx::query_as::<_, TodoRow>(
        "UPDATE todos SET parent_id = ?2, updated_at = ?3 WHERE id = ?1 RETURNING *",
    )
    .bind(id)
    .bind(parent_id)
    .bind(ts_param(now))
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_found(id))?
    .into_todo()
}

/// Writes the editable (non-status) fields of `todo`: title, priority, due
/// date, planned and actual periods, waiting-on, follow-up date, cancel reason
/// and memo.
pub async fn update_fields(
    conn: &mut SqliteConnection,
    todo: &Todo,
    now: DateTime<Utc>,
) -> Result<Todo> {
    sqlx::query_as::<_, TodoRow>(
        "UPDATE todos SET
           title = ?2, priority = ?3, due_date = ?4,
           planned_start = ?5, planned_end = ?6, actual_start = ?7, actual_end = ?8,
           waiting_on = ?9, follow_up_date = ?10, close_reason = ?11, memo_md = ?12,
           updated_at = ?13
         WHERE id = ?1
         RETURNING *",
    )
    .bind(todo.id)
    .bind(&todo.title)
    .bind(todo.priority.as_i64())
    .bind(date_param(todo.due_date))
    .bind(date_param(todo.planned_start))
    .bind(date_param(todo.planned_end))
    .bind(date_param(todo.actual_start))
    .bind(date_param(todo.actual_end))
    .bind(&todo.waiting_on)
    .bind(date_param(todo.follow_up_date))
    .bind(&todo.close_reason)
    .bind(&todo.memo_md)
    .bind(ts_param(now))
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_found(todo.id))?
    .into_todo()
}

/// Writes the result of a status transition.
pub async fn write_status(
    conn: &mut SqliteConnection,
    id: i64,
    fields: &StatusFields,
    now: DateTime<Utc>,
) -> Result<Todo> {
    sqlx::query_as::<_, TodoRow>(
        "UPDATE todos SET
           status = ?2, actual_start = ?3, actual_end = ?4,
           waiting_on = ?5, waiting_since = ?6, follow_up_date = ?7,
           closed_at = ?8, close_reason = ?9, updated_at = ?10
         WHERE id = ?1
         RETURNING *",
    )
    .bind(id)
    .bind(fields.status.as_str())
    .bind(date_param(fields.actual_start))
    .bind(date_param(fields.actual_end))
    .bind(&fields.waiting_on)
    .bind(date_param(fields.waiting_since))
    .bind(date_param(fields.follow_up_date))
    .bind(opt_ts_param(fields.closed_at))
    .bind(&fields.close_reason)
    .bind(ts_param(now))
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_found(id))?
    .into_todo()
}

/// Deletes a TODO with its work logs and events (ON DELETE CASCADE).
pub async fn delete(conn: &mut SqliteConnection, id: i64) -> Result<()> {
    let done = sqlx::query("DELETE FROM todos WHERE id = ?1")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// TODO counts per project and status (all projects, archived included).
pub async fn count_by_status(conn: &mut SqliteConnection) -> Result<Vec<StatusCount>> {
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT project_id, status, COUNT(*) FROM todos
         GROUP BY project_id, status
         ORDER BY project_id, status",
    )
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|(project_id, status, count)| {
            let status = Status::parse(&status).ok_or(RepoError::Corrupt {
                column: "todos.status",
                value: status,
            })?;
            Ok(StatusCount {
                project_id,
                status,
                count,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::EventKind;
    use crate::domain::status::{StatusChange, transition};
    use crate::domain::testing::{d, ts};
    use crate::repo::events as event_repo;
    use crate::repo::projects;
    use crate::repo::testing::{conn, project};

    fn now() -> DateTime<Utc> {
        ts(2026, 11, 19, 1, 0)
    }

    fn new_todo(project_id: i64, title: &str) -> NewTodo {
        NewTodo {
            project_id,
            parent_id: None,
            title: title.into(),
            priority: Priority::Medium,
            due_date: None,
            planned_start: None,
            planned_end: None,
            memo_md: String::new(),
        }
    }

    #[tokio::test]
    async fn insert_and_get_round_trip() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "基幹システム更改").await;
        let t = insert(
            &mut c,
            &NewTodo {
                priority: Priority::High,
                due_date: Some(d("2026-11-20")),
                planned_start: Some(d("2026-11-16")),
                planned_end: Some(d("2026-11-19")),
                memo_md: "# 概要".into(),
                ..new_todo(p.id, "API設計書のレビュー対応")
            },
            now(),
        )
        .await
        .unwrap();

        assert_eq!(t.status, Status::Todo);
        assert_eq!(t.priority, Priority::High);
        assert_eq!(t.parent_id, None);
        assert_eq!(t.due_date, Some(d("2026-11-20")));
        assert_eq!(t.actual_start, None);
        assert_eq!(t.created_at, now());
        assert_eq!(get(&mut c, t.id).await.unwrap(), t);

        let raw: (String, String, i64) =
            sqlx::query_as("SELECT due_date, status, priority FROM todos WHERE id = ?1")
                .bind(t.id)
                .fetch_one(&mut *c)
                .await
                .unwrap();
        assert_eq!(raw, ("2026-11-20".to_owned(), "todo".to_owned(), 1));
    }

    #[tokio::test]
    async fn sort_order_increases_within_a_project() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let q = project(&mut c, "Q").await;
        let a = insert(&mut c, &new_todo(p.id, "a"), now()).await.unwrap();
        let b = insert(&mut c, &new_todo(p.id, "b"), now()).await.unwrap();
        let x = insert(&mut c, &new_todo(q.id, "x"), now()).await.unwrap();
        assert_eq!((a.sort_order, b.sort_order, x.sort_order), (1, 2, 1));
    }

    #[tokio::test]
    async fn planned_start_after_end_violates_the_check_constraint() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let err = insert(
            &mut c,
            &NewTodo {
                planned_start: Some(d("2026-11-20")),
                planned_end: Some(d("2026-11-19")),
                ..new_todo(p.id, "逆転")
            },
            now(),
        )
        .await
        .unwrap_err();
        assert!(err.is_constraint_violation(), "{err:?}");

        // Same day is fine.
        insert(
            &mut c,
            &NewTodo {
                planned_start: Some(d("2026-11-20")),
                planned_end: Some(d("2026-11-20")),
                ..new_todo(p.id, "同日")
            },
            now(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn actual_start_after_end_violates_the_check_constraint() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let t = insert(&mut c, &new_todo(p.id, "t"), now()).await.unwrap();
        let fields = StatusFields {
            actual_start: Some(d("2026-11-20")),
            actual_end: Some(d("2026-11-19")),
            ..StatusFields::of(&t)
        };
        let err = write_status(&mut c, t.id, &fields, now())
            .await
            .unwrap_err();
        assert!(err.is_constraint_violation(), "{err:?}");

        let edited = Todo {
            actual_start: Some(d("2026-11-20")),
            actual_end: Some(d("2026-11-19")),
            ..t
        };
        let err = update_fields(&mut c, &edited, now()).await.unwrap_err();
        assert!(err.is_constraint_violation(), "{err:?}");
    }

    #[tokio::test]
    async fn invalid_status_violates_the_check_constraint() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let t = insert(&mut c, &new_todo(p.id, "t"), now()).await.unwrap();
        let err: RepoError = sqlx::query("UPDATE todos SET status = 'finished' WHERE id = ?1")
            .bind(t.id)
            .execute(&mut *c)
            .await
            .unwrap_err()
            .into();
        assert!(err.is_constraint_violation(), "{err:?}");
    }

    #[tokio::test]
    async fn unknown_project_violates_the_foreign_key() {
        let (_pool, mut c) = conn().await;
        let err = insert(&mut c, &new_todo(42, "どこにも属さない"), now())
            .await
            .unwrap_err();
        assert!(err.is_constraint_violation(), "{err:?}");
    }

    #[tokio::test]
    async fn corrupt_stored_values_are_reported() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let t = insert(&mut c, &new_todo(p.id, "t"), now()).await.unwrap();
        sqlx::query("UPDATE todos SET priority = 9 WHERE id = ?1")
            .bind(t.id)
            .execute(&mut *c)
            .await
            .unwrap();
        assert!(matches!(
            get(&mut c, t.id).await,
            Err(RepoError::Corrupt {
                column: "todos.priority",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn list_filters_by_project_status_and_archive() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let q = project(&mut c, "Q").await;
        let p1 = insert(&mut c, &new_todo(p.id, "p1"), now()).await.unwrap();
        let p2 = insert(&mut c, &new_todo(p.id, "p2"), now()).await.unwrap();
        let q1 = insert(&mut c, &new_todo(q.id, "q1"), now()).await.unwrap();
        let doing = StatusFields {
            status: Status::Doing,
            actual_start: Some(d("2026-11-19")),
            ..StatusFields::of(&p2)
        };
        write_status(&mut c, p2.id, &doing, now()).await.unwrap();

        let ids = |todos: Vec<Todo>| todos.into_iter().map(|t| t.id).collect::<Vec<_>>();

        let all = list(&mut c, &TodoFilter::default()).await.unwrap();
        assert_eq!(ids(all), vec![p1.id, p2.id, q1.id]);

        let only_p = TodoFilter {
            project_id: Some(p.id),
            ..TodoFilter::default()
        };
        assert_eq!(
            ids(list(&mut c, &only_p).await.unwrap()),
            vec![p1.id, p2.id]
        );

        let todo_only = TodoFilter {
            statuses: Some(vec![Status::Todo]),
            ..TodoFilter::default()
        };
        assert_eq!(
            ids(list(&mut c, &todo_only).await.unwrap()),
            vec![p1.id, q1.id]
        );

        let none = TodoFilter {
            statuses: Some(vec![]),
            ..TodoFilter::default()
        };
        assert!(list(&mut c, &none).await.unwrap().is_empty());

        projects::set_archived(&mut c, q.id, Some(now()), now())
            .await
            .unwrap();
        assert_eq!(
            ids(list(&mut c, &TodoFilter::default()).await.unwrap()),
            vec![p1.id, p2.id]
        );
        let with_archived = TodoFilter {
            include_archived_projects: true,
            ..TodoFilter::default()
        };
        assert_eq!(
            ids(list(&mut c, &with_archived).await.unwrap()),
            vec![p1.id, p2.id, q1.id]
        );
    }

    #[tokio::test]
    async fn update_fields_leaves_status_columns_alone() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let t = insert(&mut c, &new_todo(p.id, "元のタイトル"), now())
            .await
            .unwrap();
        let later = ts(2026, 11, 19, 2, 0);
        let edited = Todo {
            title: "新しいタイトル".into(),
            priority: Priority::Low,
            due_date: Some(d("2026-11-30")),
            planned_start: Some(d("2026-11-20")),
            planned_end: Some(d("2026-11-27")),
            memo_md: "- [ ] 確認".into(),
            // Status columns set here must not be written.
            status: Status::Done,
            closed_at: Some(later),
            waiting_since: Some(d("2026-11-19")),
            ..t.clone()
        };
        let saved = update_fields(&mut c, &edited, later).await.unwrap();
        assert_eq!(saved.title, "新しいタイトル");
        assert_eq!(saved.priority, Priority::Low);
        assert_eq!(saved.planned_end, Some(d("2026-11-27")));
        assert_eq!(saved.memo_md, "- [ ] 確認");
        assert_eq!(saved.updated_at, later);
        assert_eq!(saved.status, Status::Todo);
        assert_eq!(saved.closed_at, None);
        assert_eq!(saved.waiting_since, None);
        assert!(matches!(
            update_fields(&mut c, &Todo { id: 999, ..edited }, later).await,
            Err(RepoError::NotFound {
                entity: "todo",
                id: 999
            })
        ));
    }

    #[tokio::test]
    async fn status_change_and_event_commit_together() {
        let (pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let t = insert(&mut c, &new_todo(p.id, "t"), now()).await.unwrap();
        drop(c);

        let today = d("2026-11-19");
        let change = transition(
            &StatusFields::of(&t),
            &StatusChange::waiting("認証担当の回答", d("2026-11-20")),
            today,
            now(),
        )
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        let saved = write_status(&mut tx, t.id, &change.fields, now())
            .await
            .unwrap();
        event_repo::insert(&mut tx, t.id, &change.event)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let mut c = pool.acquire().await.unwrap();
        assert_eq!(get(&mut c, t.id).await.unwrap(), saved);
        assert_eq!(saved.status, Status::Waiting);
        assert_eq!(saved.waiting_on.as_deref(), Some("認証担当の回答"));
        assert_eq!(saved.waiting_since, Some(today));
        assert_eq!(saved.follow_up_date, Some(d("2026-11-20")));
        let events = event_repo::list_by_todo(&mut c, t.id).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Status);
        assert_eq!(events[0].note, change.event.note);
    }

    #[tokio::test]
    async fn rolled_back_status_change_leaves_no_trace() {
        let (pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let t = insert(&mut c, &new_todo(p.id, "t"), now()).await.unwrap();
        drop(c);

        let change = transition(
            &StatusFields::of(&t),
            &StatusChange::to(Status::Done),
            d("2026-11-19"),
            now(),
        )
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        write_status(&mut tx, t.id, &change.fields, now())
            .await
            .unwrap();
        event_repo::insert(&mut tx, t.id, &change.event)
            .await
            .unwrap();
        tx.rollback().await.unwrap();

        let mut c = pool.acquire().await.unwrap();
        assert_eq!(get(&mut c, t.id).await.unwrap(), t);
        assert!(
            event_repo::list_by_todo(&mut c, t.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn delete_todo_cascades_and_reports_missing() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let t = insert(&mut c, &new_todo(p.id, "t"), now()).await.unwrap();
        event_repo::insert(
            &mut c,
            t.id,
            &crate::domain::events::created(Status::Todo, d("2026-11-19"), now()),
        )
        .await
        .unwrap();
        delete(&mut c, t.id).await.unwrap();
        assert!(matches!(
            get(&mut c, t.id).await,
            Err(RepoError::NotFound { .. })
        ));
        assert!(
            event_repo::list_by_todo(&mut c, t.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            delete(&mut c, t.id).await,
            Err(RepoError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn counts_by_project_and_status() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let q = project(&mut c, "Q").await;
        let a = insert(&mut c, &new_todo(p.id, "a"), now()).await.unwrap();
        insert(&mut c, &new_todo(p.id, "b"), now()).await.unwrap();
        insert(&mut c, &new_todo(q.id, "c"), now()).await.unwrap();
        let cancelled = StatusFields {
            status: Status::Cancelled,
            closed_at: Some(now()),
            close_reason: Some("不要".into()),
            ..StatusFields::of(&a)
        };
        write_status(&mut c, a.id, &cancelled, now()).await.unwrap();

        let counts = count_by_status(&mut c).await.unwrap();
        assert_eq!(
            counts,
            vec![
                StatusCount {
                    project_id: p.id,
                    status: Status::Cancelled,
                    count: 1
                },
                StatusCount {
                    project_id: p.id,
                    status: Status::Todo,
                    count: 1
                },
                StatusCount {
                    project_id: q.id,
                    status: Status::Todo,
                    count: 1
                },
            ]
        );
    }

    #[tokio::test]
    async fn list_by_ids_keeps_the_given_order() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let a = insert(&mut c, &new_todo(p.id, "a"), now()).await.unwrap();
        let b = insert(&mut c, &new_todo(p.id, "b"), now()).await.unwrap();
        let c3 = insert(&mut c, &new_todo(p.id, "c"), now()).await.unwrap();

        let found = list_by_ids(&mut c, &[c3.id, a.id, 404, b.id])
            .await
            .unwrap();
        assert_eq!(found, vec![c3, a, b]);
        assert!(list_by_ids(&mut c, &[]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn children_and_parent_round_trip() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let parent = insert(&mut c, &new_todo(p.id, "親"), now()).await.unwrap();
        let other = insert(&mut c, &new_todo(p.id, "別"), now()).await.unwrap();
        let a = insert(
            &mut c,
            &NewTodo {
                parent_id: Some(parent.id),
                ..new_todo(p.id, "子a")
            },
            now(),
        )
        .await
        .unwrap();
        assert_eq!(a.parent_id, Some(parent.id));
        let b = insert(&mut c, &new_todo(p.id, "子b"), now()).await.unwrap();
        let later = ts(2026, 11, 20, 1, 0);
        let b = set_parent(&mut c, b.id, Some(parent.id), later)
            .await
            .unwrap();
        assert_eq!(b.parent_id, Some(parent.id));
        assert_eq!(b.updated_at, later);

        let ids = |v: Vec<Todo>| v.into_iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(
            ids(list_children(&mut c, &[parent.id]).await.unwrap()),
            vec![a.id, b.id]
        );
        assert!(list_children(&mut c, &[other.id]).await.unwrap().is_empty());
        assert!(list_children(&mut c, &[]).await.unwrap().is_empty());

        let a = set_parent(&mut c, a.id, None, later).await.unwrap();
        assert_eq!(a.parent_id, None);
        assert_eq!(
            ids(list_children(&mut c, &[parent.id, other.id]).await.unwrap()),
            vec![b.id]
        );
        assert!(matches!(
            set_parent(&mut c, 999, None, later).await,
            Err(RepoError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn deleting_a_parent_deletes_its_children() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let parent = insert(&mut c, &new_todo(p.id, "親"), now()).await.unwrap();
        let child = insert(
            &mut c,
            &NewTodo {
                parent_id: Some(parent.id),
                ..new_todo(p.id, "子")
            },
            now(),
        )
        .await
        .unwrap();
        delete(&mut c, parent.id).await.unwrap();
        assert!(matches!(
            get(&mut c, child.id).await,
            Err(RepoError::NotFound { .. })
        ));
    }
}
