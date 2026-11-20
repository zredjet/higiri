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
                "todo_dependencies",
                "todo_events",
                "todo_repeats",
                "todo_templates",
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
        assert_eq!(applied.0, 2);
        pool.close().await;
    }

    /// Every row of `table` as JSON arrays, in rowid order.
    async fn rows_of(pool: &SqlitePool, table: &str) -> Vec<String> {
        let columns: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info(?1)")
            .bind(table)
            .fetch_all(pool)
            .await
            .unwrap();
        let columns: Vec<String> = columns.into_iter().map(|c| c.0).collect();
        // Only the table's own column names are in the SQL.
        let sql = format!(
            "SELECT json_array({}) FROM {table} ORDER BY rowid",
            columns.join(", ")
        );
        sqlx::query_as::<_, (String,)>(sqlx::AssertSqlSafe(sql))
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.0)
            .collect()
    }

    #[tokio::test]
    async fn the_v0_3_0_migration_keeps_the_rows_of_a_v0_2_0_database() {
        const OLD_TABLES: [&str; 6] = [
            "projects",
            "todos",
            "work_logs",
            "todo_events",
            "reports",
            "settings",
        ];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        // A database of v0.2.0: only 0001 applied, with rows in every table.
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let old = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        MIGRATOR.run_to(1, &old).await.unwrap();
        sqlx::query(
            "INSERT INTO projects (id, name, created_at, updated_at)
               VALUES (1, '基幹システム更改', '2026-11-01T00:00:00.000Z', '2026-11-01T00:00:00.000Z');
             INSERT INTO todos (id, project_id, parent_id, title, status, priority, due_date,
                                closed_at, memo_md, created_at, updated_at)
               VALUES (1, 1, NULL, '移行手順書', 'doing', 1, '2026-11-20', NULL, '## メモ',
                       '2026-11-01T00:00:00.000Z', '2026-11-19T00:00:00.000Z'),
                      (2, 1, 1, '子', 'done', 2, NULL, '2026-11-18T09:00:00.000Z', '',
                       '2026-11-01T00:00:00.000Z', '2026-11-18T09:00:00.000Z');
             INSERT INTO work_logs (todo_id, log_date, body_md, minutes, created_at, updated_at)
               VALUES (1, '2026-11-19', '作業した', 90, '2026-11-19T00:00:00.000Z', '2026-11-19T00:00:00.000Z');
             INSERT INTO todo_events (todo_id, event_date, kind, old_value, new_value, note, occurred_at)
               VALUES (1, '2026-11-01', 'created', NULL, 'todo', '{}', '2026-11-01T00:00:00.000Z');
             INSERT INTO reports (kind, period_start, period_end, body_md, note_md, updated_at)
               VALUES ('daily', '2026-11-19', '2026-11-19', '# 日報', '所感', '2026-11-19T09:00:00.000Z');
             INSERT INTO settings (key, value) VALUES ('theme', '\"dark\"'), ('soon_days', '5');",
        )
        .execute(&old)
        .await
        .unwrap();
        let mut before = Vec::new();
        for table in OLD_TABLES {
            before.push(rows_of(&old, table).await);
        }
        assert!(before.iter().all(|rows| !rows.is_empty()));
        old.close().await;

        // Opening it as this version applies 0002.
        let pool = open(&path).await.unwrap();
        let versions: Vec<(i64,)> =
            sqlx::query_as("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(versions, [(1,), (2,)]);
        for (table, rows) in OLD_TABLES.into_iter().zip(&before) {
            assert_eq!(&rows_of(&pool, table).await, rows, "{table}");
        }
        // The new tables are there and empty, and work with the old rows.
        for table in ["todo_templates", "todo_repeats", "todo_dependencies"] {
            assert_eq!(rows_of(&pool, table).await, Vec::<String>::new(), "{table}");
        }
        sqlx::query(
            "INSERT INTO todo_dependencies (todo_id, depends_on_id, created_at)
             VALUES (2, 1, '2026-11-20T00:00:00.000Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
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
