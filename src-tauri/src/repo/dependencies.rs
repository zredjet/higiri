//! `todo_dependencies` table (TODO間の依存関係, v0.3.0). One row per pair:
//! the TODO `todo_id` cannot be started until `depends_on_id` is finished.
//!
//! The table refuses a pair twice (PRIMARY KEY) and a TODO waiting for itself
//! (CHECK). Circles are not the table's business: the app refuses them
//! (`domain::dependency::check_new`). Deleting either TODO deletes the row
//! (ON DELETE CASCADE).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};

use super::{RepoError, Result, read_date, read_ts, ts_param};
use crate::domain::dependency::Prerequisite;
use crate::domain::model::{Dependency, Status};

#[derive(FromRow)]
struct DependencyRow {
    todo_id: i64,
    depends_on_id: i64,
    created_at: String,
}

impl DependencyRow {
    fn into_dependency(self) -> Result<Dependency> {
        Ok(Dependency {
            todo_id: self.todo_id,
            depends_on_id: self.depends_on_id,
            created_at: read_ts("todo_dependencies.created_at", self.created_at)?,
        })
    }
}

fn collect(rows: Vec<DependencyRow>) -> Result<Vec<Dependency>> {
    rows.into_iter()
        .map(DependencyRow::into_dependency)
        .collect()
}

/// Every dependency, by (todo_id, depends_on_id).
pub async fn list(conn: &mut SqliteConnection) -> Result<Vec<Dependency>> {
    collect(
        sqlx::query_as::<_, DependencyRow>(
            "SELECT * FROM todo_dependencies ORDER BY todo_id, depends_on_id",
        )
        .fetch_all(&mut *conn)
        .await?,
    )
}

/// The dependencies whose both TODOs are among `ids` (a parent's children:
/// a template keeps them, a repeated round copies them).
pub async fn among(conn: &mut SqliteConnection, ids: &[i64]) -> Result<Vec<Dependency>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids = serde_json::Value::from(ids.to_vec()).to_string();
    collect(
        sqlx::query_as::<_, DependencyRow>(
            "SELECT * FROM todo_dependencies
             WHERE todo_id IN (SELECT value FROM json_each(?1))
               AND depends_on_id IN (SELECT value FROM json_each(?1))
             ORDER BY todo_id, depends_on_id",
        )
        .bind(ids)
        .fetch_all(&mut *conn)
        .await?,
    )
}

/// The TODOs `todo_id` waits for (`depends_on`) and those waiting for it
/// (`dependents`), each in the order the dependencies were added.
pub async fn neighbours(conn: &mut SqliteConnection, todo_id: i64) -> Result<(Vec<i64>, Vec<i64>)> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT todo_id, depends_on_id FROM todo_dependencies
         WHERE todo_id = ?1 OR depends_on_id = ?1
         ORDER BY created_at, todo_id, depends_on_id",
    )
    .bind(todo_id)
    .fetch_all(&mut *conn)
    .await?;
    let depends_on = rows
        .iter()
        .filter(|(later, _)| *later == todo_id)
        .map(|(_, first)| *first)
        .collect();
    let dependents = rows
        .iter()
        .filter(|(_, first)| *first == todo_id)
        .map(|(later, _)| *later)
        .collect();
    Ok((depends_on, dependents))
}

#[derive(FromRow)]
struct PrerequisiteRow {
    todo_id: i64,
    id: i64,
    title: String,
    parent_title: Option<String>,
    project_name: String,
    status: String,
    planned_end: Option<String>,
}

impl PrerequisiteRow {
    fn into_prerequisite(self) -> Result<Prerequisite> {
        Ok(Prerequisite {
            todo_id: self.todo_id,
            id: self.id,
            status: Status::parse(&self.status).ok_or(RepoError::Corrupt {
                column: "todos.status",
                value: self.status.clone(),
            })?,
            planned_end: read_date("todos.planned_end", self.planned_end)?,
            title: self.title,
            parent_title: self.parent_title,
            project_name: self.project_name,
        })
    }
}

/// The prerequisites query, with `$where` between the joins and the order.
macro_rules! prerequisites_sql {
    ($where:literal) => {
        concat!(
            "SELECT d.todo_id, t.id, t.title, p.title AS parent_title,
                    pr.name AS project_name, t.status, t.planned_end
               FROM todo_dependencies d
               JOIN todos t ON t.id = d.depends_on_id
               JOIN projects pr ON pr.id = t.project_id
               LEFT JOIN todos p ON p.id = t.parent_id ",
            $where,
            " ORDER BY d.todo_id, d.created_at, d.depends_on_id"
        )
    };
}

