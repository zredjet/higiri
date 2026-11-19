//! `todo_events` table (change history).

use chrono::NaiveDate;
use sqlx::{FromRow, SqliteConnection};

use super::{RepoError, Result, read_required_date, read_ts, ts_param};
use crate::domain::date::format_date;
use crate::domain::events::{EventKind, EventNote, NewEvent, TodoEvent};

#[derive(FromRow)]
struct EventRow {
    id: i64,
    todo_id: i64,
    event_date: String,
    kind: String,
    old_value: Option<String>,
    new_value: Option<String>,
    note: Option<String>,
    occurred_at: String,
}

impl EventRow {
    fn into_event(self) -> Result<TodoEvent> {
        let kind = EventKind::parse(&self.kind).ok_or(RepoError::Corrupt {
            column: "todo_events.kind",
            value: self.kind.clone(),
        })?;
        let note = match self.note {
            None => EventNote::default(),
            Some(s) => EventNote::from_json(&s).ok_or(RepoError::Corrupt {
                column: "todo_events.note",
                value: s,
            })?,
        };
        Ok(TodoEvent {
            id: self.id,
            todo_id: self.todo_id,
            event_date: read_required_date("todo_events.event_date", self.event_date)?,
            kind,
            old_value: self.old_value,
            new_value: self.new_value,
            note,
            occurred_at: read_ts("todo_events.occurred_at", self.occurred_at)?,
        })
    }
}

fn collect(rows: Vec<EventRow>) -> Result<Vec<TodoEvent>> {
    rows.into_iter().map(EventRow::into_event).collect()
}

/// Records one event. `note` is always stored as a JSON object.
pub async fn insert(
    conn: &mut SqliteConnection,
    todo_id: i64,
    event: &NewEvent,
) -> Result<TodoEvent> {
    sqlx::query_as::<_, EventRow>(
        "INSERT INTO todo_events (todo_id, event_date, kind, old_value, new_value, note, occurred_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         RETURNING *",
    )
    .bind(todo_id)
    .bind(format_date(event.event_date))
    .bind(event.kind.as_str())
    .bind(&event.old_value)
    .bind(&event.new_value)
    .bind(event.note.to_json())
    .bind(ts_param(event.occurred_at))
    .fetch_one(&mut *conn)
    .await?
    .into_event()
}

/// Events of one TODO, oldest first.
pub async fn list_by_todo(conn: &mut SqliteConnection, todo_id: i64) -> Result<Vec<TodoEvent>> {
    collect(
        sqlx::query_as::<_, EventRow>(
            "SELECT * FROM todo_events WHERE todo_id = ?1 ORDER BY occurred_at, id",
        )
        .bind(todo_id)
        .fetch_all(&mut *conn)
        .await?,
    )
}

