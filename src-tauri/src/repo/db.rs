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

/// How long a connection waits for a lock held by another one.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens (creating if needed) the database file and applies pending
/// migrations. Every pooled connection enables foreign keys and WAL mode.
pub async fn open(path: &Path) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(BUSY_TIMEOUT);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/// Why [`open_existing`] refused a database (nothing was read or written).
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// No database file, or no `_sqlx_migrations` in it (the app was never
    /// started, or the file is not the app's).
    #[error("no database")]
    Missing,
    /// Migrations of this build are not applied yet (a newer app has not
    /// started since).
    #[error("database is older than this build")]
    Older,
    /// Applied by a newer app than this build.
    #[error("database is newer than this build")]
    Newer,
    /// Still locked after the busy timeout (the app is writing a lot).
    #[error("database is busy")]
    Busy,
    #[error(transparent)]
    Failed(#[from] super::RepoError),
}

/// The newest migration this build carries.
pub fn latest_migration() -> i64 {
    MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0)
}

/// The newest migration applied to the database, or `None` when it has no
/// `_sqlx_migrations` (or nothing applied in it).
pub async fn applied_migration(conn: &mut sqlx::SqliteConnection) -> Result<Option<i64>> {
    let (tables,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(&mut *conn)
    .await?;
    if tables == 0 {
        return Ok(None);
    }
    let (version,): (Option<i64>,) =
        sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(&mut *conn)
            .await?;
    Ok(version)
}

/// Opens the database as the app does (WAL, foreign keys, 5 seconds of
/// waiting), without creating it and without applying migrations (the CLI,
/// v0.5.0). Its applied migrations must be exactly this build's; otherwise
/// the pool is closed and nothing is read or written.
pub async fn open_existing(path: &Path) -> std::result::Result<SqlitePool, OpenError> {
    open_existing_waiting(path, BUSY_TIMEOUT).await
}

async fn open_existing_waiting(
    path: &Path,
    busy_timeout: Duration,
) -> std::result::Result<SqlitePool, OpenError> {
    if !path.is_file() {
        return Err(OpenError::Missing);
    }
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(busy_timeout);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(refused)?;
    let applied = match pool.acquire().await {
        Ok(mut conn) => applied_migration(&mut conn).await,
        Err(e) => Err(e.into()),
    };
    let refusal = match applied {
        Ok(Some(v)) if v == latest_migration() => return Ok(pool),
        Ok(Some(v)) if v < latest_migration() => OpenError::Older,
        Ok(Some(_)) => OpenError::Newer,
        Ok(None) => OpenError::Missing,
        Err(e) => refused(e),
    };
    pool.close().await;
    Err(refusal)
}

/// A failure while opening: busy, a file that is not a database, or else.
fn refused(e: impl Into<super::RepoError>) -> OpenError {
    let e = e.into();
    if e.is_busy() {
        OpenError::Busy
    } else if e.is_not_a_database() {
        OpenError::Missing
    } else {
        OpenError::Failed(e)
    }
}

/// Tests: a WAL database at `path` with the migrations up to `version`
/// applied (as an older app leaves it).
#[cfg(test)]
pub async fn migrated_to(path: &Path, version: i64) {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open");
    MIGRATOR.run_to(version, &pool).await.expect("migrate");
    pool.close().await;
}

/// Tests: one migration more than this build carries recorded as applied
/// (as a newer app leaves the database).
#[cfg(test)]
pub async fn record_later_migration(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO _sqlx_migrations
           (version, description, installed_on, success, checksum, execution_time)
         VALUES (?1, 'later', CURRENT_TIMESTAMP, 1, x'00', 0)",
    )
    .bind(latest_migration() + 1)
    .execute(pool)
    .await
    .expect("record a later migration");
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

    /// The applied versions and the tables of the database at `path`, read
    /// without creating or migrating anything.
    async fn state_of(path: &Path) -> (Vec<i64>, Vec<String>) {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();
        let tables: Vec<String> = tables.into_iter().map(|t| t.0).collect();
        let versions: Vec<(i64,)> = if tables.iter().any(|t| t == "_sqlx_migrations") {
            sqlx::query_as("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap()
        } else {
            Vec::new()
        };
        pool.close().await;
        (versions.into_iter().map(|v| v.0).collect(), tables)
    }

    #[tokio::test]
    async fn open_existing_opens_a_database_of_this_build() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        open(&path).await.unwrap().close().await;
        let pool = open_existing(&path).await.unwrap();
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
        assert_eq!(
            applied_migration(&mut c).await.unwrap(),
            Some(latest_migration())
        );
        drop(c);
        pool.close().await;
    }

    #[tokio::test]
    async fn open_existing_creates_nothing_when_there_is_no_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        assert!(matches!(
            open_existing(&path).await,
            Err(OpenError::Missing)
        ));
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);

        // A folder where the file should be is not a database either.
        let folder = dir.path().join("folder.db");
        std::fs::create_dir(&folder).unwrap();
        assert!(matches!(
            open_existing(&folder).await,
            Err(OpenError::Missing)
        ));
    }

    #[tokio::test]
    async fn open_existing_refuses_a_database_without_migrations() {
        let dir = tempfile::tempdir().unwrap();
        // An SQLite file the app did not make (no _sqlx_migrations).
        let path = dir.path().join(DB_FILE_NAME);
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let other = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE notes (body TEXT)")
            .execute(&other)
            .await
            .unwrap();
        other.close().await;
        assert!(matches!(
            open_existing(&path).await,
            Err(OpenError::Missing)
        ));
        assert_eq!(state_of(&path).await, (vec![], vec!["notes".to_owned()]));

        // A file that is not SQLite at all is left as it is.
        let text = dir.path().join("text.db");
        std::fs::write(&text, "not a database, but long enough to have a header").unwrap();
        assert!(matches!(
            open_existing(&text).await,
            Err(OpenError::Missing)
        ));
        assert_eq!(
            std::fs::read_to_string(&text).unwrap(),
            "not a database, but long enough to have a header"
        );
    }

    #[tokio::test]
    async fn open_existing_refuses_an_older_database_and_leaves_it_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        migrated_to(&path, 1).await;
        let before = state_of(&path).await;
        assert_eq!(before.0, vec![1]);
        assert!(matches!(open_existing(&path).await, Err(OpenError::Older)));
        // No migration was applied.
        assert_eq!(state_of(&path).await, before);
    }

    #[tokio::test]
    async fn open_existing_refuses_a_newer_database_and_leaves_it_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let pool = open(&path).await.unwrap();
        record_later_migration(&pool).await;
        pool.close().await;
        let before = state_of(&path).await;
        assert!(matches!(open_existing(&path).await, Err(OpenError::Newer)));
        assert_eq!(state_of(&path).await, before);
    }

    #[tokio::test]
    async fn open_existing_is_busy_while_the_database_is_locked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        open(&path).await.unwrap().close().await;
        // Another connection holds an exclusive lock (readers wait in WAL
        // mode only for such a lock), with no waiting of its own.
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .locking_mode(sqlx::sqlite::SqliteLockingMode::Exclusive);
        let holder = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let mut held = holder.begin().await.unwrap();
        sqlx::query("INSERT INTO settings (key, value) VALUES ('theme', '\"dark\"')")
            .execute(&mut *held)
            .await
            .unwrap();
        // Short of the 5 seconds: the test sets its own waiting below.
        let start = std::time::Instant::now();
        let result = open_existing_waiting(&path, Duration::from_millis(200)).await;
        assert!(matches!(result, Err(OpenError::Busy)), "{result:?}");
        assert!(start.elapsed() >= Duration::from_millis(200));
        held.rollback().await.unwrap();
        holder.close().await;
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
