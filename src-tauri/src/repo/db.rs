//! Opening the database and applying migrations.

use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use super::Result;

/// File name of the database inside the app data folder.
pub const DB_FILE_NAME: &str = "todo.db";

/// Migrations embedded at compile time from `src-tauri/migrations/`.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Opens (creating if needed) the database file and applies pending
/// migrations. Every pooled connection enables foreign keys and WAL mode.
pub async fn open(path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// A migrated in-memory database for tests. Uses a single connection that is
/// never recycled, because each SQLite in-memory connection is its own database.
#[cfg(test)]
pub async fn open_in_memory() -> Result<SqlitePool> {
    use std::str::FromStr;

    let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn file_database_uses_wal_and_foreign_keys_on_every_connection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let pool = open(&path).await.unwrap();
        assert!(path.exists());

        // Hold several connections at once so the pool opens more than one.
        let mut held = Vec::new();
        for _ in 0..3 {
            let mut c = pool.acquire().await.unwrap();
            let fk: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
                .fetch_one(&mut *c)
                .await
                .unwrap();
            assert_eq!(fk.0, 1);
            let jm: (String,) = sqlx::query_as("PRAGMA journal_mode")
                .fetch_one(&mut *c)
                .await
                .unwrap();
            assert_eq!(jm.0, "wal");
            held.push(c);
        }
        drop(held);
        pool.close().await;
    }

    #[tokio::test]
    async fn migrations_create_all_tables_and_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let pool = open(&path).await.unwrap();
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let names: Vec<_> = tables.into_iter().map(|t| t.0).collect();
        assert_eq!(
            names,
            [
                "projects",
                "reports",
                "settings",
                "todo_events",
                "todos",
                "work_logs"
            ]
        );
        pool.close().await;

        // Re-opening applies nothing new and does not fail.
        let pool = open(&path).await.unwrap();
        let applied: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(applied.0, 1);
        pool.close().await;
    }

    #[tokio::test]
    async fn in_memory_database_has_foreign_keys_on() {
        let pool = open_in_memory().await.unwrap();
        let fk: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fk.0, 1);
    }
}