/// Events that happened on one day (daily report material).
pub async fn list_by_date(conn: &mut SqliteConnection, date: NaiveDate) -> Result<Vec<TodoEvent>> {
    collect(
        sqlx::query_as::<_, EventRow>(
            "SELECT * FROM todo_events WHERE event_date = ?1 ORDER BY occurred_at, id",
        )
        .bind(format_date(date))
        .fetch_all(&mut *conn)
        .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::{self, Period};
    use crate::domain::model::{Priority, Status, Todo};
    use crate::domain::status::{StatusChange, StatusFields, transition};
    use crate::domain::testing::{d, ts};
    use crate::repo::testing::{conn, project};
    use crate::repo::todos::{self, NewTodo};

    async fn new_todo(c: &mut SqliteConnection) -> Todo {
        let p = project(c, "P").await;
        todos::insert(
            c,
            &NewTodo {
                project_id: p.id,
                title: "t".into(),
                priority: Priority::Medium,
                due_date: Some(d("2026-11-20")),
                planned_start: None,
                planned_end: None,
                memo_md: String::new(),
            },
            ts(2026, 11, 17, 0, 0),
        )
        .await
        .unwrap()
    }

    async fn raw(
        c: &mut SqliteConnection,
        id: i64,
    ) -> (String, Option<String>, Option<String>, String) {
        sqlx::query_as("SELECT kind, old_value, new_value, note FROM todo_events WHERE id = ?1")
            .bind(id)
            .fetch_one(&mut *c)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn status_event_stores_plain_names_and_json_note() {
        let (_pool, mut c) = conn().await;
        let t = new_todo(&mut c).await;
        let tr = transition(
            &StatusFields::of(&t),
            &StatusChange::to(Status::Doing),
            d("2026-11-17"),
            ts(2026, 11, 17, 0, 30),
        )
        .unwrap();
        let e = insert(&mut c, t.id, &tr.event).await.unwrap();

        assert_eq!(
            raw(&mut c, e.id).await,
            (
                "status".to_owned(),
                Some("todo".to_owned()),
                Some("doing".to_owned()),
                r#"{"auto":{"actual_start":"2026-11-17"}}"#.to_owned()
            )
        );
        assert_eq!(e.note, tr.event.note);
        assert_eq!(e.event_date, d("2026-11-17"));
    }

    #[tokio::test]
    async fn every_kind_stores_a_json_note() {
        let (_pool, mut c) = conn().await;
        let t = new_todo(&mut c).await;
        let now = ts(2026, 11, 19, 1, 0);
        let created = insert(
            &mut c,
            t.id,
            &events::created(Status::Todo, d("2026-11-17"), now),
        )
        .await
        .unwrap();
        let after = Todo {
            due_date: Some(d("2026-11-25")),
            planned_start: Some(d("2026-11-20")),
            planned_end: Some(d("2026-11-24")),
            ..t.clone()
        };
        let mut ids = vec![created.id];
        for ev in events::edit_events(&t, &after, d("2026-11-19"), now) {
            ids.push(insert(&mut c, t.id, &ev).await.unwrap().id);
        }
        assert_eq!(ids.len(), 3);
        for id in &ids {
            let (_, _, _, note) = raw(&mut c, *id).await;
            assert_eq!(note, "{}");
        }
        let (kind, old, new, _) = raw(&mut c, ids[2]).await;
        assert_eq!(kind, "planned");
        assert_eq!(
            Period::from_json(old.as_deref().unwrap()),
            Some(Period {
                start: None,
                end: None
            })
        );
        assert_eq!(
            new.as_deref(),
            Some(r#"{"start":"2026-11-20","end":"2026-11-24"}"#)
        );
    }

    #[tokio::test]
    async fn lists_by_todo_and_by_date_in_time_order() {
        let (_pool, mut c) = conn().await;
        let t = new_todo(&mut c).await;
        let later = insert(
            &mut c,
            t.id,
            &events::created(Status::Todo, d("2026-11-19"), ts(2026, 11, 19, 5, 0)),
        )
        .await
        .unwrap();
        let earlier = insert(
            &mut c,
            t.id,
            &events::created(Status::Todo, d("2026-11-17"), ts(2026, 11, 17, 5, 0)),
        )
        .await
        .unwrap();
        let by_todo: Vec<_> = list_by_todo(&mut c, t.id)
            .await
            .unwrap()
            .iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(by_todo, vec![earlier.id, later.id]);
        let by_date: Vec<_> = list_by_date(&mut c, d("2026-11-19"))
            .await
            .unwrap()
            .iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(by_date, vec![later.id]);
    }

    #[tokio::test]
    async fn unreadable_note_is_reported() {
        let (_pool, mut c) = conn().await;
        let t = new_todo(&mut c).await;
        let e = insert(
            &mut c,
            t.id,
            &events::created(Status::Todo, d("2026-11-17"), ts(2026, 11, 17, 0, 0)),
        )
        .await
        .unwrap();
        sqlx::query("UPDATE todo_events SET note = '相手:人事部' WHERE id = ?1")
            .bind(e.id)
            .execute(&mut *c)
            .await
            .unwrap();
        assert!(matches!(
            list_by_todo(&mut c, t.id).await,
            Err(RepoError::Corrupt {
                column: "todo_events.note",
                ..
            })
        ));
    }
}
