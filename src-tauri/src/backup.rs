//! Database backups (C-1).
//!
//! - At the first launch of a day the database is copied to
//!   `backups/todo-YYYY-MM-DD.db`; the newest 7 such copies are kept (days the
//!   app was not started do not count).
//! - Restoring first copies the current data to
//!   `backups/before-restore-<UTC time>.db` (the newest 3 are kept, and they are
//!   listed too, so a restore can be undone), then replaces every row with the
//!   backup's rows in one transaction. The open pool stays usable, so the app
//!   does not need to restart. The JSON import (`json_data`) replaces the data
//!   the same way ([`keep_and_replace`]).
//! - The settings in `repo::settings::KEPT_ON_REPLACE` (`autostart`, `zoom`
//!   and the backup mirror's folder, which belong to this PC, and the values
//!   the app writes) keep their current values through the replacement.
//! - A restore can also take a file chosen anywhere ([`restore_file`]; v0.4.0,
//!   C-3), such as a backup on the mirror after moving to a new PC.
//! - バックアップの複製先 (v0.4.0, C-3): [`sync_mirror`] keeps a folder chosen
//!   by the user (an external drive, a synced folder) in step with the daily
//!   backups: the newest 7, under the same names. Copies taken before a
//!   restore are not mirrored.
//!
//! Copies are written with `VACUUM INTO`, never by copying the WAL-mode file
//! (the daily backups are such copies, so the mirror copies their files).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::domain::date::{format_date, parse_date};
use crate::repo::db;
use crate::repo::settings::KEPT_ON_REPLACE;

/// Folder inside the data folder.
pub const BACKUP_DIR: &str = "backups";
/// Daily backups kept.
pub const KEEP_DAILY: usize = 7;
/// Copies taken before a restore kept.
pub const KEEP_BEFORE_RESTORE: usize = 3;

const DAILY_PREFIX: &str = "todo-";
const BEFORE_RESTORE_PREFIX: &str = "before-restore-";
const EXT: &str = ".db";
/// UTC with milliseconds; sorts by time and is a valid Windows file name.
const STAMP_FORMAT: &str = "%Y%m%dT%H%M%S%.3fZ";
/// Suffix of a copy being written; renamed when complete.
const PART_SUFFIX: &str = ".part";
/// Where a backup is checked and migrated before its rows are copied.
const RESTORE_WORK_FILE: &str = "restore-work.db";

/// Every table: rows referring to others are deleted before the rows they
/// refer to, and inserted after them. `_sqlx_migrations` is not copied.
/// v0.3.0's tables: `todo_dependencies` and `todo_repeats` refer to `todos`;
/// `todo_templates` refers to nothing.
const DELETE_CHILD_FIRST: [&str; 9] = [
    "DELETE FROM main.todo_dependencies",
    "DELETE FROM main.todo_repeats",
    "DELETE FROM main.todo_templates",
    "DELETE FROM main.todo_events",
    "DELETE FROM main.work_logs",
    "DELETE FROM main.todos",
    "DELETE FROM main.reports",
    "DELETE FROM main.settings",
    "DELETE FROM main.projects",
];
const INSERT_PARENT_FIRST: [&str; 9] = [
    "INSERT INTO main.projects SELECT * FROM src.projects",
    "INSERT INTO main.todos SELECT * FROM src.todos",
    "INSERT INTO main.work_logs SELECT * FROM src.work_logs",
    "INSERT INTO main.todo_events SELECT * FROM src.todo_events",
    "INSERT INTO main.reports SELECT * FROM src.reports",
    "INSERT INTO main.settings SELECT * FROM src.settings",
    "INSERT INTO main.todo_templates SELECT * FROM src.todo_templates",
    "INSERT INTO main.todo_repeats SELECT * FROM src.todo_repeats",
    "INSERT INTO main.todo_dependencies SELECT * FROM src.todo_dependencies",
];

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("no backup named {0:?}")]
    NotFound(String),
    /// Made by a newer version of the app (has migrations this one lacks).
    #[error("backup was made by a newer version of the app")]
    TooNew,
    /// Not a readable Higiri database.
    #[error("backup is damaged or not a Higiri database")]
    Damaged,
    #[error("path is not valid UTF-8: {0:?}")]
    Path(PathBuf),
    /// The chosen file could not be read (restore_file).
    #[error("the chosen file could not be read: {0}")]
    Source(std::io::Error),
    /// The chosen file is the database in use (restore_file).
    #[error("the chosen file is the database in use")]
    InUse,
}

pub type Result<T> = std::result::Result<T, BackupError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    /// 自動バックアップ (first launch of the day).
    Daily,
    /// 復元・取り込みの前の状態 (taken just before a restore or a JSON import).
    BeforeRestore,
}

/// What a backup's file name says about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupName {
    Daily(NaiveDate),
    BeforeRestore(DateTime<Utc>),
}

impl BackupName {
    /// Only names this module writes; anything else (including paths) is None.
    pub fn parse(file_name: &str) -> Option<BackupName> {
        let stem = file_name.strip_suffix(EXT)?;
        let name = if let Some(d) = stem.strip_prefix(DAILY_PREFIX) {
            BackupName::Daily(parse_date(d)?)
        } else {
            let s = stem.strip_prefix(BEFORE_RESTORE_PREFIX)?;
            BackupName::BeforeRestore(
                NaiveDateTime::parse_from_str(s, STAMP_FORMAT)
                    .ok()?
                    .and_utc(),
            )
        };
        // Exactly the form we write (no "2026-1-5" or extra characters).
        (name.file_name() == file_name).then_some(name)
    }

    pub fn file_name(&self) -> String {
        match self {
            BackupName::Daily(d) => format!("{DAILY_PREFIX}{}{EXT}", format_date(*d)),
            BackupName::BeforeRestore(t) => {
                format!("{BEFORE_RESTORE_PREFIX}{}{EXT}", t.format(STAMP_FORMAT))
            }
        }
    }

    pub fn kind(&self) -> BackupKind {
        match self {
            BackupName::Daily(_) => BackupKind::Daily,
            BackupName::BeforeRestore(_) => BackupKind::BeforeRestore,
        }
    }
}

/// A backup file as the settings screen lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Backup {
    /// File name in the backups folder; restore_backup takes it back.
    pub file_name: String,
    pub kind: BackupKind,
    /// 自動バックアップ: the day it was taken for.
    pub date: Option<NaiveDate>,
    /// When the file was written.
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
}

/// The backups folder inside the data folder.
pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join(BACKUP_DIR)
}

