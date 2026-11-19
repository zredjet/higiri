//! SQL access (sqlx): connection setup, migrations, reads and writes.
//!
//! Every function takes `&mut SqliteConnection`, so the caller (`commands/`)
//! decides the transaction boundary: pass `&mut *tx` from `pool.begin()` to
//! group several writes, or `&mut *pool.acquire().await?` for a single read.
//! Timestamps are passed in; nothing here reads the clock.

pub mod db;
pub mod events;
pub mod projects;
pub mod settings;
pub mod todos;
pub mod work_logs;

use chrono::{DateTime, NaiveDate, Utc};

use crate::domain::date::{format_date, format_timestamp, parse_date, parse_timestamp};

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("{entity} {id} was not found")]
    NotFound { entity: &'static str, id: i64 },
    #[error("unexpected value in {column}: {value:?}")]
    Corrupt { column: &'static str, value: String },
}

pub type Result<T> = std::result::Result<T, RepoError>;

impl RepoError {
    /// True when the database rejected the write because of a CHECK,
    /// NOT NULL, FOREIGN KEY or UNIQUE constraint.
    pub fn is_constraint_violation(&self) -> bool {
        match self {
            RepoError::Sqlx(sqlx::Error::Database(e)) => {
                // SQLite primary result code 19 = SQLITE_CONSTRAINT.
                e.code()
                    .and_then(|c| c.parse::<i32>().ok())
                    .is_some_and(|c| c & 0xff == 19)
            }
            _ => false,
        }
    }
}

// ---- value conversion between domain types and stored strings ----------

fn date_param(v: Option<NaiveDate>) -> Option<String> {
    v.map(format_date)
}

fn ts_param(v: DateTime<Utc>) -> String {
    format_timestamp(v)
}

fn opt_ts_param(v: Option<DateTime<Utc>>) -> Option<String> {
    v.map(format_timestamp)
}

fn read_date(column: &'static str, v: Option<String>) -> Result<Option<NaiveDate>> {
    match v {
        None => Ok(None),
        Some(s) => parse_date(&s)
            .map(Some)
            .ok_or(RepoError::Corrupt { column, value: s }),
    }
}

fn read_required_date(column: &'static str, v: String) -> Result<NaiveDate> {
    parse_date(&v).ok_or(RepoError::Corrupt { column, value: v })
}

fn read_ts(column: &'static str, v: String) -> Result<DateTime<Utc>> {
    parse_timestamp(&v).ok_or(RepoError::Corrupt { column, value: v })
}

fn read_opt_ts(column: &'static str, v: Option<String>) -> Result<Option<DateTime<Utc>>> {
    v.map(|s| read_ts(column, s)).transpose()
}

#[cfg(test)]
pub(crate) mod testing {
    use sqlx::SqlitePool;
    use sqlx::pool::PoolConnection;
    use sqlx::sqlite::Sqlite;

    use super::db;
    use super::projects::{self, NewProject};
    use crate::domain::model::Project;
    use crate::domain::testing::ts;

    /// A migrated in-memory database and one connection to it.
    pub async fn conn() -> (SqlitePool, PoolConnection<Sqlite>) {
        let pool = db::open_in_memory().await.expect("open in-memory db");
        let conn = pool.acquire().await.expect("acquire");
        (pool, conn)
    }

    pub async fn project(conn: &mut sqlx::SqliteConnection, name: &str) -> Project {
        projects::insert(
            conn,
            &NewProject {
                name: name.into(),
                color: None,
                description: String::new(),
            },
            ts(2026, 11, 1, 0, 0),
        )
        .await
        .expect("insert project")
    }
}
