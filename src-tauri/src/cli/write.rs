//! The write sub-commands (v0.5.0 phase 33): `add`, `update`, `status`,
//! `log`, `append-memo`.
//!
//! Each one goes through the inner function the screen uses for the same
//! change (`commands::todos::create`, `update`, `change_status_impl`,
//! `commands::work_logs::save`), so the checks, the history and what closes
//! or repeats with a TODO are the screen's. No SQL here.
//!
//! The order of each one: the TODOs of projects the CLI does not show are
//! refused (`hidden`), then what the screen would ask first
//! (`needs_confirmation`), then today's daily backup is taken if it is not
//! there yet (nothing is written when it cannot be), then the write. The
//! write's transaction waits up to 5 seconds while the app writes
//! (`repo::db::begin_write`); longer, it is refused as busy.

use std::io::Read;
use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::SqlitePool;

use super::args::{AddArgs, AppendMemoArgs, Command, FROM_STDIN, LogArgs, StatusArgs, UpdateArgs};
use super::error::CliError;
use super::output::{CliTodo, Hidden, MemoOut, NextRound, StatusOut, TodoRef, WorkLogAdded};
use super::read;
use crate::backup;
use crate::commands::todos::{self, NewTodoInput, TodoFields, TodoQuery};
use crate::commands::work_logs::{self, WorkLogInput};
use crate::domain::dependency::BlockingTodo;
use crate::domain::model::{Status, Todo};
use crate::domain::status::StatusChange;
use crate::domain::views::TodoItem;

/// Reads a `--body -` from `input` (standard input): UTF-8, without a BOM.
pub fn read_body(command: &mut Command, input: &mut impl Read) -> Result<(), CliError> {
    let Some(body) = command.body_mut() else {
        return Ok(());
    };
    if body != FROM_STDIN {
        return Ok(());
    }
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes).map_err(|_| {
        CliError::validation(
            "標準入力を読めませんでした。--body に本文を書いて、もう一度実行してください。",
        )
    })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        CliError::validation("標準入力の本文を読めませんでした。UTF-8 の文字で渡してください。")
    })?;
    *body = text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned();
    Ok(())
}

/// Line breaks as the screen's editor writes them.
fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Takes today's daily backup unless it is there (as the app does at its
/// first launch of the day).
async fn back_up(pool: &SqlitePool, data_dir: &Path, today: NaiveDate) -> Result<(), CliError> {
    backup::run_daily(pool, data_dir, today)
        .await
        .map(|_| ())
        .map_err(|_| CliError::no_backup())
}

/// The fields `update_todo` takes, as they are (the screen's `fieldsOf`).
fn fields_of(t: &Todo) -> TodoFields {
    TodoFields {
        title: t.title.clone(),
        priority: t.priority,
        due_date: t.due_date,
        planned_start: t.planned_start,
        planned_end: t.planned_end,
        actual_start: t.actual_start,
        actual_end: t.actual_end,
        waiting_on: t.waiting_on.clone(),
        follow_up_date: t.follow_up_date,
        close_reason: t.close_reason.clone(),
        memo_md: t.memo_md.clone(),
    }
}

/// A date set (`--due`), cleared (`--no-due`) or left as it is.
fn date_change(
    set: Option<NaiveDate>,
    clear: bool,
    current: Option<NaiveDate>,
) -> Option<NaiveDate> {
    if clear { None } else { set.or(current) }
}

/// Every TODO of the project (archived ones too), in the list's order.
async fn project_todos(
    pool: &SqlitePool,
    project_id: i64,
    today: NaiveDate,
) -> Result<Vec<TodoItem>, CliError> {
    let query = TodoQuery {
        project_id: Some(project_id),
        ..TodoQuery::default()
    };
    Ok(todos::list(pool, &query, today)
        .await?
        .rows
        .into_iter()
        .map(|r| r.item)
        .collect())
}