/// The TODOs the TODOs `ids` wait for (`None` = of every TODO), with what
/// the later TODO shows about each: by later TODO, then in the order the
/// dependencies were added.
pub async fn prerequisites(
    conn: &mut SqliteConnection,
    ids: Option<&[i64]>,
) -> Result<Vec<Prerequisite>> {
    let rows = match ids {
        None => {
            sqlx::query_as::<_, PrerequisiteRow>(prerequisites_sql!(""))
                .fetch_all(&mut *conn)
                .await?
        }
        Some([]) => Vec::new(),
        Some(ids) => {
            sqlx::query_as::<_, PrerequisiteRow>(prerequisites_sql!(
                "WHERE d.todo_id IN (SELECT value FROM json_each(?1))"
            ))
            .bind(serde_json::Value::from(ids.to_vec()).to_string())
            .fetch_all(&mut *conn)
            .await?
        }
    };
    rows.into_iter()
        .map(PrerequisiteRow::into_prerequisite)
        .collect()
}

/// The dependency of that pair, if there is one.
pub async fn get(
    conn: &mut SqliteConnection,
    todo_id: i64,
    depends_on_id: i64,
) -> Result<Option<Dependency>> {
    sqlx::query_as::<_, DependencyRow>(
        "SELECT * FROM todo_dependencies WHERE todo_id = ?1 AND depends_on_id = ?2",
    )
    .bind(todo_id)
    .bind(depends_on_id)
    .fetch_optional(&mut *conn)
    .await?
    .map(DependencyRow::into_dependency)
    .transpose()
}

pub async fn insert(
    conn: &mut SqliteConnection,
    todo_id: i64,
    depends_on_id: i64,
    now: DateTime<Utc>,
) -> Result<Dependency> {
    sqlx::query_as::<_, DependencyRow>(
        "INSERT INTO todo_dependencies (todo_id, depends_on_id, created_at)
         VALUES (?1, ?2, ?3)
         RETURNING *",
    )
    .bind(todo_id)
    .bind(depends_on_id)
    .bind(ts_param(now))
    .fetch_one(&mut *conn)
    .await?
    .into_dependency()
}