/// Backups in the folder, newest first. A missing folder is an empty list.
pub fn list(data_dir: &Path) -> Result<Vec<Backup>> {
    let dir = dir(data_dir);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().and_then(BackupName::parse) else {
            continue;
        };
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        let created_at = match name {
            BackupName::BeforeRestore(t) => t,
            BackupName::Daily(_) => meta.modified().unwrap_or(SystemTime::UNIX_EPOCH).into(),
        };
        out.push(Backup {
            file_name: name.file_name(),
            kind: name.kind(),
            date: match name {
                BackupName::Daily(d) => Some(d),
                BackupName::BeforeRestore(_) => None,
            },
            created_at,
            size_bytes: meta.len(),
        });
    }
    out.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.file_name.cmp(&a.file_name))
    });
    Ok(out)
}

/// File names to delete so that only the newest `keep_daily` daily backups and
/// `keep_before` before-restore copies remain. Names that are not backups are
/// never returned.
pub fn names_to_prune(names: &[String], keep_daily: usize, keep_before: usize) -> Vec<String> {
    let mut daily: Vec<(NaiveDate, &String)> = Vec::new();
    let mut before: Vec<(DateTime<Utc>, &String)> = Vec::new();
    for n in names {
        match BackupName::parse(n) {
            Some(BackupName::Daily(d)) => daily.push((d, n)),
            Some(BackupName::BeforeRestore(t)) => before.push((t, n)),
            None => {}
        }
    }
    daily.sort_by_key(|&(d, _)| std::cmp::Reverse(d));
    before.sort_by_key(|&(t, _)| std::cmp::Reverse(t));
    daily
        .into_iter()
        .skip(keep_daily)
        .map(|(_, n)| n.clone())
        .chain(before.into_iter().skip(keep_before).map(|(_, n)| n.clone()))
        .collect()
}

/// Dates of the daily backups in `backups/`, newest first. A missing folder
/// is an empty list.
pub fn daily_dates(data_dir: &Path) -> std::io::Result<Vec<NaiveDate>> {
    let mut out: Vec<NaiveDate> = file_names(&dir(data_dir))?
        .iter()
        .filter_map(|n| match BackupName::parse(n) {
            Some(BackupName::Daily(d)) => Some(d),
            _ => None,
        })
        .collect();
    out.sort_by_key(|&d| std::cmp::Reverse(d));
    Ok(out)
}

/// Names of the regular files in `folder`. A missing folder is an empty list.
fn file_names(folder: &Path) -> std::io::Result<Vec<String>> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if let Some(n) = entry.file_name().to_str() {
            out.push(n.to_owned());
        }
    }
    Ok(out)
}

/// `todo-2026-11-19.db.part`: a copy to the mirror that did not finish.
fn is_daily_part(file_name: &str) -> bool {
    matches!(
        file_name
            .strip_suffix(PART_SUFFIX)
            .and_then(BackupName::parse),
        Some(BackupName::Daily(_))
    )
}

/// Brings the mirror folder in step with the daily backups (v0.4.0, C-3):
///
/// 1. removes the `.part` files a failed copy left there;
/// 2. copies each of the newest 7 daily backups the mirror does not have,
///    through `todo-YYYY-MM-DD.db.part` (a drive pulled out in the middle of
///    a copy never leaves a file that counts as a backup);
/// 3. keeps the newest 7 `todo-YYYY-MM-DD.db` there.
///
/// No other file of the mirror is touched, and it is never created: a
/// missing folder (a drive that is not plugged in) is an error. The daily
/// backups are single files (`VACUUM INTO`), so a file copy is enough.
/// Returns the newest daily backup's date (None when there is none).
/// Blocking: may take a while on a slow drive (see [`mirror`]).
pub fn sync_mirror(data_dir: &Path, mirror: &Path) -> std::io::Result<Option<NaiveDate>> {
    if !mirror.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the mirror folder is not there",
        ));
    }
    let mut daily = daily_dates(data_dir)?;
    daily.truncate(KEEP_DAILY);

    for n in file_names(mirror)? {
        if is_daily_part(&n) {
            std::fs::remove_file(mirror.join(&n))?;
        }
    }
    let there = file_names(mirror)?;
    let source_dir = dir(data_dir);
    for d in &daily {
        let name = BackupName::Daily(*d).file_name();
        if there.contains(&name) {
            continue;
        }
        copy_through_part(&source_dir.join(&name), &mirror.join(&name))?;
    }
    // Only daily names count; before-restore copies are never put there, so
    // any with that name are someone else's and stay.
    for n in names_to_prune(&file_names(mirror)?, KEEP_DAILY, usize::MAX) {
        std::fs::remove_file(mirror.join(&n))?;
    }
    Ok(daily.first().copied())
}

/// [`sync_mirror`] off the async threads (a slow or missing drive must not
/// hold up the resident timer or the commands).
pub async fn mirror(data_dir: PathBuf, mirror: PathBuf) -> std::io::Result<Option<NaiveDate>> {
    tauri::async_runtime::spawn_blocking(move || sync_mirror(&data_dir, &mirror))
        .await
        .map_err(std::io::Error::other)?
}

/// Copies `source` to `target` through `target.part`, flushed to the drive
/// before it gets its name. A failed copy removes its `.part`.
fn copy_through_part(source: &Path, target: &Path) -> std::io::Result<()> {
    let part = part_of(target);
    let copied = (|| {
        std::fs::copy(source, &part)?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&part)?
            .sync_all()?;
        std::fs::rename(&part, target)
    })();
    if copied.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    copied
}

/// Whether `path` is `base` or inside it (links and `..` resolved; letters
/// compared without case, as the folders of Windows and macOS usually are).
/// False when either cannot be resolved (does not exist).
pub fn is_within(path: &Path, base: &Path) -> bool {
    let (Ok(path), Ok(base)) = (std::fs::canonicalize(path), std::fs::canonicalize(base)) else {
        return false;
    };
    let parts = |p: &Path| -> Vec<String> {
        p.components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect()
    };
    parts(&path).starts_with(&parts(&base))
}

fn prune(dir: &Path) -> Result<()> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        if let Some(n) = entry?.file_name().to_str() {
            names.push(n.to_owned());
        }
    }
    for n in names_to_prune(&names, KEEP_DAILY, KEEP_BEFORE_RESTORE) {
        std::fs::remove_file(dir.join(&n))?;
        log::info!("backup removed: {n}");
    }
    Ok(())
}

fn path_str(p: &Path) -> Result<&str> {
    p.to_str().ok_or_else(|| BackupError::Path(p.to_owned()))
}