pub async fn add(
    pool: &SqlitePool,
    data_dir: &Path,
    args: &AddArgs,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<CliTodo, CliError> {
    let hidden = read::hidden(pool, today).await?;
    let project = read::project(pool, &args.project, today, &hidden).await?;
    if let Some(parent) = args.parent {
        read::shown_todo(pool, parent, today, &hidden).await?;
    }
    back_up(pool, data_dir, today).await?;
    let input = NewTodoInput {
        project_id: project.project.id,
        parent_id: args.parent,
        template_id: None,
        title: args.title.clone(),
        priority: args.priority,
        due_date: args.due,
        planned_start: args.start,
        planned_end: args.end,
        memo_md: None,
    };
    let item = todos::create(pool, input, today, now).await?;
    Ok(CliTodo::new(&item, &hidden))
}

pub async fn update(
    pool: &SqlitePool,
    data_dir: &Path,
    args: &UpdateArgs,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<CliTodo, CliError> {
    let hidden = read::hidden(pool, today).await?;
    let before = read::shown_todo(pool, args.id, today, &hidden).await?;
    if args.follow_up.is_some() && before.todo.status != Status::Waiting {
        return Err(CliError::validation(
            "確認日(--follow-up)は、待ちのTODOだけに入れられます。待ちにするときは higiri-cli status <id> waiting --waiting-on <相手> を使ってください。",
        ));
    }
    back_up(pool, data_dir, today).await?;
    // Only the fields named go over the TODO as it is just before the
    // write (the screen's useUpdateTodo).
    let t = todos::get(pool, args.id, today)
        .await
        .map_err(read::not_found(args.id))?
        .todo;
    let fields = TodoFields {
        title: args.title.clone().unwrap_or_else(|| t.title.clone()),
        priority: args.priority.unwrap_or(t.priority),
        due_date: date_change(args.due, args.no_due, t.due_date),
        planned_start: date_change(args.start, args.no_start, t.planned_start),
        planned_end: date_change(args.end, args.no_end, t.planned_end),
        follow_up_date: args.follow_up.or(t.follow_up_date),
        ..fields_of(&t)
    };
    let item = todos::update(pool, args.id, fields, today, now)
        .await
        .map_err(read::not_found(args.id))?;
    Ok(CliTodo::new(&item, &hidden))
}

/// A TODO to finish first, as the confirmation names it: 「親 / 子」 for a
/// child, nothing of a project the CLI does not show.
fn blocking_name(b: &BlockingTodo, hidden: &Hidden) -> String {
    if hidden.todo(b.id) {
        return "CLIに出さないプロジェクトのTODO".to_owned();
    }
    match &b.parent_title {
        Some(parent) => format!("{parent} / {}", b.title),
        None => b.title.clone(),
    }
}

pub async fn status(
    pool: &SqlitePool,
    data_dir: &Path,
    args: &StatusArgs,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<StatusOut, CliError> {
    if args.status != Status::Waiting && (args.waiting_on.is_some() || args.follow_up.is_some()) {
        return Err(CliError::validation(
            "--waiting-on と --follow-up は、waiting(待ち)にするときだけ使えます。",
        ));
    }
    if args.status != Status::Cancelled && args.reason.is_some() {
        return Err(CliError::validation(
            "--reason は、cancelled(中止)にするときだけ使えます。",
        ));
    }
    let hidden = read::hidden(pool, today).await?;
    let before = read::shown_todo(pool, args.id, today, &hidden).await?;
    let project_id = before.todo.project_id;
    let around = project_todos(pool, project_id, today).await?;
    let open_children: Vec<&Todo> = around
        .iter()
        .map(|i| &i.todo)
        .filter(|t| t.parent_id == Some(args.id) && !t.status.is_closed())
        .collect();

    // What the screen asks first, for a change it allows (a refused one is
    // refused by the write, with the screen's words).
    if before.next_statuses.contains(&args.status) {
        if args.status == Status::Doing && !before.blocked_by.is_empty() && !args.despite_blocked {
            let names: Vec<String> = before
                .blocked_by
                .iter()
                .map(|b| blocking_name(b, &hidden))
                .collect();
            return Err(CliError::blocked(&names));
        }
        if args.status.is_closed() && !open_children.is_empty() && !args.with_children {
            let names: Vec<String> = open_children.iter().map(|t| t.title.clone()).collect();
            return Err(CliError::open_children(&names, args.status.label()));
        }
    }

    back_up(pool, data_dir, today).await?;
    let change = StatusChange {
        to: args.status,
        waiting_on: args.waiting_on.clone(),
        // The screen's 「待ちにする」 offers this date.
        follow_up_date: (args.status == Status::Waiting)
            .then(|| args.follow_up.unwrap_or(before.follow_up_default)),
        reason: args.reason.clone(),
    };
    let item = todos::change_status_impl(pool, args.id, &change, today, now)
        .await
        .map_err(read::not_found(args.id))?;

    // What closed or was made with it: the project's TODOs before and after.
    let after = project_todos(pool, project_id, today).await?;
    let closed_children = open_children
        .iter()
        .filter_map(|c| {
            after
                .iter()
                .find(|a| a.todo.id == c.id && a.todo.status.is_closed())
                .map(|a| TodoRef {
                    id: a.todo.id,
                    title: a.todo.title.clone(),
                })
        })
        .collect();
    // The next round holds the repetition now.
    let next_round = after
        .iter()
        .find(|a| a.repeat.is_some() && around.iter().all(|b| b.todo.id != a.todo.id))
        .map(|a| NextRound {
            id: a.todo.id,
            due_date: a.todo.due_date,
        });
    let repeat_ended = before.repeat.is_some() && item.repeat.is_none() && next_round.is_none();
    Ok(StatusOut {
        todo: CliTodo::new(&item, &hidden),
        closed_children,
        next_round,
        repeat_ended,
    })
}

pub async fn log(
    pool: &SqlitePool,
    data_dir: &Path,
    args: &LogArgs,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<WorkLogAdded, CliError> {
    let hidden = read::hidden(pool, today).await?;
    read::shown_todo(pool, args.id, today, &hidden).await?;
    back_up(pool, data_dir, today).await?;
    let input = WorkLogInput {
        id: None,
        todo_id: args.id,
        log_date: args.date.unwrap_or(today),
        body_md: lf(&args.body),
        minutes: args.minutes,
    };
    let saved = work_logs::save(pool, input, now)
        .await
        .map_err(read::not_found(args.id))?;
    Ok(WorkLogAdded {
        id: saved.id,
        todo_id: saved.todo_id,
        log_date: saved.log_date,
        minutes: saved.minutes,
        body_md: saved.body_md,
    })
}

/// The memo with `body` after it: its trailing blanks removed, then an
/// empty line, then the text (only the text when the memo is empty).
pub fn appended(memo: &str, body: &str) -> String {
    let head = memo.trim_end();
    if head.is_empty() {
        body.to_owned()
    } else {
        format!("{head}\n\n{body}")
    }
}

pub async fn append_memo(
    pool: &SqlitePool,
    data_dir: &Path,
    args: &AppendMemoArgs,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<MemoOut, CliError> {
    let body = lf(&args.body);
    let body = body.trim();
    if body.is_empty() {
        return Err(CliError::validation("書き足す本文を入力してください。"));
    }
    let hidden = read::hidden(pool, today).await?;
    read::shown_todo(pool, args.id, today, &hidden).await?;
    back_up(pool, data_dir, today).await?;
    // The memo as it is just before the write; the other fields as they are.
    let t = todos::get(pool, args.id, today)
        .await
        .map_err(read::not_found(args.id))?
        .todo;
    let fields = TodoFields {
        memo_md: appended(&t.memo_md, body),
        ..fields_of(&t)
    };
    let item = todos::update(pool, args.id, fields, today, now)
        .await
        .map_err(read::not_found(args.id))?;
    Ok(MemoOut {
        memo_md: item.todo.memo_md.clone(),
        todo: CliTodo::new(&item, &hidden),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appending_leaves_one_empty_line() {
        assert_eq!(appended("", "追記"), "追記");
        assert_eq!(appended("  \n\n", "追記"), "追記");
        assert_eq!(appended("メモ", "追記"), "メモ\n\n追記");
        assert_eq!(appended("メモ\n", "追記"), "メモ\n\n追記");
        assert_eq!(appended("- a\n- b \n\n \n", "追記"), "- a\n- b\n\n追記");
        // The start of the memo stays as it is.
        assert_eq!(appended("\n  字下げ\n", "追記"), "\n  字下げ\n\n追記");
    }

    #[test]
    fn a_dash_body_is_read_from_standard_input() {
        use clap::Parser;
        let parse = |args: &[&str]| {
            super::super::args::Cli::try_parse_from(
                std::iter::once("higiri-cli").chain(args.iter().copied()),
            )
            .unwrap()
            .command
        };
        let mut c = parse(&["log", "3", "--body", "-"]);
        read_body(&mut c, &mut "\u{feff}- 1行目\n- 2行目\n".as_bytes()).unwrap();
        assert_eq!(c.body_mut().unwrap(), "- 1行目\n- 2行目\n");

        let mut c = parse(&["append-memo", "3", "--body", "-"]);
        read_body(&mut c, &mut "追記".as_bytes()).unwrap();
        assert_eq!(c.body_mut().unwrap(), "追記");

        // Not read unless the body is `-`.
        let mut c = parse(&["log", "3", "--body", "本文"]);
        read_body(&mut c, &mut "ほか".as_bytes()).unwrap();
        assert_eq!(c.body_mut().unwrap(), "本文");
        let mut c = parse(&["today"]);
        read_body(&mut c, &mut "ほか".as_bytes()).unwrap();

        let mut c = parse(&["log", "3", "--body", "-"]);
        let e = read_body(&mut c, &mut &[0xff, 0xfe, 0x00][..]).unwrap_err();
        assert_eq!(
            e,
            CliError::validation(
                "標準入力の本文を読めませんでした。UTF-8 の文字で渡してください。"
            )
        );
    }

    #[test]
    fn line_breaks_are_the_editors() {
        assert_eq!(lf("a\r\nb\r\n"), "a\nb\n");
        assert_eq!(lf("a\nb"), "a\nb");
    }
}