pub async fn delete(conn: &mut SqliteConnection, todo_id: i64, depends_on_id: i64) -> Result<()> {
    let done =
        sqlx::query("DELETE FROM todo_dependencies WHERE todo_id = ?1 AND depends_on_id = ?2")
            .bind(todo_id)
            .bind(depends_on_id)
            .execute(&mut *conn)
            .await?;
    if done.rows_affected() == 0 {
        return Err(RepoError::NotFound {
            entity: "dependency of todo",
            id: todo_id,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::ts;
    use crate::repo::testing::{conn, project, todo};
    use crate::repo::{projects, todos};

    #[tokio::test]
    async fn insert_get_list_delete() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let a = todo(&mut c, p.id, "a").await;
        let b = todo(&mut c, p.id, "b").await;
        let x = todo(&mut c, p.id, "x").await;
        assert_eq!(get(&mut c, b.id, a.id).await.unwrap(), None);

        let xa = insert(&mut c, x.id, a.id, ts(2026, 11, 19, 1, 0))
            .await
            .unwrap();
        let ba = insert(&mut c, b.id, a.id, ts(2026, 11, 19, 2, 0))
            .await
            .unwrap();
        let bx = insert(&mut c, b.id, x.id, ts(2026, 11, 19, 3, 0))
            .await
            .unwrap();
        assert_eq!(
            ba,
            Dependency {
                todo_id: b.id,
                depends_on_id: a.id,
                created_at: ts(2026, 11, 19, 2, 0)
            }
        );
        assert_eq!(get(&mut c, b.id, a.id).await.unwrap(), Some(ba.clone()));
        // The other direction is another pair.
        assert_eq!(get(&mut c, a.id, b.id).await.unwrap(), None);
        assert_eq!(
            list(&mut c).await.unwrap(),
            vec![ba.clone(), bx.clone(), xa.clone()]
        );

        delete(&mut c, b.id, a.id).await.unwrap();
        assert!(matches!(
            delete(&mut c, b.id, a.id).await,
            Err(RepoError::NotFound { .. })
        ));
        assert_eq!(list(&mut c).await.unwrap(), vec![bx, xa]);
    }

    #[tokio::test]
    async fn the_table_refuses_a_pair_twice_itself_and_missing_todos() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let a = todo(&mut c, p.id, "a").await;
        let b = todo(&mut c, p.id, "b").await;
        insert(&mut c, b.id, a.id, ts(2026, 11, 19, 1, 0))
            .await
            .unwrap();
        for (todo_id, depends_on_id) in [(b.id, a.id), (a.id, a.id), (a.id, 999), (999, a.id)] {
            let err = insert(&mut c, todo_id, depends_on_id, ts(2026, 11, 19, 2, 0))
                .await
                .unwrap_err();
            assert!(
                err.is_constraint_violation(),
                "{todo_id} -> {depends_on_id}: {err}"
            );
        }
        assert_eq!(list(&mut c).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn deleting_either_todo_or_a_project_deletes_the_pair() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let q = project(&mut c, "Q").await;
        let a = todo(&mut c, p.id, "a").await;
        let b = todo(&mut c, p.id, "b").await;
        let x = todo(&mut c, q.id, "x").await;
        let y = todo(&mut c, q.id, "y").await;
        // Across projects too.
        for (later, first) in [(b.id, a.id), (x.id, b.id), (y.id, a.id), (a.id, y.id)] {
            insert(&mut c, later, first, ts(2026, 11, 19, 1, 0))
                .await
                .unwrap();
        }
        // b is the later TODO of one pair and the earlier one of another.
        todos::delete(&mut c, b.id).await.unwrap();
        let left: Vec<(i64, i64)> = list(&mut c)
            .await
            .unwrap()
            .into_iter()
            .map(|d| (d.todo_id, d.depends_on_id))
            .collect();
        assert_eq!(left, [(a.id, y.id), (y.id, a.id)]);
        projects::delete(&mut c, q.id).await.unwrap();
        assert_eq!(list(&mut c).await.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn among_neighbours_and_prerequisites() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let q = project(&mut c, "Q").await;
        let parent = todo(&mut c, p.id, "親").await;
        let a = todo(&mut c, p.id, "a").await;
        let b = todo(&mut c, p.id, "b").await;
        let x = todo(&mut c, q.id, "x").await;
        todos::set_parent(&mut c, a.id, Some(parent.id), ts(2026, 11, 19, 0, 0))
            .await
            .unwrap();
        // b waits for x (added first) and a; x waits for a.
        insert(&mut c, b.id, x.id, ts(2026, 11, 19, 1, 0))
            .await
            .unwrap();
        insert(&mut c, b.id, a.id, ts(2026, 11, 19, 2, 0))
            .await
            .unwrap();
        insert(&mut c, x.id, a.id, ts(2026, 11, 19, 3, 0))
            .await
            .unwrap();

        let pairs = |v: Vec<Dependency>| {
            v.into_iter()
                .map(|d| (d.todo_id, d.depends_on_id))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            pairs(among(&mut c, &[a.id, b.id]).await.unwrap()),
            [(b.id, a.id)]
        );
        assert!(among(&mut c, &[]).await.unwrap().is_empty());

        assert_eq!(
            neighbours(&mut c, b.id).await.unwrap(),
            (vec![x.id, a.id], vec![])
        );
        assert_eq!(
            neighbours(&mut c, a.id).await.unwrap(),
            (vec![], vec![b.id, x.id])
        );

        let all = prerequisites(&mut c, None).await.unwrap();
        let shown: Vec<(i64, i64, &str, Option<&str>, &str)> = all
            .iter()
            .map(|p| {
                (
                    p.todo_id,
                    p.id,
                    p.title.as_str(),
                    p.parent_title.as_deref(),
                    p.project_name.as_str(),
                )
            })
            .collect();
        assert_eq!(
            shown,
            [
                (b.id, x.id, "x", None, "Q"),
                (b.id, a.id, "a", Some("親"), "P"),
                (x.id, a.id, "a", Some("親"), "P"),
            ]
        );
        assert_eq!(all[0].status, Status::Todo);
        assert_eq!(all[0].planned_end, None);
        let of_x = prerequisites(&mut c, Some(&[x.id])).await.unwrap();
        assert_eq!(of_x, vec![all[2].clone()]);
        assert!(prerequisites(&mut c, Some(&[])).await.unwrap().is_empty());
    }
}
