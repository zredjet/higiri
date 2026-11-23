//! `projects` table.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};

use super::{RepoError, Result, opt_ts_param, read_opt_ts, read_ts, ts_param};
use crate::domain::model::Project;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProject {
    pub name: String,
    /// `None` uses the table default (`#3B6FD4`).
    pub color: Option<String>,
    pub description: String,
}

/// Editable fields of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEdit {
    pub name: String,
    pub color: String,
    pub description: String,
}

#[derive(FromRow)]
struct ProjectRow {
    id: i64,
    name: String,
    color: String,
    description: String,
    sort_order: i64,
    archived_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl ProjectRow {
    fn into_project(self) -> Result<Project> {
        Ok(Project {
            id: self.id,
            name: self.name,
            color: self.color,
            description: self.description,
            sort_order: self.sort_order,
            archived_at: read_opt_ts("projects.archived_at", self.archived_at)?,
            created_at: read_ts("projects.created_at", self.created_at)?,
            updated_at: read_ts("projects.updated_at", self.updated_at)?,
        })
    }
}

fn not_found(id: i64) -> RepoError {
    RepoError::NotFound {
        entity: "project",
        id,
    }
}

/// Adds a project at the end of the list.
pub async fn insert(
    conn: &mut SqliteConnection,
    new: &NewProject,
    now: DateTime<Utc>,
) -> Result<Project> {
    let row = match &new.color {
        Some(color) => {
            sqlx::query_as::<_, ProjectRow>(
                "INSERT INTO projects (name, color, description, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM projects), ?4, ?4)
                 RETURNING *",
            )
            .bind(&new.name)
            .bind(color)
            .bind(&new.description)
            .bind(ts_param(now))
            .fetch_one(&mut *conn)
            .await?
        }
        None => {
            sqlx::query_as::<_, ProjectRow>(
                "INSERT INTO projects (name, description, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM projects), ?3, ?3)
                 RETURNING *",
            )
            .bind(&new.name)
            .bind(&new.description)
            .bind(ts_param(now))
            .fetch_one(&mut *conn)
            .await?
        }
    };
    row.into_project()
}

pub async fn get(conn: &mut SqliteConnection, id: i64) -> Result<Project> {
    sqlx::query_as::<_, ProjectRow>("SELECT * FROM projects WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| not_found(id))?
        .into_project()
}

/// Projects in display order. Archived ones only when `include_archived`.
pub async fn list(conn: &mut SqliteConnection, include_archived: bool) -> Result<Vec<Project>> {
    sqlx::query_as::<_, ProjectRow>(
        "SELECT * FROM projects
         WHERE ?1 OR archived_at IS NULL
         ORDER BY sort_order, id",
    )
    .bind(include_archived)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(ProjectRow::into_project)
    .collect()
}

pub async fn update(
    conn: &mut SqliteConnection,
    id: i64,
    edit: &ProjectEdit,
    now: DateTime<Utc>,
) -> Result<Project> {
    sqlx::query_as::<_, ProjectRow>(
        "UPDATE projects SET name = ?2, color = ?3, description = ?4, updated_at = ?5
         WHERE id = ?1
         RETURNING *",
    )
    .bind(id)
    .bind(&edit.name)
    .bind(&edit.color)
    .bind(&edit.description)
    .bind(ts_param(now))
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_found(id))?
    .into_project()
}

/// Archives (`Some(when)`) or restores (`None`) a project.
pub async fn set_archived(
    conn: &mut SqliteConnection,
    id: i64,
    archived_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<Project> {
    sqlx::query_as::<_, ProjectRow>(
        "UPDATE projects SET archived_at = ?2, updated_at = ?3
         WHERE id = ?1
         RETURNING *",
    )
    .bind(id)
    .bind(opt_ts_param(archived_at))
    .bind(ts_param(now))
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_found(id))?
    .into_project()
}

/// Writes a project's place in the display order (v0.6.0). `updated_at` stays:
/// the order is not part of the project.
pub async fn set_sort_order(conn: &mut SqliteConnection, id: i64, sort_order: i64) -> Result<()> {
    let done = sqlx::query("UPDATE projects SET sort_order = ?2 WHERE id = ?1")
        .bind(id)
        .bind(sort_order)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// Deletes a project. Its TODOs, work logs and events go with it (ON DELETE CASCADE).
pub async fn delete(conn: &mut SqliteConnection, id: i64) -> Result<()> {
    let done = sqlx::query("DELETE FROM projects WHERE id = ?1")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events;
    use crate::domain::model::{Priority, Status};
    use crate::domain::testing::{d, ts};
    use crate::repo::events as event_repo;
    use crate::repo::testing::{conn, project};
    use crate::repo::todos::{self, NewTodo};
    use crate::repo::work_logs::{self, NewWorkLog};

    async fn count(conn: &mut SqliteConnection, sql: &'static str) -> i64 {
        let row: (i64,) = sqlx::query_as(sql).fetch_one(&mut *conn).await.unwrap();
        row.0
    }

    #[tokio::test]
    async fn insert_uses_default_color_and_appends() {
        let (_pool, mut c) = conn().await;
        let a = project(&mut c, "基幹システム更改").await;
        let b = insert(
            &mut c,
            &NewProject {
                name: "社内ツール整備".into(),
                color: Some("#2E8B57".into()),
                description: "説明".into(),
            },
            ts(2026, 11, 2, 0, 0),
        )
        .await
        .unwrap();
        assert_eq!(a.color, "#3B6FD4");
        assert_eq!(b.color, "#2E8B57");
        assert_eq!(b.description, "説明");
        assert!(b.sort_order > a.sort_order);
        assert_eq!(a.archived_at, None);

        let raw: (String,) = sqlx::query_as("SELECT created_at FROM projects WHERE id = ?1")
            .bind(b.id)
            .fetch_one(&mut *c)
            .await
            .unwrap();
        assert_eq!(raw.0, "2026-11-02T00:00:00.000Z");
    }

    #[tokio::test]
    async fn get_update_and_not_found() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "採用サイト改修").await;
        assert_eq!(get(&mut c, p.id).await.unwrap(), p);

        let later = ts(2026, 11, 19, 3, 0);
        let edited = update(
            &mut c,
            p.id,
            &ProjectEdit {
                name: "採用サイト刷新".into(),
                color: "#C0392B".into(),
                description: "2027年春公開".into(),
            },
            later,
        )
        .await
        .unwrap();
        assert_eq!(edited.name, "採用サイト刷新");
        assert_eq!(edited.color, "#C0392B");
        assert_eq!(edited.updated_at, later);
        assert_eq!(edited.created_at, p.created_at);

        assert!(matches!(
            get(&mut c, 999).await,
            Err(RepoError::NotFound {
                entity: "project",
                id: 999
            })
        ));
        let edit = ProjectEdit {
            name: "x".into(),
            color: "#000000".into(),
            description: String::new(),
        };
        assert!(matches!(
            update(&mut c, 999, &edit, later).await,
            Err(RepoError::NotFound { .. })
        ));
        assert!(matches!(
            delete(&mut c, 999).await,
            Err(RepoError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn archived_projects_are_hidden_until_restored() {
        let (_pool, mut c) = conn().await;
        let a = project(&mut c, "A").await;
        let b = project(&mut c, "B").await;
        let when = ts(2026, 11, 19, 0, 0);
        let archived = set_archived(&mut c, a.id, Some(when), when).await.unwrap();
        assert_eq!(archived.archived_at, Some(when));

        let active: Vec<_> = list(&mut c, false)
            .await
            .unwrap()
            .iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(active, vec![b.id]);
        let all: Vec<_> = list(&mut c, true)
            .await
            .unwrap()
            .iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(all, vec![a.id, b.id]);

        set_archived(&mut c, a.id, None, when).await.unwrap();
        assert_eq!(list(&mut c, false).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn delete_removes_todos_logs_and_events() {
        let (_pool, mut c) = conn().await;
        let keep = project(&mut c, "残す").await;
        let gone = project(&mut c, "消す").await;
        let now = ts(2026, 11, 19, 0, 0);
        for p in [&keep, &gone] {
            let t = todos::insert(
                &mut c,
                &NewTodo {
                    project_id: p.id,
                    parent_id: None,
                    title: "作業".into(),
                    priority: Priority::Medium,
                    due_date: None,
                    planned_start: None,
                    planned_end: None,
                    memo_md: String::new(),
                },
                now,
            )
            .await
            .unwrap();
            work_logs::insert(
                &mut c,
                &NewWorkLog {
                    todo_id: t.id,
                    log_date: d("2026-11-19"),
                    body_md: "- 進めた".into(),
                    minutes: Some(30),
                },
                now,
            )
            .await
            .unwrap();
            event_repo::insert(
                &mut c,
                t.id,
                &events::created(Status::Todo, d("2026-11-19"), now),
            )
            .await
            .unwrap();
        }
        assert_eq!(count(&mut c, "SELECT COUNT(*) FROM todos").await, 2);

        delete(&mut c, gone.id).await.unwrap();

        assert_eq!(count(&mut c, "SELECT COUNT(*) FROM projects").await, 1);
        assert_eq!(count(&mut c, "SELECT COUNT(*) FROM todos").await, 1);
        assert_eq!(count(&mut c, "SELECT COUNT(*) FROM work_logs").await, 1);
        assert_eq!(count(&mut c, "SELECT COUNT(*) FROM todo_events").await, 1);
        let left: (i64,) = sqlx::query_as("SELECT project_id FROM todos")
            .fetch_one(&mut *c)
            .await
            .unwrap();
        assert_eq!(left.0, keep.id);
    }
}
