//! higiri-cli (v0.5.0): Higiri's TODOs as JSON, for Claude Code and the
//! like: read them, add them and change them (phase 33). The executable is
//! `src/bin/higiri-cli.rs`.
//!
//! - The database is the app's (`todo.db` in the app data folder, `dev/`
//!   with `--dev`), opened without creating it or applying migrations: its
//!   version must be this build's ([`crate::repo::db::open_existing`]).
//! - Everything goes through the inner functions of the screens' commands;
//!   there is no SQL here. The clock is read here, with `commands::clock`.
//! - Before a write, today's daily backup is taken unless it is there
//!   (`write`). The read sub-commands take none.
//! - Success: one JSON object on standard output, exit code 0. Refusal or
//!   failure: `{"error":{"kind","message"}}` on standard error, exit code 1.
//!   Wrong arguments: clap's message, exit code 2.

pub mod args;
pub mod error;
pub mod output;
pub mod read;
pub mod write;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use chrono::{DateTime, NaiveDate, Utc};
use clap::Parser;
use serde::Serialize;

use self::args::{Cli, Command};
use self::error::CliError;
use crate::commands::clock;
use crate::repo::db;

/// The app's identifier (`tauri.conf.json`): the app data folder's name.
pub const IDENTIFIER: &str = "com.redjet.higiri";

/// The folder holding `todo.db`, found as Tauri finds the app's
/// (`app_data_dir`): `%APPDATA%\com.redjet.higiri` on Windows,
/// `~/Library/Application Support/com.redjet.higiri` on macOS. `dev` is the
/// development builds' `dev` folder under it.
pub fn data_folder(dev: bool) -> Option<PathBuf> {
    let app = dirs::data_dir()?.join(IDENTIFIER);
    Some(if dev { app.join("dev") } else { app })
}

pub fn main() -> ExitCode {
    let mut cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // Help and version go to standard output with 0; errors to
            // standard error with 2.
            let _ = e.print();
            return ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(2));
        }
    };
    let result = match (
        write::read_body(&mut cli.command, &mut std::io::stdin().lock()),
        data_folder(cli.dev),
    ) {
        (Err(e), _) => Err(e),
        (Ok(()), None) => Err(CliError::no_data_folder()),
        (Ok(()), Some(dir)) => match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime.block_on(run(&cli.command, &dir, clock::today(), clock::now())),
            Err(_) => Err(CliError::internal()),
        },
    };
    let reply = Reply::of(result);
    // Nothing to do when the reader went away (a closed pipe).
    let _ = match &reply {
        Reply::Out(line) => writeln!(std::io::stdout().lock(), "{line}"),
        Reply::Error(line) => writeln!(std::io::stderr().lock(), "{line}"),
    };
    ExitCode::from(reply.code())
}

/// What the CLI writes, one line, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Reply {
    /// Standard output, exit code 0.
    Out(String),
    /// Standard error, exit code 1.
    Error(String),
}

impl Reply {
    fn of(result: Result<String, CliError>) -> Reply {
        match result {
            Ok(out) => Reply::Out(out),
            Err(e) => Reply::Error(e.to_line()),
        }
    }

    fn code(&self) -> u8 {
        match self {
            Reply::Out(_) => 0,
            Reply::Error(_) => 1,
        }
    }
}

/// Runs `command` on the database in `data_dir` as of `today` and `now`:
/// the JSON for standard output (keys in the order of the forms in
/// `output`), or the error for standard error. A `--body -` is already read
/// ([`write::read_body`]).
pub async fn run(
    command: &Command,
    data_dir: &Path,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<String, CliError> {
    run_on(
        command,
        &data_dir.join(db::DB_FILE_NAME),
        data_dir,
        today,
        now,
    )
    .await
}

/// [`run`] on the database file `db` (the performance check's may have
/// another name).
pub(crate) async fn run_on(
    command: &Command,
    db: &Path,
    data_dir: &Path,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<String, CliError> {
    let pool = db::open_existing(db).await?;
    let out = dispatch(&pool, command, data_dir, today, now).await;
    pool.close().await;
    out
}

async fn dispatch(
    pool: &sqlx::SqlitePool,
    command: &Command,
    data_dir: &Path,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<String, CliError> {
    match command {
        Command::Today => json(read::today_summary(pool, data_dir, today).await?),
        Command::List(args) => json(read::list(pool, args, today).await?),
        Command::Show(args) => json(read::show(pool, args, today).await?),
        Command::Search(args) => json(read::search_todos(pool, &args.words, today).await?),
        Command::Projects(args) => json(read::project_list(pool, args.archived, today).await?),
        Command::Add(args) => json(write::add(pool, data_dir, args, today, now).await?),
        Command::Update(args) => json(write::update(pool, data_dir, args, today, now).await?),
        Command::Status(args) => json(write::status(pool, data_dir, args, today, now).await?),
        Command::Log(args) => json(write::log(pool, data_dir, args, today, now).await?),
        Command::AppendMemo(args) => {
            json(write::append_memo(pool, data_dir, args, today, now).await?)
        }
    }
}

fn json(out: impl Serialize) -> Result<String, CliError> {
    serde_json::to_string(&out).map_err(|_| CliError::internal())
}

#[cfg(test)]
mod tests;
