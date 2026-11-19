//! All data as one JSON file (C-2): [`export_to`] writes it, [`import`]
//! replaces all data with such a file.
//!
//! ```text
//! {
//!   "format": "higiri-export",
//!   "format_version": 1,
//!   "schema_version": 1,
//!   "app_version": "0.1.0",
//!   "exported_at": "2026-11-19T01:30:00.000Z",
//!   "projects": [
//!     {"id":1,"name":"…",…},
//!     …
//!   ],
//!   "todos": […], "work_logs": […], "todo_events": […], "reports": […],
//!   "settings": {
//!     "theme": "dark",
//!     …
//!   }
//! }
//! ```
//!
//! - A row is an object keyed by column name, with the value as stored
//!   (integer, string or null). `todo_events.note` and the settings values are
//!   stored JSON and are embedded as JSON values, with their stored text kept
//!   as is (key order included). One row per line.
//! - `schema_version` is the newest applied migration. Only `format_version`
//!   1 can be read.
//!
//! Import: the file's format and versions are checked, its rows go into a work
//! database (`backups/import-work.db`) migrated to the file's
//! `schema_version`, where the foreign keys and CHECK constraints check them;
//! then the newer migrations run and every row is read back with the app's own
//! readers. Only then is the current data kept and replaced, exactly as a
//! restore does ([`backup::keep_and_replace`]): the settings in
//! `KEPT_ON_REPLACE` keep their current values. A refused file changes
//! nothing.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow};
use sqlx::{
    AssertSqlSafe, Column, Connection, Row, SqliteConnection, SqlitePool, TypeInfo, ValueRef,
};

use crate::backup::{self, BackupError};
use crate::domain::date::format_timestamp;
use crate::repo::db::MIGRATOR;
use crate::repo::todos::TodoFilter;
use crate::repo::{self, RepoError};

/// `format` of a Higiri export.
pub const FORMAT: &str = "higiri-export";
/// The only `format_version` this version reads and writes.
pub const FORMAT_VERSION: i64 = 1;
/// Where an import is checked before it replaces the data.
const IMPORT_WORK_FILE: &str = "import-work.db";
/// Suffix of the export while it is written; renamed when complete.
const PART_SUFFIX: &str = ".part";

/// A table in the file. Parent-first: rows are inserted in this order.
struct Table {
    name: &'static str,
    /// Every row, in the file's order.
    select: &'static str,
    /// The column holding stored JSON (embedded as a JSON value).
    json_column: Option<&'static str>,
}

const TABLES: [Table; 5] = [
    Table {
        name: "projects",
        select: "SELECT * FROM projects ORDER BY id",
        json_column: None,
    },
    Table {
        name: "todos",
        select: "SELECT * FROM todos ORDER BY id",
        json_column: None,
    },
    Table {
        name: "work_logs",
        select: "SELECT * FROM work_logs ORDER BY id",
        json_column: None,
    },
    Table {
        name: "todo_events",
        select: "SELECT * FROM todo_events ORDER BY id",
        json_column: Some("note"),
    },
    Table {
        name: "reports",
        select: "SELECT * FROM reports ORDER BY kind, period_start",
        json_column: None,
    },
];

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    /// A stored value the file cannot hold (a BLOB; the app never writes one).
    #[error("unexpected {kind} value in {table}.{column}")]
    Unexpected {
        table: &'static str,
        column: String,
        kind: String,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("file error: {0}")]
    Read(std::io::Error),
    /// Not JSON at all (or cut off).
    #[error("the file is not JSON")]
    NotJson,
    /// JSON, but not a Higiri export.
    #[error("the file is not a Higiri export")]
    NotOurs,
    /// Written by a newer version (`format_version` or `schema_version`).
    #[error("the file was written by a newer version of the app")]
    TooNew,
    /// A Higiri export whose shape or value types are wrong.
    #[error("the file's contents do not have the export's shape")]
    Malformed,
    /// Rejected by a FOREIGN KEY, CHECK, NOT NULL or UNIQUE constraint.
    #[error("the file's rows break a constraint")]
    Constraint,
    /// A value the app cannot read (a date, a status name, a note) in this
    /// column. The value itself is not kept (it would reach the log).
    #[error("the file has a value the app cannot read in {column}")]
    Unreadable { column: &'static str },
    /// Keeping or replacing the current data failed.
    #[error("replace failed: {0}")]
    Replace(#[from] BackupError),
    #[error("database error: {0}")]
    Db(RepoError),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

/// A database error from the work database: constraint violations are the
/// file's fault, anything else is ours.
fn db_error(e: sqlx::Error) -> ImportError {
    let e = RepoError::from(e);
    if e.is_constraint_violation() {
        ImportError::Constraint
    } else {
        ImportError::Db(e)
    }
}

// ---- export -------------------------------------------------------------

/// A stored value.
enum Cell {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    /// Stored JSON text, embedded as a JSON value.
    Json(Box<RawValue>),
}

impl Serialize for Cell {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Cell::Null => s.serialize_none(),
            Cell::Int(v) => s.serialize_i64(*v),
            Cell::Real(v) => s.serialize_f64(*v),
            Cell::Text(v) => s.serialize_str(v),
            Cell::Json(v) => v.serialize(s),
        }
    }
}

/// The rows of one table, columns in table order.
struct Rows {
    columns: Vec<String>,
    rows: Vec<Vec<Cell>>,
}

