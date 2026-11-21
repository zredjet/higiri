//! What higiri-cli writes: the TODO form (`CliTodo`) and the forms of the
//! sub-commands. Keys are English snake_case and values are stored ones
//! (dates `YYYY-MM-DD`, statuses `doing`), with the Japanese names beside
//! them. Day counts and the 日限注意 category are Rust's, never recounted.
//!
//! The memo and the work logs' text are in `show` only.

use std::collections::HashSet;

use chrono::NaiveDate;
use serde::Serialize;

use crate::domain::backup_mirror::BackupMirrorAlert;
use crate::domain::calendar::DayCount;
use crate::domain::dashboard::UrgencyCounts;
use crate::domain::date::format_timestamp;
use crate::domain::model::{Priority, Status};
use crate::domain::repeat::RepeatInfo;
use crate::domain::report::urgency_text;
use crate::domain::subtodo::ChildCounts;
use crate::domain::urgency::Urgency;
use crate::domain::views::{ProjectSummary, TodoItem};

/// The projects the CLI does not show, and their TODOs (for the other
/// projects' links to them).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hidden {
    pub projects: HashSet<i64>,
    pub todos: HashSet<i64>,
}

impl Hidden {
    pub fn project(&self, id: i64) -> bool {
        self.projects.contains(&id)
    }

    pub fn todo(&self, id: i64) -> bool {
        self.todos.contains(&id)
    }
}

/// `{ id, title }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TodoRef {
    pub id: i64,
    pub title: String,
}

/// `{ id, name }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectRef {
    pub id: i64,
    pub name: String,
}

/// Another TODO this one is linked to, or `{ "hidden": true }` when it is in
/// a project the CLI does not show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Linked<T> {
    Shown(T),
    Hidden { hidden: bool },
}

