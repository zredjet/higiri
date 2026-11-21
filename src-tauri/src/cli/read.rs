//! The read sub-commands: `today`, `list`, `show`, `search`, `projects`.
//!
//! Each one calls the inner functions of the screens' commands (no SQL
//! here) and turns what they return into the CLI's forms. The projects the
//! CLI does not show (`cli_hidden_projects`) are left out everywhere: before
//! counting and ordering in `today` and `search`, and refused when named.

use std::path::Path;

use chrono::NaiveDate;
use sqlx::SqlitePool;

use super::args::{ListArgs, ShowArgs};
use super::error::CliError;
use super::output::{
    self, Attention, ChildOut, CliTodo, Hidden, Progress, ProjectOut, ProjectsOut, SearchHitOut,
    SearchOut, ShowOut, Today, TodoListOut, WorkLogOut,
};
use crate::commands::error::ErrorKind;
use crate::commands::todos::TodoQuery;
use crate::commands::work_logs::TimelineEntry;
use crate::commands::{dashboard, dependencies, projects, search, settings, todos, work_logs};
use crate::domain::calendar::DayCount;
use crate::domain::search::{MatchField, SearchHit};
use crate::domain::views::ProjectSummary;

/// The projects the CLI does not show and their TODOs.
pub async fn hidden(pool: &SqlitePool, today: NaiveDate) -> Result<Hidden, CliError> {
    let mut out = Hidden {
        projects: settings::get(pool)
            .await?
            .cli_hidden_projects
            .into_iter()
            .collect(),
        ..Hidden::default()
    };
    // Every TODO of each hidden project (archived ones too), for the other
    // projects' TODOs that wait for them or that they wait for.
    for &project_id in &out.projects {
        let query = TodoQuery {
            project_id: Some(project_id),
            ..TodoQuery::default()
        };
        let list = todos::list(pool, &query, today).await?;
        out.todos.extend(list.rows.iter().map(|r| r.item.todo.id));
    }
    Ok(out)
}

/// The project `name_or_id` names (archived ones too): its id when it is a
/// number and a project has it, else the one project with exactly that name.
pub async fn project(
    pool: &SqlitePool,
    name_or_id: &str,
    today: NaiveDate,
    hidden: &Hidden,
) -> Result<ProjectSummary, CliError> {
    let all = projects::list(pool, true, today).await?;
    let wanted = name_or_id.trim();
    let by_id = wanted
        .parse::<i64>()
        .ok()
        .and_then(|id| all.iter().find(|s| s.project.id == id));
    let found = match by_id {
        Some(s) => s.clone(),
        None => {
            let mut named = all.iter().filter(|s| s.project.name == wanted);
            match (named.next(), named.next()) {
                (Some(s), None) => s.clone(),
                (None, _) => {
                    return Err(CliError::not_found(format!(
                        "プロジェクト「{wanted}」が見つかりません。higiri-cli projects で名前かidを確かめてください。"
                    )));
                }
                (Some(_), Some(_)) => {
                    return Err(CliError::validation(format!(
                        "「{wanted}」という名前のプロジェクトが2つ以上あります。higiri-cli projects でidを確かめて、idで指してください。"
                    )));
                }
            }
        }
    };
    if hidden.project(found.project.id) {
        return Err(CliError::hidden_project());
    }
    Ok(found)
}

pub async fn today_summary(
    pool: &SqlitePool,
    data_dir: &Path,
    today: NaiveDate,
) -> Result<Today, CliError> {
    let hidden = hidden(pool, today).await?;
    let left_out: Vec<i64> = hidden.projects.iter().copied().collect();
    let dash = dashboard::without_projects(pool, data_dir, today, &left_out).await?;
    let day_count = if dash.business_days {
        DayCount::Business
    } else {
        DayCount::Calendar
    };
    Ok(Today {
        today: dash.today,
        day_count,
        day_count_label: output::day_count_label(day_count),
        soon_days: dash.soon_days,
        attention: Attention {
            total: dash.attention_total,
            counts: dash.counts,
            todos: output::todos(&dash.attention, &hidden),
        },
        today_work: output::todos(&dash.today_work, &hidden),
        waiting: output::todos(&dash.waiting, &hidden),
        projects: dash.projects.iter().map(Progress::new).collect(),
        backup_mirror_alert: dash.backup_mirror_alert,
    })
}