/// One row as a JSON object (keys in column order).
struct RowRef<'a> {
    columns: &'a [String],
    cells: &'a [Cell],
}

impl Serialize for RowRef<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.cells.len()))?;
        for (name, cell) in self.columns.iter().zip(self.cells) {
            map.serialize_entry(name, cell)?;
        }
        map.end()
    }
}

fn read_cell(row: &SqliteRow, i: usize, table: &Table) -> Result<Cell, ExportError> {
    let raw = row.try_get_raw(i)?;
    if raw.is_null() {
        return Ok(Cell::Null);
    }
    let type_info = raw.type_info();
    let column = row.columns()[i].name();
    Ok(match type_info.name() {
        "INTEGER" => Cell::Int(row.try_get_unchecked(i)?),
        "REAL" => Cell::Real(row.try_get_unchecked(i)?),
        "TEXT" => {
            let text: String = row.try_get_unchecked(i)?;
            if table.json_column == Some(column) {
                match serde_json::from_str::<Box<RawValue>>(&text) {
                    Ok(json) => Cell::Json(json),
                    // Never written by the app; kept as a string so that no
                    // text is lost (the import then refuses it as unreadable).
                    Err(_) => {
                        log::warn!("{}.{column} is not JSON; exported as a string", table.name);
                        Cell::Text(text)
                    }
                }
            } else {
                Cell::Text(text)
            }
        }
        other => {
            return Err(ExportError::Unexpected {
                table: table.name,
                column: column.to_owned(),
                kind: other.to_owned(),
            });
        }
    })
}

async fn read_rows(conn: &mut SqliteConnection, table: &Table) -> Result<Rows, ExportError> {
    let fetched = sqlx::query(table.select).fetch_all(&mut *conn).await?;
    let columns = fetched
        .first()
        .map(|r| r.columns().iter().map(|c| c.name().to_owned()).collect())
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(fetched.len());
    for row in &fetched {
        let cells = (0..row.columns().len())
            .map(|i| read_cell(row, i, table))
            .collect::<Result<Vec<_>, _>>()?;
        rows.push(cells);
    }
    Ok(Rows { columns, rows })
}

/// Settings as (key, stored JSON), by key. A value that is not JSON is left
/// out: the app reads it as the default, and so does an import without it.
async fn read_settings(
    conn: &mut SqliteConnection,
) -> Result<Vec<(String, Box<RawValue>)>, ExportError> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM settings ORDER BY key")
            .fetch_all(&mut *conn)
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(key, value)| match serde_json::from_str(&value) {
            Ok(json) => Some((key, json)),
            Err(_) => {
                log::warn!("setting {key} is not JSON; left out of the export");
                None
            }
        })
        .collect())
}

/// The newest migration applied to the database.
async fn schema_version(conn: &mut SqliteConnection) -> Result<i64, sqlx::Error> {
    let v: (Option<i64>,) =
        sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(&mut *conn)
            .await?;
    Ok(v.0.unwrap_or(0))
}

/// The whole file, read in one transaction so that the tables agree.
pub async fn export_json(
    pool: &SqlitePool,
    app_version: &str,
    now: DateTime<Utc>,
) -> Result<Vec<u8>, ExportError> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    let schema = schema_version(&mut tx).await?;
    let mut tables = Vec::with_capacity(TABLES.len());
    for table in &TABLES {
        tables.push((table.name, read_rows(&mut tx, table).await?));
    }
    let settings = read_settings(&mut tx).await?;
    tx.commit().await?;

    let mut out = Vec::new();
    let mut put = |s: &str| out.extend_from_slice(s.as_bytes());
    put("{\n");
    put(&format!("  \"format\": {},\n", json_str(FORMAT)));
    put(&format!("  \"format_version\": {FORMAT_VERSION},\n"));
    put(&format!("  \"schema_version\": {schema},\n"));
    put(&format!("  \"app_version\": {},\n", json_str(app_version)));
    put(&format!(
        "  \"exported_at\": {},\n",
        json_str(&format_timestamp(now))
    ));
    for (name, rows) in &tables {
        put(&format!("  {}: [", json_str(name)));
        for (i, cells) in rows.rows.iter().enumerate() {
            put(if i == 0 { "\n    " } else { ",\n    " });
            put(&serde_json::to_string(&RowRef {
                columns: &rows.columns,
                cells,
            })?);
        }
        put(if rows.rows.is_empty() {
            "],\n"
        } else {
            "\n  ],\n"
        });
    }
    put("  \"settings\": {");
    for (i, (key, value)) in settings.iter().enumerate() {
        put(if i == 0 { "\n    " } else { ",\n    " });
        put(&format!("{}: {}", json_str(key), value.get()));
    }
    put(if settings.is_empty() {
        "}\n"
    } else {
        "\n  }\n"
    });
    put("}\n");
    Ok(out)
}

/// A string as a JSON string (quoted and escaped).
fn json_str(s: &str) -> String {
    serde_json::Value::from(s).to_string()
}