/// Writes a consistent copy of the database to `target` (via a `.part` file,
/// so an interrupted copy never looks like a backup).
async fn vacuum_into(pool: &SqlitePool, target: &Path) -> Result<()> {
    let part = part_of(target);
    remove_if_exists(&part)?;
    sqlx::query("VACUUM INTO ?1")
        .bind(path_str(&part)?)
        .execute(pool)
        .await?;
    std::fs::rename(&part, target)?;
    Ok(())
}

/// `target` with `.part` added: where a copy is written before it gets its name.
fn part_of(target: &Path) -> PathBuf {
    let mut part = target.as_os_str().to_owned();
    part.push(PART_SUFFIX);
    PathBuf::from(part)
}

fn remove_if_exists(p: &Path) -> Result<()> {
    match std::fs::remove_file(p) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Removes a database file and its -wal / -shm / -journal companions.
pub fn remove_db_files(p: &Path) -> Result<()> {
    remove_if_exists(p)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut s = p.as_os_str().to_owned();
        s.push(suffix);
        remove_if_exists(Path::new(&s))?;
    }
    Ok(())
}

/// Takes today's backup unless it already exists, then removes old ones.
/// Returns the new file, or None when today's backup was already there.
pub async fn run_daily(
    pool: &SqlitePool,
    data_dir: &Path,
    today: NaiveDate,
) -> Result<Option<PathBuf>> {
    let dir = dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(BackupName::Daily(today).file_name());
    if target.exists() {
        return Ok(None);
    }
    vacuum_into(pool, &target).await?;
    prune(&dir)?;
    Ok(Some(target))
}

/// Replaces all data with the backup `file_name` (in the backups folder),
/// after keeping a copy of the current data. Returns the name of that copy.
pub async fn restore(
    pool: &SqlitePool,
    data_dir: &Path,
    file_name: &str,
    now: DateTime<Utc>,
) -> Result<String> {
    // Rebuilt from the parsed name, so no other path can be reached.
    let name =
        BackupName::parse(file_name).ok_or_else(|| BackupError::NotFound(file_name.to_owned()))?;
    let source = dir(data_dir).join(name.file_name());
    if !source.is_file() {
        return Err(BackupError::NotFound(file_name.to_owned()));
    }
    restore_from(pool, data_dir, &source, now).await
}

/// Replaces all data with the Higiri database `source`, a file the user chose
/// anywhere (a backup on the mirror, in `backups/`, …), after keeping a copy
/// of the current data, as [`restore`] does. The file itself is never
/// changed. The database in use (`todo.db` of the data folder, and its
/// `-wal` and `-shm` files) is refused, however it was reached (another case
/// of the letters, a link): as a plain file it may lack the latest changes
/// (they can still be in its `-wal` file). Returns the name of the kept copy.
pub async fn restore_file(
    pool: &SqlitePool,
    data_dir: &Path,
    source: &Path,
    now: DateTime<Utc>,
) -> Result<String> {
    if is_in_use(data_dir, source) {
        return Err(BackupError::InUse);
    }
    restore_from(pool, data_dir, source, now).await
}

/// Whether `path` is the database in use or one of its `-wal` / `-shm`
/// files (compared after `canonicalize`, see [`is_within`]).
fn is_in_use(data_dir: &Path, path: &Path) -> bool {
    let db = data_dir.join(db::DB_FILE_NAME);
    let mut files = vec![db.clone()];
    for suffix in ["-wal", "-shm"] {
        let mut s = db.as_os_str().to_owned();
        s.push(suffix);
        files.push(PathBuf::from(s));
    }
    files.iter().any(|f| is_within(path, f))
}

async fn restore_from(
    pool: &SqlitePool,
    data_dir: &Path,
    source: &Path,
    now: DateTime<Utc>,
) -> Result<String> {
    let dir = dir(data_dir);
    std::fs::create_dir_all(&dir)?;

    // 1. Check and migrate a work copy; the backup itself is never changed.
    let work = dir.join(RESTORE_WORK_FILE);
    remove_db_files(&work)?;
    if let Err(e) = std::fs::copy(source, &work) {
        let _ = remove_db_files(&work);
        return Err(BackupError::Source(e));
    }
    let result = async {
        prepare(&work).await?;
        keep_and_replace(pool, data_dir, &work, now).await
    }
    .await;
    if let Err(e) = remove_db_files(&work) {
        log::warn!("could not remove {}: {e}", work.display());
    }
    result
}

/// Keeps the current data as a 「復元・取り込みの前の状態」 copy, replaces
/// every row with the rows of `work` (a checked and migrated Higiri database)
/// in one transaction, then removes old copies. Returns the copy's file name.
/// Used by the restore and by the JSON import.
pub async fn keep_and_replace(
    pool: &SqlitePool,
    data_dir: &Path,
    work: &Path,
    now: DateTime<Utc>,
) -> Result<String> {
    let dir = dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    // 2. Keep the current data.
    let kept = BackupName::BeforeRestore(now).file_name();
    vacuum_into(pool, &dir.join(&kept)).await?;

    // 3. Replace every row in one transaction.
    copy_rows(pool, work).await?;
    prune(&dir)?;
    Ok(kept)
}

/// Checks that `work` is an intact Higiri database and brings its schema up to
/// this version's migrations.
async fn prepare(work: &Path) -> Result<()> {
    let options = SqliteConnectOptions::new()
        .filename(work)
        .journal_mode(SqliteJournalMode::Delete)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| BackupError::Damaged)?;
    let result = async {
        let check: (String,) = sqlx::query_as("PRAGMA quick_check")
            .fetch_one(&pool)
            .await
            .map_err(|_| BackupError::Damaged)?;
        if check.0 != "ok" {
            return Err(BackupError::Damaged);
        }
        let ours: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_one(&pool)
        .await?;
        if ours.0 == 0 {
            return Err(BackupError::Damaged);
        }
        match db::MIGRATOR.run(&pool).await {
            Ok(()) => Ok(()),
            Err(sqlx::migrate::MigrateError::VersionMissing(_)) => Err(BackupError::TooNew),
            Err(e) => Err(e.into()),
        }
    }
    .await;
    pool.close().await;
    result
}

