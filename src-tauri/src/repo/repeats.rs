//! `todo_repeats` table (繰り返しのTODO, v0.3.0). One row per repeating
//! TODO: only the current round has one.
//!
//! `rule` is stored as JSON ([`RepeatRule`]). A rule that cannot be read (or
//! is out of range) is an error, like any other stored value. Deleting the
//! TODO deletes its row (ON DELETE CASCADE).

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, SqliteConnection};

use super::{RepoError, Result, date_param, read_date, read_ts, ts_param};
use crate::domain::repeat::{RepeatRule, TodoRepeat};

#[derive(FromRow)]
struct RepeatRow {
    todo_id: i64,
    rule: String,
    until_date: Option<String>,
    created_at: String,
    updated_at: String,
}

impl RepeatRow {
    fn into_repeat(self) -> Result<TodoRepeat> {
        let rule = RepeatRule::from_json(&self.rule).ok_or(RepoError::Corrupt {
            column: "todo_repeats.rule",
            value: self.rule.clone(),
        })?;
        Ok(TodoRepeat {
            todo_id: self.todo_id,
            rule,
            until_date: read_date("todo_repeats.until_date", self.until_date)?,
            created_at: read_ts("todo_repeats.created_at", self.created_at)?,
            updated_at: read_ts("todo_repeats.updated_at", self.updated_at)?,
        })
    }
}

fn not_found(todo_id: i64) -> RepoError {
    RepoError::NotFound {
        entity: "todo repeat",
        id: todo_id,
    }
}

/// Every repetition, by TODO.
pub async fn list(conn: &mut SqliteConnection) -> Result<Vec<TodoRepeat>> {
    sqlx::query_as::<_, RepeatRow>("SELECT * FROM todo_repeats ORDER BY todo_id")
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(RepeatRow::into_repeat)
        .collect()
}

/// The repetition of a TODO, if it has one.
pub async fn get(conn: &mut SqliteConnection, todo_id: i64) -> Result<Option<TodoRepeat>> {
    sqlx::query_as::<_, RepeatRow>("SELECT * FROM todo_repeats WHERE todo_id = ?1")
        .bind(todo_id)
        .fetch_optional(&mut *conn)
        .await?
        .map(RepeatRow::into_repeat)
        .transpose()
}

/// Gives a TODO a repetition. The database refuses a second one for the same
/// TODO (PRIMARY KEY) and a TODO that does not exist (FOREIGN KEY).
pub async fn insert(
    conn: &mut SqliteConnection,
    todo_id: i64,
    rule: &RepeatRule,
    until_date: Option<NaiveDate>,
    now: DateTime<Utc>,
) -> Result<TodoRepeat> {
    sqlx::query_as::<_, RepeatRow>(
        "INSERT INTO todo_repeats (todo_id, rule, until_date, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         RETURNING *",
    )
    .bind(todo_id)
    .bind(rule.to_json())
    .bind(date_param(until_date))
    .bind(ts_param(now))
    .fetch_one(&mut *conn)
    .await?
    .into_repeat()
}

/// Replaces the rule and the end of a TODO's repetition.
pub async fn update(
    conn: &mut SqliteConnection,
    todo_id: i64,
    rule: &RepeatRule,
    until_date: Option<NaiveDate>,
    now: DateTime<Utc>,
) -> Result<TodoRepeat> {
    sqlx::query_as::<_, RepeatRow>(
        "UPDATE todo_repeats SET rule = ?2, until_date = ?3, updated_at = ?4
         WHERE todo_id = ?1
         RETURNING *",
    )
    .bind(todo_id)
    .bind(rule.to_json())
    .bind(date_param(until_date))
    .bind(ts_param(now))
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_found(todo_id))?
    .into_repeat()
}