/// Writes the export to `path`: first to a `.part` file in the same folder,
/// renamed when complete, so a failed export never leaves a partial file.
pub async fn export_to(
    pool: &SqlitePool,
    path: &Path,
    app_version: &str,
    now: DateTime<Utc>,
) -> Result<(), ExportError> {
    let bytes = export_json(pool, app_version, now).await?;
    let mut part = path.as_os_str().to_owned();
    part.push(PART_SUFFIX);
    let part = PathBuf::from(part);
    let written = (|| {
        let mut file = std::fs::File::create(&part)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&part, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    Ok(written?)
}

// ---- import -------------------------------------------------------------

/// The fields checked before anything else is read. The other keys are
/// skipped, but still parsed: JSON that is broken anywhere fails here.
#[derive(Deserialize)]
struct Header {
    format: Option<serde_json::Value>,
    format_version: Option<serde_json::Value>,
    schema_version: Option<serde_json::Value>,
}

type FileRow = BTreeMap<String, Box<RawValue>>;

/// The data part of the file. Every table must be there (an export always
/// writes them all); other top-level keys are ignored.
#[derive(Deserialize)]
struct Document {
    projects: Vec<FileRow>,
    todos: Vec<FileRow>,
    work_logs: Vec<FileRow>,
    todo_events: Vec<FileRow>,
    reports: Vec<FileRow>,
    settings: BTreeMap<String, Box<RawValue>>,
}

impl Document {
    fn rows(&self, table: &str) -> &[FileRow] {
        match table {
            "projects" => &self.projects,
            "todos" => &self.todos,
            "work_logs" => &self.work_logs,
            "todo_events" => &self.todo_events,
            "reports" => &self.reports,
            _ => &[],
        }
    }
}

/// The newest migration this version knows.
pub fn latest_schema_version() -> i64 {
    MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0)
}

/// Checks the format and versions and reads the data part. Returns the
/// file's `schema_version` too.
fn parse(bytes: &[u8]) -> Result<(i64, Document), ImportError> {
    // Notepad on Windows may add a byte order mark.
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let header: Header = serde_json::from_slice(bytes).map_err(|e| {
        if e.is_data() {
            // Valid JSON of another shape (an array, a number, …).
            ImportError::NotOurs
        } else {
            ImportError::NotJson
        }
    })?;
    if header.format.as_ref().and_then(|v| v.as_str()) != Some(FORMAT) {
        return Err(ImportError::NotOurs);
    }
    let format_version = header
        .format_version
        .as_ref()
        .and_then(|v| v.as_i64())
        .ok_or(ImportError::Malformed)?;
    if format_version > FORMAT_VERSION {
        return Err(ImportError::TooNew);
    }
    if format_version != FORMAT_VERSION {
        return Err(ImportError::Malformed);
    }
    let schema = header
        .schema_version
        .as_ref()
        .and_then(|v| v.as_i64())
        .ok_or(ImportError::Malformed)?;
    if schema > latest_schema_version() {
        return Err(ImportError::TooNew);
    }
    if !MIGRATOR.version_exists(schema) {
        return Err(ImportError::Malformed);
    }
    let doc = serde_json::from_slice(bytes).map_err(|_| ImportError::Malformed)?;
    Ok((schema, doc))
}

/// How a column's values are checked and bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Integer,
    Text,
    /// Stored JSON: any JSON value, stored as its text (null = NULL).
    Json,
    /// A declared type this version does not expect: a number or a string.
    Other,
}

enum Bind {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

fn bind_value(kind: Kind, raw: &RawValue) -> Result<Bind, ImportError> {
    if kind == Kind::Json {
        return Ok(match raw.get() {
            "null" => Bind::Null,
            text => Bind::Text(text.to_owned()),
        });
    }
    let value: serde_json::Value =
        serde_json::from_str(raw.get()).map_err(|_| ImportError::Malformed)?;
    Ok(match (kind, value) {
        (_, serde_json::Value::Null) => Bind::Null,
        (Kind::Integer | Kind::Other, serde_json::Value::Number(n)) if n.is_i64() => {
            Bind::Int(n.as_i64().unwrap_or_default())
        }
        (Kind::Other, serde_json::Value::Number(n)) => {
            Bind::Real(n.as_f64().ok_or(ImportError::Malformed)?)
        }
        (Kind::Text | Kind::Other, serde_json::Value::String(s)) => Bind::Text(s),
        _ => return Err(ImportError::Malformed),
    })
}

/// Column name → kind, from the work database (at the file's schema version).
async fn column_kinds(
    conn: &mut SqliteConnection,
    table: &Table,
) -> Result<HashMap<String, Kind>, ImportError> {
    let columns: Vec<(String, String)> =
        sqlx::query_as("SELECT name, type FROM pragma_table_info(?1)")
            .bind(table.name)
            .fetch_all(&mut *conn)
            .await
            .map_err(db_error)?;
    Ok(columns
        .into_iter()
        .map(|(name, declared)| {
            let kind = if table.json_column == Some(name.as_str()) {
                Kind::Json
            } else {
                match declared.to_ascii_uppercase().as_str() {
                    "INTEGER" => Kind::Integer,
                    "TEXT" => Kind::Text,
                    _ => Kind::Other,
                }
            };
            (name, kind)
        })
        .collect())
}

/// Bound values per INSERT statement (SQLite allows 32,766).
const MAX_PARAMS: usize = 4_000;

/// Inserts the rows, many per statement: consecutive rows with the same
/// columns (normally all of them) share one multi-row INSERT.
async fn insert_rows(
    conn: &mut SqliteConnection,
    table: &Table,
    rows: &[FileRow],
) -> Result<(), ImportError> {
    let kinds = column_kinds(conn, table).await?;
    let mut start = 0;
    while start < rows.len() {
        let columns: Vec<&str> = rows[start].keys().map(String::as_str).collect();
        if columns.is_empty() {
            return Err(ImportError::Malformed);
        }
        let per_statement = (MAX_PARAMS / columns.len()).max(1);
        let mut values = Vec::new();
        let mut end = start;
        while end < rows.len()
            && end - start < per_statement
            && rows[end]
                .keys()
                .map(String::as_str)
                .eq(columns.iter().copied())
        {
            for (name, raw) in &rows[end] {
                // An unknown column is not this schema version's export.
                let kind = kinds.get(name).ok_or(ImportError::Malformed)?;
                values.push(bind_value(*kind, raw)?);
            }
            end += 1;
        }
        // Only the table's own column names (checked above) are in the SQL.
        let names: Vec<String> = columns.iter().map(|n| format!("\"{n}\"")).collect();
        let group = format!("({})", vec!["?"; columns.len()].join(", "));
        let sql = format!(
            "INSERT INTO {} ({}) VALUES {}",
            table.name,
            names.join(", "),
            vec![group.as_str(); end - start].join(", ")
        );
        let mut query = sqlx::query(AssertSqlSafe(sql));
        for value in values {
            query = match value {
                Bind::Null => query.bind(None::<String>),
                Bind::Int(v) => query.bind(v),
                Bind::Real(v) => query.bind(v),
                Bind::Text(v) => query.bind(v),
            };
        }
        query.execute(&mut *conn).await.map_err(db_error)?;
        start = end;
    }
    Ok(())
}

async fn insert_settings(
    conn: &mut SqliteConnection,
    settings: &BTreeMap<String, Box<RawValue>>,
) -> Result<(), ImportError> {
    // Unknown keys and unreadable values are kept as they are: the app
    // ignores the one and reads the other as the default, as it does for a
    // database. KEPT_ON_REPLACE keys are put back to this PC's values later.
    for (key, value) in settings {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?1, ?2)")
            .bind(key)
            .bind(value.get())
            .execute(&mut *conn)
            .await
            .map_err(db_error)?;
    }
    Ok(())
}

