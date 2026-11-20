//! `todo_dependencies` table (TODO間の依存関係, v0.3.0). One row per pair:
//! the TODO `todo_id` cannot be started until `depends_on_id` is finished.
//!
//! The table refuses a pair twice (PRIMARY KEY) and a TODO waiting for itself
//! (CHECK). Circles are not the table's business: the app refuses them
//! (phase 22). Deleting either TODO deletes the row (ON DELETE CASCADE).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};

use super::{RepoError, Result, read_ts, ts_param};
use crate::domain::model::Dependency;

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
}