/// The list as the screen filters and orders it, without the rows that
/// only hold matching children (their children carry `parent`).
pub async fn list(
    pool: &SqlitePool,
    args: &ListArgs,
    today: NaiveDate,
) -> Result<TodoListOut, CliError> {
    let hidden = hidden(pool, today).await?;
    let project_id = match &args.project {
        Some(p) => Some(project(pool, p, today, &hidden).await?.project.id),
        None => None,
    };
    let query = TodoQuery {
        project_id,
        statuses: (!args.status.is_empty()).then(|| args.status.clone()),
        priority: args.priority,
        due: args.due,
        sort: args.sort,
    };
    let list = todos::list(pool, &query, today).await?;
    let matching: Vec<_> = list
        .rows
        .iter()
        .filter(|r| !r.bridge && !hidden.project(r.item.todo.project_id))
        .map(|r| &r.item)
        .collect();
    Ok(TodoListOut {
        total: matching.len() as i64,
        todos: output::todos(matching.into_iter().take(args.limit as usize), &hidden),
    })
}

pub async fn show(
    pool: &SqlitePool,
    args: &ShowArgs,
    today: NaiveDate,
) -> Result<ShowOut, CliError> {
    let item = todos::get(pool, args.id, today)
        .await
        .map_err(|e| match e.kind {
            ErrorKind::NotFound => CliError::todo_not_found(args.id),
            _ => e.into(),
        })?;
    let hidden = hidden(pool, today).await?;
    if hidden.project(item.todo.project_id) {
        return Err(CliError::hidden_todo());
    }
    let logs: Vec<_> = work_logs::timeline(pool, args.id)
        .await?
        .into_iter()
        .filter_map(|e| match e {
            TimelineEntry::WorkLog(l) => Some(l),
            TimelineEntry::Event(_) => None,
        })
        .collect();
    let child_todos = if item.children.is_some() {
        let query = TodoQuery {
            project_id: Some(item.todo.project_id),
            ..TodoQuery::default()
        };
        todos::list(pool, &query, today)
            .await?
            .rows
            .iter()
            .map(|r| &r.item.todo)
            .filter(|t| t.parent_id == Some(item.todo.id))
            .map(|t| ChildOut {
                id: t.id,
                title: t.title.clone(),
                status: t.status,
                status_label: t.status.label(),
                due_date: t.due_date,
            })
            .collect()
    } else {
        Vec::new()
    };
    let around = dependencies::list(pool, args.id, today).await?;
    Ok(ShowOut {
        todo: CliTodo::new(&item, &hidden),
        memo_md: item.todo.memo_md.clone(),
        work_logs_total: logs.len() as i64,
        work_logs: logs
            .into_iter()
            .take(args.logs as usize)
            .map(|l| WorkLogOut {
                log_date: l.log_date,
                minutes: l.minutes,
                body_md: l.body_md,
            })
            .collect(),
        child_todos,
        depends_on: around
            .depends_on
            .iter()
            .map(|i| output::linked(i, &hidden))
            .collect(),
        dependents: around
            .dependents
            .iter()
            .map(|i| output::linked(i, &hidden))
            .collect(),
    })
}

/// Where a hit was found, without the text around it (the memo and the
/// work logs are not shown in a list).
fn matched(hit: &SearchHit) -> Vec<&'static str> {
    let mut out = Vec::new();
    if hit.title.iter().any(|s| s.hit) {
        out.push("title");
    }
    for m in &hit.matches {
        let name = match m.field {
            MatchField::Memo => "memo",
            MatchField::WaitingOn => "waiting_on",
            MatchField::CloseReason => "close_reason",
            MatchField::WorkLog => "work_log",
        };
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

pub async fn search_todos(
    pool: &SqlitePool,
    words: &[String],
    today: NaiveDate,
) -> Result<SearchOut, CliError> {
    let hidden = hidden(pool, today).await?;
    let left_out: Vec<i64> = hidden.projects.iter().copied().collect();
    let found = search::search_without_projects(pool, &words.join(" "), today, &left_out).await?;
    Ok(SearchOut {
        hits: found
            .hits
            .iter()
            .map(|h| SearchHitOut {
                todo: CliTodo::new(&h.item, &hidden),
                matched: matched(h),
            })
            .collect(),
        total: found.total,
    })
}

pub async fn project_list(
    pool: &SqlitePool,
    archived: bool,
    today: NaiveDate,
) -> Result<ProjectsOut, CliError> {
    let hidden = hidden(pool, today).await?;
    let all = projects::list(pool, archived, today).await?;
    let (shown, left_out): (Vec<_>, Vec<_>) =
        all.iter().partition(|s| !hidden.project(s.project.id));
    Ok(ProjectsOut {
        projects: shown.into_iter().map(ProjectOut::new).collect(),
        hidden_projects: left_out.len() as i64,
    })
}