/// Reads every row back with the app's readers, so that nothing imported
/// stops a screen from loading (a date in another form, an unknown status).
async fn check_readable(conn: &mut SqliteConnection) -> Result<(), ImportError> {
    let unreadable = |e: RepoError| match e {
        RepoError::Corrupt { column, .. } => ImportError::Unreadable { column },
        RepoError::Sqlx(e) => db_error(e),
        other => ImportError::Db(other),
    };
    repo::projects::list(conn, true).await.map_err(unreadable)?;
    repo::todos::list(
        conn,
        &TodoFilter {
            include_archived_projects: true,
            ..TodoFilter::default()
        },
    )
    .await
    .map_err(unreadable)?;
    repo::work_logs::list_all(conn).await.map_err(unreadable)?;
    repo::events::list_all(conn).await.map_err(unreadable)?;
    repo::reports::list_all(conn).await.map_err(unreadable)?;
    Ok(())
}

/// Builds the work database from the file and checks it.
async fn fill_work(work: &Path, schema: i64, doc: &Document) -> Result<(), ImportError> {
    let options = SqliteConnectOptions::new()
        .filename(work)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Delete)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(db_error)?;
    let result = async {
        // The file's schema first, so its rows fit; newer migrations after.
        MIGRATOR.run_to(schema, &pool).await?;
        let mut conn = pool.acquire().await.map_err(db_error)?;
        let mut tx = conn.begin().await.map_err(db_error)?;
        // Checked at COMMIT (todos refer to todos).
        sqlx::query("PRAGMA defer_foreign_keys = ON")
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        for table in &TABLES {
            insert_rows(&mut tx, table, doc.rows(table.name)).await?;
        }
        insert_settings(&mut tx, &doc.settings).await?;
        tx.commit().await.map_err(db_error)?;
        drop(conn);
        MIGRATOR.run(&pool).await?;
        let mut conn = pool.acquire().await.map_err(db_error)?;
        check_readable(&mut conn).await
    }
    .await;
    pool.close().await;
    result
}

/// Replaces all data with the export in `bytes`, after keeping a copy of the
/// current data. Returns the name of that copy. A refused file changes
/// nothing (no copy is taken either).
pub async fn import_bytes(
    pool: &SqlitePool,
    data_dir: &Path,
    bytes: &[u8],
    now: DateTime<Utc>,
) -> Result<String, ImportError> {
    let (schema, doc) = parse(bytes)?;
    let dir = backup::dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let work = dir.join(IMPORT_WORK_FILE);
    backup::remove_db_files(&work)?;
    let result = async {
        fill_work(&work, schema, &doc).await?;
        Ok(backup::keep_and_replace(pool, data_dir, &work, now).await?)
    }
    .await;
    if let Err(e) = backup::remove_db_files(&work) {
        log::warn!("could not remove {}: {e}", work.display());
    }
    result
}

