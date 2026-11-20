//! `work_logs` table.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, SqliteConnection};

use super::{RepoError, Result, read_required_date, read_ts, ts_param};
use crate::domain::date::format_date;
use crate::domain::model::WorkLog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorkLog {
    pub todo_id: i64,
    pub log_date: NaiveDate,
    pub body_md: String,
    pub minutes: Option<i64>,
}

/// Editable fields of a work log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkLogEdit {
    pub log_date: NaiveDate,
    pub body_md: String,
    pub minutes: Option<i64>,
}

#[derive(FromRow)]
struct WorkLogRow {
    id: i64,
    todo_id: i64,
    log_date: String,
    body_md: String,
    minutes: Option<i64>,
    created_at: String,
    updated_at: String,
}

impl WorkLogRow {
    fn into_log(self) -> Result<WorkLog> {
        Ok(WorkLog {
            id: self.id,
            todo_id: self.todo_id,
            log_date: read_required_date("work_logs.log_date", self.log_date)?,
            body_md: self.body_md,
            minutes: self.minutes,
            created_at: read_ts("work_logs.created_at", self.created_at)?,
            updated_at: read_ts("work_logs.updated_at", self.updated_at)?,
        })
    }
}

fn not_found(id: i64) -> RepoError {
    RepoError::NotFound {
        entity: "work_log",
        id,
    }
}

fn collect(rows: Vec<WorkLogRow>) -> Result<Vec<WorkLog>> {
    rows.into_iter().map(WorkLogRow::into_log).collect()
}

pub async fn insert(
    conn: &mut SqliteConnection,
    new: &NewWorkLog,
    now: DateTime<Utc>,
) -> Result<WorkLog> {
    sqlx::query_as::<_, WorkLogRow>(
        "INSERT INTO work_logs (todo_id, log_date, body_md, minutes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         RETURNING *",
    )
    .bind(new.todo_id)
    .bind(format_date(new.log_date))
    .bind(&new.body_md)
    .bind(new.minutes)
    .bind(ts_param(now))
    .fetch_one(&mut *conn)
    .await?
    .into_log()
}

pub async fn get(conn: &mut SqliteConnection, id: i64) -> Result<WorkLog> {
    sqlx::query_as::<_, WorkLogRow>("SELECT * FROM work_logs WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| not_found(id))?
        .into_log()
}

pub async fn update(
    conn: &mut SqliteConnection,
    id: i64,
    edit: &WorkLogEdit,
    now: DateTime<Utc>,
) -> Result<WorkLog> {
    sqlx::query_as::<_, WorkLogRow>(
        "UPDATE work_logs SET log_date = ?2, body_md = ?3, minutes = ?4, updated_at = ?5
         WHERE id = ?1
         RETURNING *",
    )
    .bind(id)
    .bind(format_date(edit.log_date))
    .bind(&edit.body_md)
    .bind(edit.minutes)
    .bind(ts_param(now))
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_found(id))?
    .into_log()
}