/// Removes a TODO's repetition.
pub async fn delete(conn: &mut SqliteConnection, todo_id: i64) -> Result<()> {
    let done = sqlx::query("DELETE FROM todo_repeats WHERE todo_id = ?1")
        .bind(todo_id)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() == 0 {
        return Err(not_found(todo_id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repeat::DayOfWeek;
    use crate::domain::testing::{d, ts};
    use crate::repo::testing::{conn, project, todo};
    use crate::repo::{projects, todos};

    fn weekly() -> RepeatRule {
        RepeatRule::Weekly {
            weekdays: vec![DayOfWeek::Mon, DayOfWeek::Thu],
        }
    }

    #[tokio::test]
    async fn insert_get_list_update_delete() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let a = todo(&mut c, p.id, "a").await;
        let b = todo(&mut c, p.id, "b").await;
        assert_eq!(get(&mut c, a.id).await.unwrap(), None);

        let rb = insert(
            &mut c,
            b.id,
            &RepeatRule::Daily,
            None,
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();
        let ra = insert(
            &mut c,
            a.id,
            &weekly(),
            Some(d("2027-03-31")),
            ts(2026, 11, 19, 2, 0),
        )
        .await
        .unwrap();
        assert_eq!(ra.rule, weekly());
        assert_eq!(ra.until_date, Some(d("2027-03-31")));
        assert_eq!(get(&mut c, a.id).await.unwrap(), Some(ra.clone()));
        assert_eq!(list(&mut c).await.unwrap(), vec![ra.clone(), rb.clone()]);

        let changed = update(
            &mut c,
            a.id,
            &RepeatRule::MonthlyWeekday {
                week: -1,
                weekday: DayOfWeek::Fri,
            },
            None,
            ts(2026, 11, 20, 0, 0),
        )
        .await
        .unwrap();
        assert_eq!(changed.until_date, None);
        assert_eq!(changed.created_at, ra.created_at);
        assert_eq!(changed.updated_at, ts(2026, 11, 20, 0, 0));
        let stored: (String,) = sqlx::query_as("SELECT rule FROM todo_repeats WHERE todo_id = ?1")
            .bind(a.id)
            .fetch_one(&mut *c)
            .await
            .unwrap();
        assert_eq!(
            stored.0,
            r#"{"kind":"monthly_weekday","week":-1,"weekday":"fri"}"#
        );

        delete(&mut c, b.id).await.unwrap();
        assert_eq!(get(&mut c, b.id).await.unwrap(), None);
        assert!(matches!(
            delete(&mut c, b.id).await,
            Err(RepoError::NotFound { .. })
        ));
        assert!(matches!(
            update(
                &mut c,
                b.id,
                &RepeatRule::Daily,
                None,
                ts(2026, 11, 20, 0, 0)
            )
            .await,
            Err(RepoError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn one_per_todo_and_only_for_todos_that_exist() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let a = todo(&mut c, p.id, "a").await;
        insert(
            &mut c,
            a.id,
            &RepeatRule::Daily,
            None,
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();
        for todo_id in [a.id, 999] {
            let err = insert(&mut c, todo_id, &weekly(), None, ts(2026, 11, 19, 2, 0))
                .await
                .unwrap_err();
            assert!(err.is_constraint_violation(), "{todo_id}: {err}");
        }
    }

    #[tokio::test]
    async fn deleting_the_todo_or_its_project_deletes_the_repeat() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let q = project(&mut c, "Q").await;
        let a = todo(&mut c, p.id, "a").await;
        let b1 = todo(&mut c, q.id, "b1").await;
        let b2 = todo(&mut c, q.id, "b2").await;
        for t in [&a, &b1, &b2] {
            insert(
                &mut c,
                t.id,
                &RepeatRule::Daily,
                None,
                ts(2026, 11, 19, 1, 0),
            )
            .await
            .unwrap();
        }
        todos::delete(&mut c, a.id).await.unwrap();
        assert_eq!(get(&mut c, a.id).await.unwrap(), None);
        projects::delete(&mut c, q.id).await.unwrap();
        assert_eq!(list(&mut c).await.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn an_unreadable_rule_or_date_is_an_error() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        for (rule, until, column) in [
            ("not json", None, "todo_repeats.rule"),
            (r#"{"kind":"hourly"}"#, None, "todo_repeats.rule"),
            // Out of range.
            (
                r#"{"kind":"every_days","days":1}"#,
                None,
                "todo_repeats.rule",
            ),
            (
                r#"{"kind":"weekly","weekdays":[]}"#,
                None,
                "todo_repeats.rule",
            ),
            (
                r#"{"kind":"daily"}"#,
                Some("2027/03/31"),
                "todo_repeats.until_date",
            ),
        ] {
            let t = todo(&mut c, p.id, "t").await;
            sqlx::query(
                "INSERT INTO todo_repeats (todo_id, rule, until_date, created_at, updated_at)
                 VALUES (?1, ?2, ?3, '2026-11-19T00:00:00.000Z', '2026-11-19T00:00:00.000Z')",
            )
            .bind(t.id)
            .bind(rule)
            .bind(until)
            .execute(&mut *c)
            .await
            .unwrap();
            let err = get(&mut c, t.id).await.unwrap_err();
            assert!(
                matches!(&err, RepoError::Corrupt { column: col, .. } if *col == column),
                "{rule}: {err}"
            );
        }
        assert!(list(&mut c).await.is_err());
    }
}