/// Replaces every row of the database with the rows of `work` (a migrated
/// Higiri database) in one transaction, keeping the current values of the
/// settings in [`KEPT_ON_REPLACE`] (a key that is not set now is not set
/// afterwards either).
pub async fn copy_rows(pool: &SqlitePool, work: &Path) -> Result<()> {
    let mut conn = pool.acquire().await?;
    // ATTACH cannot run inside a transaction.
    sqlx::query("ATTACH DATABASE ?1 AS src")
        .bind(path_str(work)?)
        .execute(&mut *conn)
        .await?;
    let copied = async {
        let mut tx = db::begin_write_on(&mut conn).await?;
        // Checked at COMMIT instead of row by row (todos refer to todos).
        sqlx::query("PRAGMA defer_foreign_keys = ON")
            .execute(&mut *tx)
            .await?;
        let mut kept: Vec<(String, String)> = Vec::new();
        for key in KEPT_ON_REPLACE {
            let row: Option<(String, String)> =
                sqlx::query_as("SELECT key, value FROM main.settings WHERE key = ?1")
                    .bind(key)
                    .fetch_optional(&mut *tx)
                    .await?;
            kept.extend(row);
        }
        for sql in DELETE_CHILD_FIRST {
            sqlx::query(sql).execute(&mut *tx).await?;
        }
        for sql in INSERT_PARENT_FIRST {
            sqlx::query(sql).execute(&mut *tx).await?;
        }
        for key in KEPT_ON_REPLACE {
            sqlx::query("DELETE FROM main.settings WHERE key = ?1")
                .bind(key)
                .execute(&mut *tx)
                .await?;
        }
        for (key, value) in &kept {
            sqlx::query("INSERT INTO main.settings (key, value) VALUES (?1, ?2)")
                .bind(key)
                .bind(value)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }
    .await;
    // Detach even when the copy failed (a dropped transaction rolls back first).
    let detached = sqlx::query("DETACH DATABASE src").execute(&mut *conn).await;
    copied?;
    detached?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, ts};
    use crate::repo::projects::{self, NewProject};

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn names_round_trip_and_reject_anything_else() {
        let daily = BackupName::Daily(d("2026-11-19"));
        assert_eq!(daily.file_name(), "todo-2026-11-19.db");
        assert_eq!(BackupName::parse("todo-2026-11-19.db"), Some(daily));

        let before = BackupName::BeforeRestore(ts(2026, 11, 19, 11, 30));
        assert_eq!(before.file_name(), "before-restore-20261119T113000.000Z.db");
        assert_eq!(BackupName::parse(&before.file_name()), Some(before));

        for bad in [
            "todo-2026-1-5.db",
            "todo-2026-11-19.db.part",
            "todo.db",
            "../todo-2026-11-19.db",
            "todo-2026-11-19.db/../../x",
            "restore-work.db",
            "before-restore-2026.db",
        ] {
            assert_eq!(BackupName::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn prune_keeps_the_newest_seven_days_and_three_copies() {
        let all = names(&[
            "todo-2026-11-01.db",
            "todo-2026-11-03.db",
            "todo-2026-11-10.db",
            "todo-2026-11-12.db",
            "todo-2026-11-15.db",
            "todo-2026-11-16.db",
            "todo-2026-11-18.db",
            "todo-2026-11-19.db",
            "before-restore-20261110T010000.000Z.db",
            "before-restore-20261118T010000.000Z.db",
            "before-restore-20261119T010000.000Z.db",
            "before-restore-20261119T020000.000Z.db",
            "todo.db",
            "notes.txt",
        ]);
        let mut gone = names_to_prune(&all, KEEP_DAILY, KEEP_BEFORE_RESTORE);
        gone.sort();
        // Days without a launch do not count: 11/01 goes, 11/03 stays.
        assert_eq!(
            gone,
            names(&[
                "before-restore-20261110T010000.000Z.db",
                "todo-2026-11-01.db"
            ])
        );
    }

    async fn file_db(dir: &Path) -> SqlitePool {
        db::open(&dir.join(db::DB_FILE_NAME)).await.unwrap()
    }

    async fn add_project(pool: &SqlitePool, name: &str) {
        let mut c = pool.acquire().await.unwrap();
        projects::insert(
            &mut c,
            &NewProject {
                name: name.into(),
                color: None,
                description: String::new(),
            },
            ts(2026, 11, 1, 0, 0),
        )
        .await
        .unwrap();
    }

    async fn project_names(pool: &SqlitePool) -> Vec<String> {
        sqlx::query_as::<_, (String,)>("SELECT name FROM projects ORDER BY id")
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.0)
            .collect()
    }

    #[tokio::test]
    async fn daily_backup_once_a_day_then_listed() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        add_project(&pool, "A").await;

        let first = run_daily(&pool, tmp.path(), d("2026-11-19")).await.unwrap();
        assert!(first.is_some());
        assert_eq!(
            run_daily(&pool, tmp.path(), d("2026-11-19")).await.unwrap(),
            None
        );

        let listed = list(tmp.path()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].file_name, "todo-2026-11-19.db");
        assert_eq!(listed[0].kind, BackupKind::Daily);
        assert_eq!(listed[0].date, Some(d("2026-11-19")));
        assert!(listed[0].size_bytes > 0);
        // No .part file is left behind.
        assert_eq!(std::fs::read_dir(dir(tmp.path())).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn restore_replaces_the_data_and_keeps_the_current_one() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        add_project(&pool, "A").await;
        run_daily(&pool, tmp.path(), d("2026-11-19")).await.unwrap();
        add_project(&pool, "B").await;
        assert_eq!(project_names(&pool).await, ["A", "B"]);

        let kept = restore(
            &pool,
            tmp.path(),
            "todo-2026-11-19.db",
            ts(2026, 11, 19, 12, 0),
        )
        .await
        .unwrap();
        assert_eq!(project_names(&pool).await, ["A"]);
        assert_eq!(kept, "before-restore-20261119T120000.000Z.db");

        // The kept copy can undo the restore.
        restore(&pool, tmp.path(), &kept, ts(2026, 11, 19, 12, 5))
            .await
            .unwrap();
        assert_eq!(project_names(&pool).await, ["A", "B"]);

        // Both kept copies are listed, newest first, next to the daily backup.
        // (The daily backup's time is its file time, so its place in the list
        // depends on the clock; only the kept copies' order is checked.)
        let listed = list(tmp.path()).unwrap();
        let kept_names: Vec<&str> = listed
            .iter()
            .filter(|b| b.kind == BackupKind::BeforeRestore)
            .map(|b| b.file_name.as_str())
            .collect();
        assert_eq!(
            kept_names,
            [
                "before-restore-20261119T120500.000Z.db",
                "before-restore-20261119T120000.000Z.db"
            ]
        );
        assert!(listed.iter().any(|b| b.file_name == "todo-2026-11-19.db"));
        // The work file is gone, and the pool still works (foreign keys on).
        assert!(!dir(tmp.path()).join(RESTORE_WORK_FILE).exists());
        let fk: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fk.0, 1);
    }

    #[tokio::test]
    async fn restore_keeps_the_pc_and_app_written_settings() {
        use crate::commands::settings::{self as cmd, SettingsInput};
        use crate::domain::settings::{Settings, Theme};
        use crate::repo::settings as repo_settings;

        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        // At backup time: autostart off, 表示倍率 80%, the other kept values
        // unset, and a user setting (theme, notification times) that the
        // restore brings back.
        cmd::update(
            &pool,
            SettingsInput {
                theme: Theme::Dark,
                notify_times: vec!["08:00".into()],
                zoom: 80,
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        let mirror_then = tmp.path().join("mirror-then").display().to_string();
        let mirror_now = tmp.path().join("mirror-now").display().to_string();
        {
            let mut c = pool.acquire().await.unwrap();
            repo_settings::set_backup_mirror(&mut c, Some(&mirror_then), Some(d("2026-11-17")))
                .await
                .unwrap();
            repo_settings::set_cli_hidden_projects(&mut c, &[3])
                .await
                .unwrap();
        }
        run_daily(&pool, tmp.path(), d("2026-11-18")).await.unwrap();

        // Since then: autostart on, 表示倍率 125%, another mirror, the app
        // wrote its values, the user changed the theme.
        cmd::update(
            &pool,
            SettingsInput {
                theme: Theme::Light,
                autostart: true,
                zoom: 125,
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        {
            let mut c = pool.acquire().await.unwrap();
            repo_settings::set_last_reminded_on(&mut c, Some(d("2026-11-19")))
                .await
                .unwrap();
            repo_settings::set_last_notified_at(&mut c, Some(ts(2026, 11, 19, 3, 55)))
                .await
                .unwrap();
            repo_settings::set_tray_hint_shown(&mut c, true)
                .await
                .unwrap();
            repo_settings::set_backup_mirror(&mut c, Some(&mirror_now), Some(d("2026-11-19")))
                .await
                .unwrap();
            repo_settings::set_cli_hidden_projects(&mut c, &[1, 5])
                .await
                .unwrap();
        }
        let before = cmd::get(&pool).await.unwrap();

        restore(
            &pool,
            tmp.path(),
            "todo-2026-11-18.db",
            ts(2026, 11, 19, 4, 0),
        )
        .await
        .unwrap();
        let after = cmd::get(&pool).await.unwrap();
        // Restored from the backup.
        assert_eq!(after.theme, Theme::Dark);
        assert_eq!(after.notify_times, ["08:00"]);
        // With the backup's projects (v0.5.0).
        assert_eq!(after.cli_hidden_projects, vec![3]);
        // Kept as they were.
        assert!(after.autostart);
        assert_eq!(after.zoom, 125);
        assert_eq!(after.backup_mirror_dir, Some(mirror_now.clone()));
        assert_eq!(after.backup_mirror_last_ok, Some(d("2026-11-19")));
        assert_eq!(after.last_reminded_on, before.last_reminded_on);
        assert_eq!(after.last_notified_at, before.last_notified_at);
        assert!(after.tray_hint_shown);

        // Undoing the restore brings the theme back and still keeps the
        // current values (tray_hint_shown changed in between).
        let kept = list(tmp.path())
            .unwrap()
            .into_iter()
            .find(|b| b.kind == BackupKind::BeforeRestore)
            .unwrap();
        {
            let mut c = pool.acquire().await.unwrap();
            repo_settings::set_tray_hint_shown(&mut c, false)
                .await
                .unwrap();
        }
        restore(&pool, tmp.path(), &kept.file_name, ts(2026, 11, 19, 4, 5))
            .await
            .unwrap();
        let undone = cmd::get(&pool).await.unwrap();
        assert_eq!(undone.theme, Theme::Light);
        assert!(!undone.tray_hint_shown);
        assert!(undone.autostart);
        assert_eq!(undone.zoom, 125);
        assert_eq!(undone.backup_mirror_dir, Some(mirror_now));
    }

    #[tokio::test]
    async fn restore_leaves_unset_kept_keys_unset() {
        use crate::repo::settings as repo_settings;

        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        {
            let mut c = pool.acquire().await.unwrap();
            repo_settings::set_tray_hint_shown(&mut c, true)
                .await
                .unwrap();
        }
        run_daily(&pool, tmp.path(), d("2026-11-18")).await.unwrap();
        sqlx::query("DELETE FROM settings")
            .execute(&pool)
            .await
            .unwrap();
        restore(
            &pool,
            tmp.path(),
            "todo-2026-11-18.db",
            ts(2026, 11, 19, 4, 0),
        )
        .await
        .unwrap();
        let keys: Vec<(String,)> = sqlx::query_as("SELECT key FROM settings")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(keys, vec![]);
    }

    /// The v0.3.0 tables as (template names, repeating TODO ids, dependency pairs).
    async fn v0_3_0_rows(pool: &SqlitePool) -> (Vec<String>, Vec<i64>, Vec<(i64, i64)>) {
        use crate::repo::{dependencies, repeats, templates};
        let mut c = pool.acquire().await.unwrap();
        (
            templates::list(&mut c)
                .await
                .unwrap()
                .into_iter()
                .map(|t| t.name)
                .collect(),
            repeats::list(&mut c)
                .await
                .unwrap()
                .into_iter()
                .map(|r| r.todo_id)
                .collect(),
            dependencies::list(&mut c)
                .await
                .unwrap()
                .into_iter()
                .map(|d| (d.todo_id, d.depends_on_id))
                .collect(),
        )
    }

    fn template(name: &str) -> crate::repo::templates::TemplateFields {
        use crate::domain::model::Priority;
        use crate::domain::template::TemplateBody;
        crate::repo::templates::TemplateFields {
            name: name.into(),
            body: TemplateBody {
                title: name.into(),
                priority: Priority::Medium,
                memo_md: String::new(),
                children: vec![],
            },
        }
    }

    #[tokio::test]
    async fn restore_replaces_templates_repeats_and_dependencies() {
        use crate::domain::repeat::RepeatRule;
        use crate::repo::testing::todo;
        use crate::repo::{dependencies, repeats, templates};

        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        add_project(&pool, "A").await;
        let at = ts(2026, 11, 18, 0, 0);
        let (t1, t2, t3) = {
            let mut c = pool.acquire().await.unwrap();
            let t1 = todo(&mut c, 1, "t1").await.id;
            let t2 = todo(&mut c, 1, "t2").await.id;
            let t3 = todo(&mut c, 1, "t3").await.id;
            templates::insert(&mut c, &template("月次締め"), at)
                .await
                .unwrap();
            repeats::insert(&mut c, t1, &RepeatRule::Daily, None, at)
                .await
                .unwrap();
            dependencies::insert(&mut c, t2, t1, at).await.unwrap();
            (t1, t2, t3)
        };
        run_daily(&pool, tmp.path(), d("2026-11-18")).await.unwrap();
        let backed_up = v0_3_0_rows(&pool).await;
        assert_eq!(
            backed_up,
            (vec!["月次締め".to_owned()], vec![t1], vec![(t2, t1)])
        );

        // Since then: every table changed.
        {
            let mut c = pool.acquire().await.unwrap();
            let old = templates::list(&mut c).await.unwrap();
            templates::delete(&mut c, old[0].id).await.unwrap();
            templates::insert(&mut c, &template("週次"), at)
                .await
                .unwrap();
            repeats::delete(&mut c, t1).await.unwrap();
            repeats::insert(&mut c, t3, &RepeatRule::MonthlyLast, None, at)
                .await
                .unwrap();
            dependencies::insert(&mut c, t3, t2, at).await.unwrap();
        }
        let changed = v0_3_0_rows(&pool).await;
        assert_ne!(changed, backed_up);

        restore(
            &pool,
            tmp.path(),
            "todo-2026-11-18.db",
            ts(2026, 11, 19, 0, 0),
        )
        .await
        .unwrap();
        assert_eq!(v0_3_0_rows(&pool).await, backed_up);
        // And back again from the copy kept before the restore.
        let kept = list(tmp.path())
            .unwrap()
            .into_iter()
            .find(|b| b.kind == BackupKind::BeforeRestore)
            .unwrap();
        restore(&pool, tmp.path(), &kept.file_name, ts(2026, 11, 19, 0, 5))
            .await
            .unwrap();
        assert_eq!(v0_3_0_rows(&pool).await, changed);
    }

    #[tokio::test]
    async fn a_v0_2_0_backup_is_restored_with_the_new_tables_empty() {
        use crate::domain::repeat::RepeatRule;
        use crate::repo::testing::todo;
        use crate::repo::{dependencies, repeats, templates};

        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        add_project(&pool, "今のプロジェクト").await;
        {
            let at = ts(2026, 11, 19, 0, 0);
            let mut c = pool.acquire().await.unwrap();
            let a = todo(&mut c, 1, "a").await.id;
            let b = todo(&mut c, 1, "b").await.id;
            templates::insert(&mut c, &template("月次締め"), at)
                .await
                .unwrap();
            repeats::insert(&mut c, a, &RepeatRule::Daily, None, at)
                .await
                .unwrap();
            dependencies::insert(&mut c, b, a, at).await.unwrap();
        }

        // A backup taken by v0.2.0: only 0001 applied.
        let bdir = dir(tmp.path());
        std::fs::create_dir_all(&bdir).unwrap();
        let old = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(bdir.join("todo-2026-11-17.db"))
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Delete),
            )
            .await
            .unwrap();
        db::MIGRATOR.run_to(1, &old).await.unwrap();
        sqlx::query(
            "INSERT INTO projects (id, name, created_at, updated_at)
             VALUES (1, 'v0.2.0のプロジェクト', '2026-11-01T00:00:00.000Z', '2026-11-01T00:00:00.000Z')",
        )
        .execute(&old)
        .await
        .unwrap();
        old.close().await;

        restore(
            &pool,
            tmp.path(),
            "todo-2026-11-17.db",
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();
        assert_eq!(project_names(&pool).await, ["v0.2.0のプロジェクト"]);
        assert_eq!(v0_3_0_rows(&pool).await, (vec![], vec![], vec![]));
        // The backup itself was not migrated (only its work copy was).
        let old = SqlitePool::connect_with(
            SqliteConnectOptions::new().filename(bdir.join("todo-2026-11-17.db")),
        )
        .await
        .unwrap();
        let applied: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&old)
            .await
            .unwrap();
        assert_eq!(applied.0, 1);
        old.close().await;
    }

    #[tokio::test]
    async fn restore_refuses_unknown_damaged_and_newer_backups() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        add_project(&pool, "A").await;
        let bdir = dir(tmp.path());
        std::fs::create_dir_all(&bdir).unwrap();
        let now = ts(2026, 11, 19, 12, 0);

        assert!(matches!(
            restore(&pool, tmp.path(), "todo-2026-11-18.db", now).await,
            Err(BackupError::NotFound(_))
        ));
        assert!(matches!(
            restore(&pool, tmp.path(), "../todo.db", now).await,
            Err(BackupError::NotFound(_))
        ));

        std::fs::write(bdir.join("todo-2026-11-17.db"), b"not a database").unwrap();
        assert!(matches!(
            restore(&pool, tmp.path(), "todo-2026-11-17.db", now).await,
            Err(BackupError::Damaged)
        ));

        // A backup with a migration this version does not know.
        run_daily(&pool, tmp.path(), d("2026-11-16")).await.unwrap();
        // Options, not a URL: a Windows path is not a valid URL.
        let newer = SqlitePool::connect_with(
            SqliteConnectOptions::new().filename(bdir.join("todo-2026-11-16.db")),
        )
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
             VALUES (99990101000000, 'future', 1, x'00', 0)",
        )
        .execute(&newer)
        .await
        .unwrap();
        newer.close().await;
        assert!(matches!(
            restore(&pool, tmp.path(), "todo-2026-11-16.db", now).await,
            Err(BackupError::TooNew)
        ));

        // Nothing was changed, and no copy of the current data was taken.
        assert_eq!(project_names(&pool).await, ["A"]);
        assert!(
            list(tmp.path())
                .unwrap()
                .iter()
                .all(|b| b.kind == BackupKind::Daily)
        );
    }

    // ---- バックアップの複製先 (v0.4.0) ----

    /// Writes files into a folder (created first).
    fn put_files(folder: &Path, files: &[(&str, &str)]) {
        std::fs::create_dir_all(folder).unwrap();
        for (name, body) in files {
            std::fs::write(folder.join(name), body).unwrap();
        }
    }

    /// File names in a folder, sorted, with their contents.
    fn contents(folder: &Path) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = std::fs::read_dir(folder)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().unwrap().is_file())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read_to_string(e.path()).unwrap_or_default(),
                )
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn the_mirror_gets_only_the_daily_backups_it_lacks() {
        let data = tempfile::tempdir().unwrap();
        let mirror = tempfile::tempdir().unwrap();
        put_files(
            &dir(data.path()),
            &[
                ("todo-2026-11-19.db", "19"),
                ("todo-2026-11-18.db", "18"),
                ("todo-2026-11-16.db", "16"),
                ("before-restore-20261119T010000.000Z.db", "kept"),
                ("restore-work.db", "work"),
            ],
        );
        let backups_before = contents(&dir(data.path()));
        put_files(
            mirror.path(),
            &[
                // Already there: not copied again.
                ("todo-2026-11-18.db", "18 on the mirror"),
                // Left by a copy that did not finish.
                ("todo-2026-11-19.db.part", "half"),
                // Not ours.
                ("notes.txt", "memo"),
                ("todo.db", "other"),
                ("before-restore-20261101T010000.000Z.db", "someone else's"),
                ("todo-2026-11-17.db.bak", "x"),
            ],
        );
        std::fs::create_dir(mirror.path().join("todo-2026-11-10.db")).unwrap();

        let newest = sync_mirror(data.path(), mirror.path()).unwrap();
        assert_eq!(newest, Some(d("2026-11-19")));
        assert_eq!(
            contents(mirror.path()),
            [
                (
                    "before-restore-20261101T010000.000Z.db".to_owned(),
                    "someone else's".to_owned()
                ),
                ("notes.txt".to_owned(), "memo".to_owned()),
                ("todo-2026-11-16.db".to_owned(), "16".to_owned()),
                ("todo-2026-11-17.db.bak".to_owned(), "x".to_owned()),
                (
                    "todo-2026-11-18.db".to_owned(),
                    "18 on the mirror".to_owned()
                ),
                ("todo-2026-11-19.db".to_owned(), "19".to_owned()),
                ("todo.db".to_owned(), "other".to_owned()),
            ]
        );
        assert!(mirror.path().join("todo-2026-11-10.db").is_dir());
        // The backups folder is as it was.
        assert_eq!(contents(&dir(data.path())), backups_before);

        // In step: nothing changes.
        assert_eq!(
            sync_mirror(data.path(), mirror.path()).unwrap(),
            Some(d("2026-11-19"))
        );
        assert_eq!(contents(mirror.path()).len(), 7);
    }

    #[test]
    fn the_mirror_keeps_the_newest_seven() {
        let data = tempfile::tempdir().unwrap();
        let mirror = tempfile::tempdir().unwrap();
        put_files(
            &dir(data.path()),
            &[("todo-2026-11-19.db", "19"), ("todo-2026-11-18.db", "18")],
        );
        // Nine old ones, from the weeks before.
        let old: Vec<String> = (1..=9)
            .map(|day| format!("todo-2026-11-{day:02}.db"))
            .collect();
        let old_files: Vec<(&str, &str)> = old.iter().map(|n| (n.as_str(), "old")).collect();
        put_files(mirror.path(), &old_files);

        sync_mirror(data.path(), mirror.path()).unwrap();
        let names: Vec<String> = contents(mirror.path())
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            names,
            [
                "todo-2026-11-05.db",
                "todo-2026-11-06.db",
                "todo-2026-11-07.db",
                "todo-2026-11-08.db",
                "todo-2026-11-09.db",
                "todo-2026-11-18.db",
                "todo-2026-11-19.db",
            ]
        );
    }

    #[test]
    fn only_the_newest_seven_daily_backups_are_copied() {
        let data = tempfile::tempdir().unwrap();
        let mirror = tempfile::tempdir().unwrap();
        // Eight in the backups folder (one more than it keeps).
        let names: Vec<String> = (10..=17)
            .map(|day| format!("todo-2026-11-{day}.db"))
            .collect();
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x")).collect();
        put_files(&dir(data.path()), &files);
        assert_eq!(
            sync_mirror(data.path(), mirror.path()).unwrap(),
            Some(d("2026-11-17"))
        );
        let there = contents(mirror.path());
        assert_eq!(there.len(), 7);
        assert_eq!(there[0].0, "todo-2026-11-11.db");
    }

    #[test]
    fn no_daily_backup_yet_is_nothing_to_copy() {
        let data = tempfile::tempdir().unwrap();
        let mirror = tempfile::tempdir().unwrap();
        assert_eq!(sync_mirror(data.path(), mirror.path()).unwrap(), None);
        assert!(contents(mirror.path()).is_empty());
    }

    #[test]
    fn a_missing_mirror_fails_and_is_not_created() {
        let data = tempfile::tempdir().unwrap();
        put_files(&dir(data.path()), &[("todo-2026-11-19.db", "19")]);
        let gone = tempfile::tempdir().unwrap();
        let mirror = gone.path().join("USB");
        let err = sync_mirror(data.path(), &mirror).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(!mirror.exists());
        // A file in place of the folder fails too.
        std::fs::write(&mirror, "a file").unwrap();
        assert!(sync_mirror(data.path(), &mirror).is_err());
        assert_eq!(
            contents(&dir(data.path())),
            [("todo-2026-11-19.db".to_owned(), "19".to_owned())]
        );
    }

    #[test]
    fn a_copy_that_fails_leaves_no_part_file() {
        let data = tempfile::tempdir().unwrap();
        let mirror = tempfile::tempdir().unwrap();
        put_files(&dir(data.path()), &[("todo-2026-11-19.db", "19")]);
        // A folder with the backup's name: the copy cannot take its name.
        std::fs::create_dir(mirror.path().join("todo-2026-11-19.db")).unwrap();
        assert!(sync_mirror(data.path(), mirror.path()).is_err());
        assert!(contents(mirror.path()).is_empty());
        assert_eq!(contents(&dir(data.path())).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn a_mirror_that_cannot_be_written_fails() {
        use std::os::unix::fs::PermissionsExt;
        let data = tempfile::tempdir().unwrap();
        let mirror = tempfile::tempdir().unwrap();
        put_files(&dir(data.path()), &[("todo-2026-11-19.db", "19")]);
        std::fs::set_permissions(mirror.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let result = sync_mirror(data.path(), mirror.path());
        std::fs::set_permissions(mirror.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(result.is_err());
        assert!(contents(mirror.path()).is_empty());
        assert_eq!(contents(&dir(data.path())).len(), 1);
    }

    #[tokio::test]
    async fn the_mirror_copies_real_backups_off_the_async_threads() {
        let data = tempfile::tempdir().unwrap();
        let pool = file_db(data.path()).await;
        add_project(&pool, "A").await;
        run_daily(&pool, data.path(), d("2026-11-19"))
            .await
            .unwrap();
        let usb = tempfile::tempdir().unwrap();
        let newest = mirror(data.path().to_owned(), usb.path().to_owned())
            .await
            .unwrap();
        assert_eq!(newest, Some(d("2026-11-19")));
        assert_eq!(
            std::fs::read(usb.path().join("todo-2026-11-19.db")).unwrap(),
            std::fs::read(dir(data.path()).join("todo-2026-11-19.db")).unwrap()
        );
    }

    #[test]
    fn daily_dates_are_newest_first_and_only_daily() {
        let data = tempfile::tempdir().unwrap();
        assert_eq!(daily_dates(data.path()).unwrap(), vec![]);
        put_files(
            &dir(data.path()),
            &[
                ("todo-2026-11-16.db", ""),
                ("todo-2026-11-19.db", ""),
                ("todo-2026-11-18.db.part", ""),
                ("before-restore-20261119T010000.000Z.db", ""),
            ],
        );
        assert_eq!(
            daily_dates(data.path()).unwrap(),
            vec![d("2026-11-19"), d("2026-11-16")]
        );
    }

    #[test]
    fn within_means_the_folder_or_inside_it() {
        let data = tempfile::tempdir().unwrap();
        let inside = data.path().join("backups");
        std::fs::create_dir(&inside).unwrap();
        let other = tempfile::tempdir().unwrap();
        assert!(is_within(data.path(), data.path()));
        assert!(is_within(&inside, data.path()));
        // Through `..` and in other letters.
        assert!(is_within(&inside.join(".."), data.path()));
        let upper = PathBuf::from(data.path().to_string_lossy().to_uppercase());
        if upper.exists() {
            assert!(is_within(&upper, data.path()));
        }
        assert!(!is_within(other.path(), data.path()));
        // The folder holding the data folder is not inside it.
        assert!(!is_within(data.path().parent().unwrap(), data.path()));
        // A folder that does not exist cannot be resolved.
        assert!(!is_within(&data.path().join("none"), data.path()));
    }

    // ---- ファイルを選んで復元 (v0.4.0) ----

    #[tokio::test]
    async fn a_file_outside_the_backups_folder_is_restored() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        add_project(&pool, "A").await;
        run_daily(&pool, tmp.path(), d("2026-11-18")).await.unwrap();
        // On the mirror of an old PC.
        let usb = tempfile::tempdir().unwrap();
        let chosen = usb.path().join("todo-2026-11-18.db");
        std::fs::copy(dir(tmp.path()).join("todo-2026-11-18.db"), &chosen).unwrap();
        let chosen_bytes = std::fs::read(&chosen).unwrap();
        add_project(&pool, "B").await;

        let kept = restore_file(&pool, tmp.path(), &chosen, ts(2026, 11, 19, 12, 0))
            .await
            .unwrap();
        assert_eq!(project_names(&pool).await, ["A"]);
        assert_eq!(kept, "before-restore-20261119T120000.000Z.db");
        assert!(dir(tmp.path()).join(&kept).is_file());
        // The chosen file is unchanged and nothing else was put next to it.
        assert_eq!(std::fs::read(&chosen).unwrap(), chosen_bytes);
        assert_eq!(std::fs::read_dir(usb.path()).unwrap().count(), 1);
        assert!(!dir(tmp.path()).join(RESTORE_WORK_FILE).exists());

        // A file in the backups folder can be chosen too (the kept copy).
        restore_file(
            &pool,
            tmp.path(),
            &dir(tmp.path()).join(&kept),
            ts(2026, 11, 19, 12, 5),
        )
        .await
        .unwrap();
        assert_eq!(project_names(&pool).await, ["A", "B"]);
    }

    #[tokio::test]
    async fn a_chosen_file_that_is_not_a_backup_changes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = file_db(tmp.path()).await;
        add_project(&pool, "A").await;
        let files = tempfile::tempdir().unwrap();
        let now = ts(2026, 11, 19, 12, 0);

        // Not a database.
        let text = files.path().join("todo-2026-11-17.db");
        std::fs::write(&text, b"not a database").unwrap();
        assert!(matches!(
            restore_file(&pool, tmp.path(), &text, now).await,
            Err(BackupError::Damaged)
        ));
        // A database, but not Higiri's.
        let other = files.path().join("other.db");
        let other_pool = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .filename(&other)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE notes (body TEXT)")
            .execute(&other_pool)
            .await
            .unwrap();
        other_pool.close().await;
        assert!(matches!(
            restore_file(&pool, tmp.path(), &other, now).await,
            Err(BackupError::Damaged)
        ));
        // Made by a newer version.
        run_daily(&pool, tmp.path(), d("2026-11-16")).await.unwrap();
        let newer = files.path().join("newer.db");
        std::fs::copy(dir(tmp.path()).join("todo-2026-11-16.db"), &newer).unwrap();
        let newer_pool = SqlitePool::connect_with(SqliteConnectOptions::new().filename(&newer))
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
             VALUES (99990101000000, 'future', 1, x'00', 0)",
        )
        .execute(&newer_pool)
        .await
        .unwrap();
        newer_pool.close().await;
        let newer_bytes = std::fs::read(&newer).unwrap();
        assert!(matches!(
            restore_file(&pool, tmp.path(), &newer, now).await,
            Err(BackupError::TooNew)
        ));
        // The chosen file was not migrated.
        assert_eq!(std::fs::read(&newer).unwrap(), newer_bytes);
        // Gone since it was chosen.
        assert!(matches!(
            restore_file(&pool, tmp.path(), &files.path().join("gone.db"), now).await,
            Err(BackupError::Source(_))
        ));
        // The database in use, and its -wal and -shm files (there while the
        // pool is open).
        for name in ["todo.db", "todo.db-wal", "todo.db-shm"] {
            let file = tmp.path().join(name);
            assert!(file.is_file(), "{name}");
            assert!(
                matches!(
                    restore_file(&pool, tmp.path(), &file, now).await,
                    Err(BackupError::InUse)
                ),
                "{name}"
            );
        }
        // Reached through `..` or with other letters (where the folder
        // ignores case, as on macOS and Windows).
        let around = tmp.path().join("backups").join("..").join("todo.db");
        assert!(matches!(
            restore_file(&pool, tmp.path(), &around, now).await,
            Err(BackupError::InUse)
        ));
        let upper = tmp.path().join("TODO.DB");
        if upper.exists() {
            assert!(matches!(
                restore_file(&pool, tmp.path(), &upper, now).await,
                Err(BackupError::InUse)
            ));
        }
        // Through a link.
        #[cfg(unix)]
        {
            let link = files.path().join("link.db");
            std::os::unix::fs::symlink(tmp.path().join("todo.db"), &link).unwrap();
            assert!(matches!(
                restore_file(&pool, tmp.path(), &link, now).await,
                Err(BackupError::InUse)
            ));
        }

        // Nothing was changed, and no copy of the current data was taken.
        assert_eq!(project_names(&pool).await, ["A"]);
        assert!(
            list(tmp.path())
                .unwrap()
                .iter()
                .all(|b| b.kind == BackupKind::Daily)
        );
        assert!(!dir(tmp.path()).join(RESTORE_WORK_FILE).exists());
    }
}