impl<T> Linked<T> {
    fn hidden() -> Linked<T> {
        Linked::Hidden { hidden: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliUrgency {
    pub kind: Urgency,
    /// 「超過」.
    pub label: &'static str,
    /// The daily report's 日限注意 parentheses: 「日限 11/18、2日超過」.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliWaiting {
    pub on: Option<String>,
    pub since: Option<NaiveDate>,
    pub follow_up_date: Option<NaiveDate>,
    /// Day of waiting, the first day being 1 (「待ち3日目」).
    pub day: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliWarning {
    pub kind: &'static str,
    pub label: &'static str,
}

/// A TODO as the CLI shows it (read sub-commands, and the TODO returned
/// after a write).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliTodo {
    pub id: i64,
    pub title: String,
    /// For a child: its parent.
    pub parent: Option<TodoRef>,
    pub project: ProjectRef,
    pub status: Status,
    pub status_label: &'static str,
    pub priority: Priority,
    pub priority_label: &'static str,
    pub due_date: Option<NaiveDate>,
    pub planned_start: Option<NaiveDate>,
    pub planned_end: Option<NaiveDate>,
    pub actual_start: Option<NaiveDate>,
    pub actual_end: Option<NaiveDate>,
    pub urgency: Option<CliUrgency>,
    /// Open TODOs with a 日限: days until it (negative when overdue).
    /// Business days when `business_days`.
    pub days_until_due: Option<i64>,
    pub business_days: bool,
    /// While 待ち.
    pub waiting: Option<CliWaiting>,
    pub close_reason: Option<String>,
    pub warnings: Vec<CliWarning>,
    /// The unfinished TODOs it waits for.
    pub blocked_by: Vec<Linked<TodoRef>>,
    /// For a parent: its children counted as the screen's 「子 2/3」.
    pub children: Option<ChildCounts>,
    pub repeat: Option<RepeatInfo>,
    /// UTC, as stored (`2026-11-19T01:30:00.000Z`).
    pub updated_at: String,
}

impl CliTodo {
    pub fn new(item: &TodoItem, hidden: &Hidden) -> CliTodo {
        let t = &item.todo;
        let a = &item.assessment;
        let mut warnings = Vec::new();
        let mut warn = |on: bool, kind, label| {
            if on {
                warnings.push(CliWarning { kind, label });
            }
        };
        // In the order of the list's warnings (todoText.ts warningTexts).
        warn(
            a.warnings.plan_misses_due,
            "plan_misses_due",
            "計画が間に合わない",
        );
        warn(a.warnings.plan_overrun, "plan_overrun", "予定超過");
        warn(item.due_after_parent, "due_after_parent", "親の日限より後");
        warn(
            item.starts_before_dependency,
            "starts_before_dependency",
            "先行TODOより前に始まる予定",
        );
        CliTodo {
            id: t.id,
            title: t.title.clone(),
            parent: t.parent_id.map(|id| TodoRef {
                id,
                title: item.parent_title.clone().unwrap_or_default(),
            }),
            project: ProjectRef {
                id: t.project_id,
                name: item.project_name.clone(),
            },
            status: t.status,
            status_label: t.status.label(),
            priority: t.priority,
            priority_label: t.priority.label(),
            due_date: t.due_date,
            planned_start: t.planned_start,
            planned_end: t.planned_end,
            actual_start: t.actual_start,
            actual_end: t.actual_end,
            urgency: a.urgency.map(|kind| CliUrgency {
                kind,
                label: kind.label(),
                text: urgency_text(t, a).unwrap_or_default(),
            }),
            days_until_due: a.days_until_due,
            business_days: a.business_days,
            waiting: (t.status == Status::Waiting).then(|| CliWaiting {
                on: t.waiting_on.clone(),
                since: t.waiting_since,
                follow_up_date: t.follow_up_date,
                day: a.waiting_day,
            }),
            close_reason: t.close_reason.clone(),
            warnings,
            blocked_by: item
                .blocked_by
                .iter()
                .map(|b| {
                    if hidden.todo(b.id) {
                        Linked::hidden()
                    } else {
                        Linked::Shown(TodoRef {
                            id: b.id,
                            title: b.title.clone(),
                        })
                    }
                })
                .collect(),
            children: item.children,
            repeat: item.repeat.clone(),
            updated_at: format_timestamp(t.updated_at),
        }
    }
}

/// The TODOs of a list, in order.
pub fn todos<'a>(items: impl IntoIterator<Item = &'a TodoItem>, hidden: &Hidden) -> Vec<CliTodo> {
    items.into_iter().map(|i| CliTodo::new(i, hidden)).collect()
}

// ---- today -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attention {
    pub total: i64,
    pub counts: UrgencyCounts,
    /// Most severe category first, then the oldest 日限 (the dashboard's).
    pub todos: Vec<CliTodo>,
}

/// A project's progress: 完了 of all but 中止, and its 日限注意.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub id: i64,
    pub name: String,
    pub progress_done: i64,
    pub progress_total: i64,
    pub attention_count: i64,
}

impl Progress {
    pub fn new(s: &ProjectSummary) -> Progress {
        Progress {
            id: s.project.id,
            name: s.project.name.clone(),
            progress_done: s.progress_done,
            progress_total: s.progress_total,
            attention_count: s.attention_count,
        }
    }
}

/// `today`: the dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Today {
    pub today: NaiveDate,
    pub day_count: DayCount,
    /// 「暦日」「営業日」.
    pub day_count_label: &'static str,
    /// 「間近」: the 日限 is within this many days (business days in 営業日).
    pub soon_days: u32,
    pub attention: Attention,
    /// 作業中, then 未着手 whose 予定期間 contains today.
    pub today_work: Vec<CliTodo>,
    /// The next 確認日 first.
    pub waiting: Vec<CliTodo>,
    /// The active projects.
    pub projects: Vec<Progress>,
    /// The daily backups have not reached the mirror for 3 days or more.
    pub backup_mirror_alert: Option<BackupMirrorAlert>,
}