pub async fn delete(conn: &mut SqliteConnection, id: i64) -> Result<()> {
    let done = sqlx::query("DELETE FROM work_logs WHERE id = ?1")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// Logs of one TODO, oldest first.
pub async fn list_by_todo(conn: &mut SqliteConnection, todo_id: i64) -> Result<Vec<WorkLog>> {
    collect(
        sqlx::query_as::<_, WorkLogRow>(
            "SELECT * FROM work_logs WHERE todo_id = ?1 ORDER BY log_date, id",
        )
        .bind(todo_id)
        .fetch_all(&mut *conn)
        .await?,
    )
}

/// Logs of these TODOs, by TODO and then oldest first.
pub async fn list_by_todos(conn: &mut SqliteConnection, todo_ids: &[i64]) -> Result<Vec<WorkLog>> {
    if todo_ids.is_empty() {
        return Ok(Vec::new());
    }
    collect(
        sqlx::query_as::<_, WorkLogRow>(
            "SELECT * FROM work_logs WHERE todo_id IN (SELECT value FROM json_each(?1))
             ORDER BY todo_id, log_date, id",
        )
        .bind(serde_json::Value::from(todo_ids.to_vec()).to_string())
        .fetch_all(&mut *conn)
        .await?,
    )
}

/// Every log (checking that imported data can be read).
pub async fn list_all(conn: &mut SqliteConnection) -> Result<Vec<WorkLog>> {
    collect(
        sqlx::query_as::<_, WorkLogRow>("SELECT * FROM work_logs ORDER BY id")
            .fetch_all(&mut *conn)
            .await?,
    )
}

/// Logs written for one day (daily report material).
pub async fn list_by_date(conn: &mut SqliteConnection, date: NaiveDate) -> Result<Vec<WorkLog>> {
    collect(
        sqlx::query_as::<_, WorkLogRow>(
            "SELECT * FROM work_logs WHERE log_date = ?1 ORDER BY todo_id, id",
        )
        .bind(format_date(date))
        .fetch_all(&mut *conn)
        .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Priority;
    use crate::domain::testing::{d, ts};
    use crate::repo::testing::{conn, project};
    use crate::repo::todos::{self, NewTodo};

    async fn todo_id(c: &mut SqliteConnection) -> i64 {
        let p = project(c, "P").await;
        todos::insert(
            c,
            &NewTodo {
                project_id: p.id,
                parent_id: None,
                title: "t".into(),
                priority: Priority::Medium,
                due_date: None,
                planned_start: None,
                planned_end: None,
                memo_md: String::new(),
            },
            ts(2026, 11, 1, 0, 0),
        )
        .await
        .unwrap()
        .id
    }

    fn log(todo_id: i64, date: &str, body: &str, minutes: Option<i64>) -> NewWorkLog {
        NewWorkLog {
            todo_id,
            log_date: d(date),
            body_md: body.into(),
            minutes,
        }
    }

    #[tokio::test]
    async fn insert_update_delete() {
        let (_pool, mut c) = conn().await;
        let tid = todo_id(&mut c).await;
        let now = ts(2026, 11, 18, 9, 0);
        let l = insert(
            &mut c,
            &log(tid, "2026-11-18", "レビュー指摘を整理", Some(120)),
            now,
        )
        .await
        .unwrap();
        assert_eq!(l.minutes, Some(120));
        assert_eq!(get(&mut c, l.id).await.unwrap(), l);

        let later = ts(2026, 11, 19, 9, 0);
        let edited = update(
            &mut c,
            l.id,
            &WorkLogEdit {
                log_date: d("2026-11-19"),
                body_md: "- 指摘12件のうち9件を反映".into(),
                minutes: None,
            },
            later,
        )
        .await
        .unwrap();
        assert_eq!(edited.log_date, d("2026-11-19"));
        assert_eq!(edited.minutes, None);
        assert_eq!(edited.created_at, now);
        assert_eq!(edited.updated_at, later);

        delete(&mut c, l.id).await.unwrap();
        assert!(matches!(
            get(&mut c, l.id).await,
            Err(RepoError::NotFound { .. })
        ));
        assert!(matches!(
            delete(&mut c, l.id).await,
            Err(RepoError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn lists_by_todo_and_by_date() {
        let (_pool, mut c) = conn().await;
        let tid = todo_id(&mut c).await;
        let now = ts(2026, 11, 19, 9, 0);
        let b = insert(&mut c, &log(tid, "2026-11-19", "b", None), now)
            .await
            .unwrap();
        let a = insert(&mut c, &log(tid, "2026-11-18", "a", Some(30)), now)
            .await
            .unwrap();
        let c2 = insert(&mut c, &log(tid, "2026-11-19", "c", Some(15)), now)
            .await
            .unwrap();

        let by_todo: Vec<_> = list_by_todo(&mut c, tid)
            .await
            .unwrap()
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(by_todo, vec![a.id, b.id, c2.id]);

        let by_date: Vec<_> = list_by_date(&mut c, d("2026-11-19"))
            .await
            .unwrap()
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(by_date, vec![b.id, c2.id]);
        assert!(
            list_by_date(&mut c, d("2026-11-20"))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn log_for_unknown_todo_is_rejected() {
        let (_pool, mut c) = conn().await;
        let err = insert(
            &mut c,
            &log(404, "2026-11-19", "x", None),
            ts(2026, 11, 19, 0, 0),
        )
        .await
        .unwrap_err();
        assert!(err.is_constraint_violation(), "{err:?}");
    }

    #[tokio::test]
    async fn lists_the_logs_of_several_todos() {
        let (_pool, mut c) = conn().await;
        let t1 = todo_id(&mut c).await;
        let t2 = todo_id(&mut c).await;
        let t3 = todo_id(&mut c).await;
        let now = ts(2026, 11, 19, 0, 0);
        let a = insert(&mut c, &log(t2, "2026-11-19", "a", None), now)
            .await
            .unwrap();
        let b = insert(&mut c, &log(t1, "2026-11-19", "b", None), now)
            .await
            .unwrap();
        let c2 = insert(&mut c, &log(t1, "2026-11-18", "c", None), now)
            .await
            .unwrap();
        insert(&mut c, &log(t3, "2026-11-18", "d", None), now)
            .await
            .unwrap();

        let ids: Vec<i64> = list_by_todos(&mut c, &[t2, t1])
            .await
            .unwrap()
            .iter()
            .map(|l| l.id)
            .collect();
        assert_eq!(ids, vec![c2.id, b.id, a.id]);
        assert!(list_by_todos(&mut c, &[]).await.unwrap().is_empty());
    }
}