/// [`import_bytes`] with the file at `path`.
pub async fn import(
    pool: &SqlitePool,
    data_dir: &Path,
    path: &Path,
    now: DateTime<Utc>,
) -> Result<String, ImportError> {
    let bytes = std::fs::read(path).map_err(ImportError::Read)?;
    import_bytes(pool, data_dir, &bytes, now).await
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::backup::BackupKind;
    use crate::commands::settings::{self as cmd_settings, SettingsInput};
    use crate::domain::settings::{Settings, Theme};
    use crate::domain::testing::{d, ts};
    use crate::repo::db;
    use crate::repo::settings::{self as repo_settings, KEPT_ON_REPLACE};

    const NOW: (i32, u32, u32, u32, u32) = (2026, 11, 19, 1, 30);

    fn now() -> DateTime<Utc> {
        let (y, m, day, h, min) = NOW;
        ts(y, m, day, h, min)
    }

    async fn file_db(dir: &Path) -> SqlitePool {
        db::open(&dir.join(db::DB_FILE_NAME)).await.unwrap()
    }

    /// The sample data, a saved report, settings changed by the user (one of
    /// them unreadable, one unknown) and values the app wrote.
    async fn source() -> SqlitePool {
        let pool = db::open_in_memory().await.unwrap();
        crate::sample::seed_if_empty(&pool, d("2026-11-19"))
            .await
            .unwrap();
        crate::commands::reports::save(
            &pool,
            crate::commands::reports::ReportKey {
                kind: crate::domain::report::ReportKind::Daily,
                date: d("2026-11-19"),
            },
            "# 日報 2026-11-19\n\n## 所感・連絡事項\n- 引用符 \" と改行\n",
            ts(2026, 11, 19, 9, 0),
        )
        .await
        .unwrap();
        cmd_settings::update(
            &pool,
            SettingsInput {
                theme: Theme::Dark,
                notify_times: vec!["08:30".into()],
                autostart: true,
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        let mut c = pool.acquire().await.unwrap();
        repo_settings::set_last_reminded_on(&mut c, Some(d("2026-11-18")))
            .await
            .unwrap();
        repo_settings::set_last_notified_at(&mut c, Some(ts(2026, 11, 18, 3, 55)))
            .await
            .unwrap();
        repo_settings::set_tray_hint_shown(&mut c, true)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES
               ('soon_days', '\"abc\"'), ('future_key', '{\"b\":1,\"a\":[2]}')
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .execute(&mut *c)
        .await
        .unwrap();
        pool
    }

    async fn export(pool: &SqlitePool) -> Vec<u8> {
        export_json(pool, "0.1.0", now()).await.unwrap()
    }

    /// Every stored row of every table (and the settings), as the export
    /// writes them: the stored values themselves.
    async fn dump(pool: &SqlitePool) -> (Vec<String>, BTreeMap<String, String>) {
        let mut c = pool.acquire().await.unwrap();
        let mut tables = Vec::new();
        for table in &TABLES {
            let rows = read_rows(&mut c, table).await.unwrap();
            for cells in &rows.rows {
                let row = RowRef {
                    columns: &rows.columns,
                    cells,
                };
                tables.push(format!(
                    "{} {}",
                    table.name,
                    serde_json::to_string(&row).unwrap()
                ));
            }
        }
        let settings: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
            .fetch_all(&mut *c)
            .await
            .unwrap();
        (tables, settings.into_iter().collect())
    }

    fn without_kept(mut s: BTreeMap<String, String>) -> BTreeMap<String, String> {
        for key in KEPT_ON_REPLACE {
            s.remove(key);
        }
        s
    }

    fn kept(s: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        s.iter()
            .filter(|(k, _)| KEPT_ON_REPLACE.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    #[tokio::test]
    async fn the_file_has_the_designed_shape() {
        let src = source().await;
        let bytes = export(&src).await;
        let text = String::from_utf8(bytes.clone()).unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["format"], "higiri-export");
        assert_eq!(v["format_version"], 1);
        assert_eq!(v["schema_version"], 1);
        assert_eq!(latest_schema_version(), 1);
        assert_eq!(v["app_version"], "0.1.0");
        assert_eq!(v["exported_at"], "2026-11-19T01:30:00.000Z");
        // Rows keyed by column, values as stored.
        let todo = &v["todos"][0];
        assert_eq!(todo["id"], 1);
        assert!(todo["project_id"].is_i64());
        assert!(todo["parent_id"].is_null());
        assert!(todo["created_at"].as_str().unwrap().ends_with('Z'));
        // Stored JSON is embedded as JSON, with its stored text kept.
        let note = &v["todo_events"][0]["note"];
        assert!(note.is_object(), "{note}");
        assert_eq!(v["settings"]["theme"], "dark");
        assert_eq!(v["settings"]["notify_times"], json!(["08:30"]));
        assert_eq!(v["settings"]["last_reminded_on"], "2026-11-18");
        assert!(text.contains(r#""future_key": {"b":1,"a":[2]}"#), "{text}");
        let stored_note: (String,) =
            sqlx::query_as("SELECT note FROM todo_events WHERE note LIKE '%auto%' LIMIT 1")
                .fetch_one(&src)
                .await
                .unwrap();
        assert!(text.contains(&format!(r#""note":{}"#, stored_note.0)));
        // One row per line, readable Japanese (not \u escapes).
        let counts: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM projects), (SELECT COUNT(*) FROM todos),
                    (SELECT COUNT(*) FROM work_logs), (SELECT COUNT(*) FROM todo_events)",
        )
        .fetch_one(&src)
        .await
        .unwrap();
        let row_lines = text
            .lines()
            .filter(|l| l.starts_with("    {\"id\":"))
            .count() as i64;
        assert_eq!(row_lines, counts.0 + counts.1 + counts.2 + counts.3);
        assert!(text.contains("所感・連絡事項"));
        assert!(text.starts_with("{\n  \"format\": \"higiri-export\",\n"));
        assert!(text.ends_with("\n  }\n}\n"));
    }

    #[tokio::test]
    async fn an_empty_database_exports_empty_lists() {
        let pool = db::open_in_memory().await.unwrap();
        let text = String::from_utf8(export(&pool).await).unwrap();
        assert!(text.contains("  \"projects\": [],\n"), "{text}");
        assert!(text.ends_with("  \"settings\": {}\n}\n"), "{text}");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["todos"], json!([]));
    }

    #[tokio::test]
    async fn import_into_an_empty_database_brings_every_row() {
        let src = source().await;
        let bytes = export(&src).await;
        let tmp = tempfile::tempdir().unwrap();
        let dst = file_db(tmp.path()).await;

        let kept_copy = import_bytes(&dst, tmp.path(), &bytes, ts(2026, 11, 19, 2, 0))
            .await
            .unwrap();
        assert_eq!(kept_copy, "before-restore-20261119T020000.000Z.db");

        let (src_rows, src_settings) = dump(&src).await;
        let (dst_rows, dst_settings) = dump(&dst).await;
        assert!(src_rows.len() > 30);
        assert_eq!(dst_rows, src_rows);
        // Every setting but the kept ones, unknown and unreadable ones included.
        assert_eq!(
            without_kept(dst_settings.clone()),
            without_kept(src_settings)
        );
        assert_eq!(dst_settings["future_key"], r#"{"b":1,"a":[2]}"#);
        assert_eq!(dst_settings["soon_days"], "\"abc\"");
        let read = cmd_settings::get(&dst).await.unwrap();
        assert_eq!(read.soon_days, Settings::default().soon_days);
        assert_eq!(read.theme, Theme::Dark);
        // The empty database had none of the kept keys, and still has none.
        assert_eq!(kept(&dst_settings), BTreeMap::new());

        // The work database is gone; the copy of the data before is listed.
        assert!(!backup::dir(tmp.path()).join(IMPORT_WORK_FILE).exists());
        let listed = backup::list(tmp.path()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].kind, BackupKind::BeforeRestore);
        assert_eq!(listed[0].file_name, kept_copy);
    }

    #[tokio::test]
    async fn import_keeps_the_pc_and_app_written_settings() {
        let src = source().await;
        let bytes = export(&src).await;
        let tmp = tempfile::tempdir().unwrap();
        let dst = file_db(tmp.path()).await;
        cmd_settings::update(
            &dst,
            SettingsInput {
                autostart: false,
                theme: Theme::Light,
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        {
            let mut c = dst.acquire().await.unwrap();
            repo_settings::set_last_reminded_on(&mut c, Some(d("2026-11-19")))
                .await
                .unwrap();
            repo_settings::set_last_notified_at(&mut c, Some(ts(2026, 11, 19, 0, 0)))
                .await
                .unwrap();
            // tray_hint_shown stays unset here, though the file has true.
        }
        let before = dump(&dst).await.1;

        import_bytes(&dst, tmp.path(), &bytes, now()).await.unwrap();
        let after = dump(&dst).await.1;
        assert_eq!(kept(&after), kept(&before));
        let read = cmd_settings::get(&dst).await.unwrap();
        assert!(!read.autostart);
        assert_eq!(read.last_reminded_on, Some(d("2026-11-19")));
        assert_eq!(read.last_notified_at, Some(ts(2026, 11, 19, 0, 0)));
        assert!(!read.tray_hint_shown);
        assert!(!after.contains_key("tray_hint_shown"));
        // The user settings came from the file.
        assert_eq!(read.theme, Theme::Dark);
        assert_eq!(read.notify_times, ["08:30"]);
    }

    #[tokio::test]
    async fn new_rows_get_ids_after_the_imported_ones() {
        let src = source().await;
        let bytes = export(&src).await;
        let tmp = tempfile::tempdir().unwrap();
        let dst = file_db(tmp.path()).await;
        import_bytes(&dst, tmp.path(), &bytes, now()).await.unwrap();

        for (table, insert) in [
            (
                "projects",
                "INSERT INTO projects (name, created_at, updated_at)
                 VALUES ('新規', '2026-11-19T00:00:00.000Z', '2026-11-19T00:00:00.000Z') RETURNING id",
            ),
            (
                "todos",
                "INSERT INTO todos (project_id, title, created_at, updated_at)
                 VALUES (1, '新規', '2026-11-19T00:00:00.000Z', '2026-11-19T00:00:00.000Z') RETURNING id",
            ),
            (
                "work_logs",
                "INSERT INTO work_logs (todo_id, log_date, body_md, created_at, updated_at)
                 VALUES (1, '2026-11-19', '新規', '2026-11-19T00:00:00.000Z', '2026-11-19T00:00:00.000Z') RETURNING id",
            ),
            (
                "todo_events",
                "INSERT INTO todo_events (todo_id, event_date, kind, note, occurred_at)
                 VALUES (1, '2026-11-19', 'due_date', '{}', '2026-11-19T00:00:00.000Z') RETURNING id",
            ),
        ] {
            let max: (i64,) = sqlx::query_as(AssertSqlSafe(format!("SELECT MAX(id) FROM {table}")))
                .fetch_one(&dst)
                .await
                .unwrap();
            let new: (i64,) = sqlx::query_as(insert).fetch_one(&dst).await.unwrap();
            assert_eq!(new.0, max.0 + 1, "{table}");
        }
        // No table uses AUTOINCREMENT (no sqlite_sequence to carry over).
        let seq: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE name = 'sqlite_sequence'")
                .fetch_one(&dst)
                .await
                .unwrap();
        assert_eq!(seq.0, 0);
    }

    #[tokio::test]
    async fn an_empty_export_empties_the_data_and_keeps_the_kept_settings() {
        let empty = db::open_in_memory().await.unwrap();
        let bytes = export(&empty).await;
        let tmp = tempfile::tempdir().unwrap();
        let dst = file_db(tmp.path()).await;
        crate::sample::seed_if_empty(&dst, d("2026-11-19"))
            .await
            .unwrap();
        {
            let mut c = dst.acquire().await.unwrap();
            repo_settings::set_tray_hint_shown(&mut c, true)
                .await
                .unwrap();
        }
        import_bytes(&dst, tmp.path(), &bytes, now()).await.unwrap();
        let (rows, settings) = dump(&dst).await;
        assert_eq!(rows, Vec::<String>::new());
        assert_eq!(settings.keys().collect::<Vec<_>>(), ["tray_hint_shown"]);
    }

    #[tokio::test]
    async fn a_byte_order_mark_and_reformatted_json_are_read() {
        let src = source().await;
        let v: Value = serde_json::from_slice(&export(&src).await).unwrap();
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend(serde_json::to_vec_pretty(&v).unwrap());
        let tmp = tempfile::tempdir().unwrap();
        let dst = file_db(tmp.path()).await;
        import_bytes(&dst, tmp.path(), &bytes, now()).await.unwrap();
        assert_eq!(dump(&dst).await.0.len(), dump(&src).await.0.len());
        // Notes reformatted by hand are still read.
        let mut c = dst.acquire().await.unwrap();
        repo::events::list_all(&mut c).await.unwrap();
    }

    #[tokio::test]
    async fn missing_columns_take_their_defaults() {
        let file = json!({
            "format": "higiri-export", "format_version": 1, "schema_version": 1,
            "projects": [{"id": 3, "name": "P", "created_at": "2026-11-19T00:00:00.000Z",
                          "updated_at": "2026-11-19T00:00:00.000Z"}],
            "todos": [], "work_logs": [], "todo_events": [], "reports": [], "settings": {}
        });
        let tmp = tempfile::tempdir().unwrap();
        let dst = file_db(tmp.path()).await;
        import_bytes(&dst, tmp.path(), file.to_string().as_bytes(), now())
            .await
            .unwrap();
        let p: (i64, String, String, i64) =
            sqlx::query_as("SELECT id, color, description, sort_order FROM projects")
                .fetch_one(&dst)
                .await
                .unwrap();
        assert_eq!(p, (3, "#3B6FD4".into(), String::new(), 0));
    }

    #[tokio::test]
    async fn rows_with_other_columns_and_many_rows_are_all_inserted() {
        let full = |id: i64| {
            json!({"id": id, "name": format!("P{id}"), "color": "#000000", "description": "",
                   "sort_order": id, "archived_at": null,
                   "created_at": "2026-11-19T00:00:00.000Z", "updated_at": "2026-11-19T00:00:00.000Z"})
        };
        // 1,200 rows of 8 columns: three statements of up to 500 rows, with
        // a row of fewer columns in between.
        let mut projects: Vec<Value> = (1..=700).map(full).collect();
        projects.push(
            json!({"id": 701, "name": "short", "created_at": "2026-11-19T00:00:00.000Z",
                             "updated_at": "2026-11-19T00:00:00.000Z"}),
        );
        projects.extend((702..=1_200).map(full));
        let file = json!({
            "format": "higiri-export", "format_version": 1, "schema_version": 1,
            "projects": projects,
            "todos": [], "work_logs": [], "todo_events": [], "reports": [], "settings": {}
        });
        let tmp = tempfile::tempdir().unwrap();
        let dst = file_db(tmp.path()).await;
        import_bytes(&dst, tmp.path(), file.to_string().as_bytes(), now())
            .await
            .unwrap();
        let rows: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT id, name, color FROM projects ORDER BY id")
                .fetch_all(&dst)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1_200);
        assert_eq!(rows[0], (1, "P1".into(), "#000000".into()));
        assert_eq!(rows[700], (701, "short".into(), "#3B6FD4".into()));
        assert_eq!(rows[1_199], (1_200, "P1200".into(), "#000000".into()));
    }

    /// What `import_bytes` says about `file`, and that nothing changed.
    async fn refused(file: &[u8]) -> ImportError {
        let tmp = tempfile::tempdir().unwrap();
        let dst = file_db(tmp.path()).await;
        crate::sample::seed_if_empty(&dst, d("2026-11-19"))
            .await
            .unwrap();
        let before = dump(&dst).await;
        let err = import_bytes(&dst, tmp.path(), file, now())
            .await
            .unwrap_err();
        assert_eq!(dump(&dst).await, before, "{err}");
        // No copy was taken and the work database is gone.
        assert_eq!(backup::list(tmp.path()).unwrap(), vec![]);
        assert!(!backup::dir(tmp.path()).join(IMPORT_WORK_FILE).exists());
        err
    }

    /// A change made to an export by hand.
    type Edit = Box<dyn FnOnce(&mut Value)>;

    /// The sample export with `change` applied.
    async fn edited(change: impl FnOnce(&mut Value)) -> Vec<u8> {
        let src = source().await;
        let mut v: Value = serde_json::from_slice(&export(&src).await).unwrap();
        change(&mut v);
        serde_json::to_vec(&v).unwrap()
    }

    #[tokio::test]
    async fn files_that_are_not_ours_are_refused() {
        for file in [&b""[..], b"{\"format\":", b"\xff\xfe{", b"not json"] {
            assert!(matches!(refused(file).await, ImportError::NotJson));
        }
        for file in [
            &b"[1, 2]"[..],
            b"\"higiri-export\"",
            b"{}",
            b"{\"format\": \"other-app\", \"format_version\": 1}",
            b"{\"format\": 1}",
        ] {
            assert!(matches!(refused(file).await, ImportError::NotOurs));
        }
    }

    #[tokio::test]
    async fn files_of_a_newer_version_are_refused() {
        let newer_format = edited(|v| v["format_version"] = json!(2)).await;
        assert!(matches!(refused(&newer_format).await, ImportError::TooNew));
        let newer_schema = edited(|v| v["schema_version"] = json!(2)).await;
        assert!(matches!(refused(&newer_schema).await, ImportError::TooNew));
    }

    #[tokio::test]
    async fn files_of_the_wrong_shape_are_refused() {
        let cases: Vec<Edit> = vec![
            Box::new(|v| v["format_version"] = json!(0)),
            Box::new(|v| v["format_version"] = json!("1")),
            Box::new(|v| {
                v.as_object_mut()
                    .unwrap()
                    .remove("format_version")
                    .map(drop)
                    .unwrap()
            }),
            Box::new(|v| v["schema_version"] = json!(0)),
            Box::new(|v| v["schema_version"] = json!(1.5)),
            Box::new(|v| {
                v.as_object_mut()
                    .unwrap()
                    .remove("work_logs")
                    .map(drop)
                    .unwrap()
            }),
            Box::new(|v| v["settings"] = json!([])),
            Box::new(|v| v["todos"][0]["id"] = json!("1")),
            Box::new(|v| v["todos"][0]["title"] = json!(1)),
            Box::new(|v| v["todos"][0]["priority"] = json!(true)),
            Box::new(|v| v["todos"][0]["priority"] = json!(1.5)),
            Box::new(|v| v["todos"][0]["memo_md"] = json!({"a": 1})),
            Box::new(|v| v["todos"][0]["extra"] = json!(1)),
            Box::new(|v| v["projects"][0] = json!({})),
            Box::new(|v| v["projects"][0] = json!([1])),
        ];
        for (i, change) in cases.into_iter().enumerate() {
            let file = edited(change).await;
            let err = refused(&file).await;
            assert!(matches!(err, ImportError::Malformed), "case {i}: {err}");
        }
    }

    #[tokio::test]
    async fn rows_breaking_a_constraint_are_refused() {
        let cases: Vec<Edit> = vec![
            // FOREIGN KEY
            Box::new(|v| v["todos"][0]["project_id"] = json!(999)),
            Box::new(|v| v["todos"][0]["parent_id"] = json!(999)),
            Box::new(|v| v["work_logs"][0]["todo_id"] = json!(999)),
            Box::new(|v| v["projects"].as_array_mut().unwrap().clear()),
            // CHECK
            Box::new(|v| v["todos"][0]["status"] = json!("later")),
            Box::new(|v| {
                v["todos"][0]["planned_start"] = json!("2026-11-20");
                v["todos"][0]["planned_end"] = json!("2026-11-19");
            }),
            Box::new(|v| v["reports"][0]["kind"] = json!("yearly")),
            // NOT NULL
            Box::new(|v| v["todos"][0]["title"] = json!(null)),
            // UNIQUE (PRIMARY KEY)
            Box::new(|v| {
                let first = v["projects"][0].clone();
                v["projects"].as_array_mut().unwrap().push(first);
            }),
        ];
        for (i, change) in cases.into_iter().enumerate() {
            let file = edited(change).await;
            let err = refused(&file).await;
            assert!(matches!(err, ImportError::Constraint), "case {i}: {err}");
        }
    }

    #[tokio::test]
    async fn values_the_app_cannot_read_are_refused() {
        let cases: Vec<Edit> = vec![
            Box::new(|v| v["todos"][0]["due_date"] = json!("2026/11/19")),
            Box::new(|v| v["todos"][0]["created_at"] = json!("昨日")),
            Box::new(|v| v["todos"][0]["priority"] = json!(5)),
            Box::new(|v| v["projects"][0]["archived_at"] = json!("2026-11-19")),
            Box::new(|v| v["work_logs"][0]["log_date"] = json!("2026-13-01")),
            Box::new(|v| v["todo_events"][0]["kind"] = json!("moved")),
            Box::new(|v| v["todo_events"][0]["note"] = json!("相手:人事部")),
            Box::new(|v| v["reports"][0]["period_start"] = json!("11/19")),
        ];
        for (i, change) in cases.into_iter().enumerate() {
            let file = edited(change).await;
            let err = refused(&file).await;
            assert!(
                matches!(err, ImportError::Unreadable { .. }),
                "case {i}: {err}"
            );
            // The log line names the column, never the value.
            assert!(!err.to_string().contains('"'), "case {i}: {err}");
        }
    }
}