pub fn day_count_label(d: DayCount) -> &'static str {
    match d {
        DayCount::Calendar => "暦日",
        DayCount::Business => "営業日",
    }
}

// ---- list, search, projects -------------------------------------------------------

/// `list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TodoListOut {
    /// At most `--limit`, parents followed by their children.
    pub todos: Vec<CliTodo>,
    /// Every TODO that matches.
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHitOut {
    pub todo: CliTodo,
    /// Where the words were found: `title`, `memo`, `waiting_on`,
    /// `close_reason`, `work_log` (in this order, each once).
    pub matched: Vec<&'static str>,
}

/// `search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchOut {
    /// At most 100, title hits first, then the most recently updated.
    pub hits: Vec<SearchHitOut>,
    /// Every TODO that contains every word.
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectOut {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub archived: bool,
    pub progress_done: i64,
    pub progress_total: i64,
    pub attention_count: i64,
}

impl ProjectOut {
    pub fn new(s: &ProjectSummary) -> ProjectOut {
        ProjectOut {
            id: s.project.id,
            name: s.project.name.clone(),
            description: s.project.description.clone(),
            archived: s.project.archived_at.is_some(),
            progress_done: s.progress_done,
            progress_total: s.progress_total,
            attention_count: s.attention_count,
        }
    }
}

/// `projects`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectsOut {
    /// In the side navigation's order.
    pub projects: Vec<ProjectOut>,
    /// How many projects of the list the CLI does not show.
    pub hidden_projects: i64,
}

// ---- show --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkLogOut {
    pub log_date: NaiveDate,
    pub minutes: Option<i64>,
    pub body_md: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChildOut {
    pub id: i64,
    pub title: String,
    pub status: Status,
    pub status_label: &'static str,
    pub due_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkedTodo {
    pub id: i64,
    pub title: String,
    pub status: Status,
    pub status_label: &'static str,
}

/// `show`: the TODO form with its memo, its newest work logs, its children
/// and the TODOs around it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShowOut {
    #[serde(flatten)]
    pub todo: CliTodo,
    pub memo_md: String,
    /// Newest first, at most `--logs`.
    pub work_logs: Vec<WorkLogOut>,
    /// Every work log of the TODO.
    pub work_logs_total: i64,
    /// Its children (`children` holds their counts).
    pub child_todos: Vec<ChildOut>,
    /// 先行TODO: the TODOs it waits for (finished ones too).
    pub depends_on: Vec<Linked<LinkedTodo>>,
    /// 後続TODO: the TODOs waiting for it.
    pub dependents: Vec<Linked<LinkedTodo>>,
}

/// A TODO around another one, unless it is in a project the CLI does not
/// show.
pub fn linked(item: &TodoItem, hidden: &Hidden) -> Linked<LinkedTodo> {
    if hidden.project(item.todo.project_id) {
        return Linked::hidden();
    }
    Linked::Shown(LinkedTodo {
        id: item.todo.id,
        title: item.todo.title.clone(),
        status: item.todo.status,
        status_label: item.todo.status.label(),
    })
}

// ---- writes (v0.5.0 phase 33) ---------------------------------------------------

/// The next round made when a repeating TODO closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NextRound {
    pub id: i64,
    pub due_date: Option<NaiveDate>,
}

/// `status`: the TODO and what changed with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusOut {
    pub todo: CliTodo,
    /// The open children closed with it, in the list's order.
    pub closed_children: Vec<TodoRef>,
    /// The next round, when it repeats and closed.
    pub next_round: Option<NextRound>,
    /// It repeated, closed, and no next round was made (past the end date).
    pub repeat_ended: bool,
}

/// `log`: the work log added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkLogAdded {
    pub id: i64,
    pub todo_id: i64,
    pub log_date: NaiveDate,
    pub minutes: Option<i64>,
    pub body_md: String,
}

/// `append-memo`: the TODO form with the memo as it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoOut {
    #[serde(flatten)]
    pub todo: CliTodo,
    pub memo_md: String,
}
