//! 一括変更 (T-14, v0.4.0). Pure.
//!
//! The TODOs selected in the list get one change: the 日限 (a date, or moved
//! by N days), the 予定期間 moved by N days, the 優先度, or the status. What
//! cannot take the change is left out with the reason, and the rest change
//! ([`plan`]). The confirmation (`preview_bulk_change`) and the save
//! (`apply_bulk_change`) both use it, the save on the data as it is then.
//!
//! - Dates are not changed on closed TODOs. Days are calendar days (as the
//!   Gantt drag counts them), whatever 日数の数え方 is set. A 予定期間 with one
//!   end moves that end only. Moving a 予定期間 moves no 後続TODO (G-8 is the
//!   Gantt drag's).
//! - The 優先度 changes on closed TODOs too.
//! - A status goes through the transition table one TODO at a time, as
//!   `change_status` does ([`transition`]'s message is the reason). Closing a
//!   parent closes its open children: one that cannot close keeps the parent
//!   out. A selected child whose parent closes is closed with it (counted in
//!   `closing_children`, not on its own). 待ち is not offered (who and when
//!   differ for each TODO).
//! - Nothing is stopped by a dependency: the TODOs going 作業中 that wait for
//!   unfinished TODOs are only listed (`blocked`), as the other ways to
//!   作業中 ask first.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};

use super::dependency::{BlockingTodo, Dependencies};
use super::model::{Priority, Status, Todo};
use super::repeat::{RepeatsByTodo, shift};
use super::status::{StatusChange, StatusFields, TransitionError, transition};
use super::urgency::Rules;

/// The change given to every selected TODO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BulkChange {
    /// 日限を日付で決める. `null` is refused (the 日限 is not emptied).
    SetDue {
        date: Option<NaiveDate>,
    },
    /// 日限をN日ずらす (−365〜365, not 0).
    ShiftDue {
        days: i64,
    },
    /// 予定期間をN日ずらす (−365〜365, not 0).
    ShiftPlanned {
        days: i64,
    },
    Priority {
        priority: Priority,
    },
    /// One of [`BULK_STATUSES`]. 中止 needs the reason, given to every TODO.
    Status {
        status: Status,
        reason: Option<String>,
    },
}

/// Most days a date moves either way.
pub const SHIFT_DAYS_MAX: i64 = 365;

/// The statuses the bulk change offers, in the screen's order (not 待ち).
pub const BULK_STATUSES: [Status; 5] = [
    Status::Todo,
    Status::Doing,
    Status::Hold,
    Status::Done,
    Status::Cancelled,
];

/// What is wrong with the input. The text is shown under the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BulkInputError {
    #[error("日限を入力してください。")]
    DateRequired,

    #[error("ずらす日数を、0以外の整数で入力してください。")]
    ZeroDays,

    #[error("ずらす日数は-365〜365の範囲で入力してください。")]
    DaysOutOfRange,

    #[error(
        "「待ち」には一括で変えられません。待っている相手と確認日を、1件ずつ詳細パネルで入力してください。"
    )]
    Waiting,

    #[error("中止の理由を入力してください。")]
    ReasonRequired,
}

impl BulkChange {
    /// The change with its input checked: the reason trimmed (and dropped
    /// unless 中止).
    pub fn checked(self) -> Result<BulkChange, BulkInputError> {
        match self {
            BulkChange::SetDue { date: None } => Err(BulkInputError::DateRequired),
            BulkChange::ShiftDue { days } | BulkChange::ShiftPlanned { days } if days == 0 => {
                Err(BulkInputError::ZeroDays)
            }
            BulkChange::ShiftDue { days } | BulkChange::ShiftPlanned { days }
                if days.abs() > SHIFT_DAYS_MAX =>
            {
                Err(BulkInputError::DaysOutOfRange)
            }
            BulkChange::Status { status, reason } => {
                if !BULK_STATUSES.contains(&status) {
                    return Err(BulkInputError::Waiting);
                }
                if status != Status::Cancelled {
                    return Ok(BulkChange::Status {
                        status,
                        reason: None,
                    });
                }
                let reason = reason
                    .as_deref()
                    .map(str::trim)
                    .filter(|r| !r.is_empty())
                    .ok_or(BulkInputError::ReasonRequired)?;
                Ok(BulkChange::Status {
                    status,
                    reason: Some(reason.to_owned()),
                })
            }
            other => Ok(other),
        }
    }

    /// The status change each TODO goes through, for a status change.
    pub fn status_change(&self) -> Option<StatusChange> {
        match self {
            BulkChange::Status { status, reason } => Some(StatusChange {
                to: *status,
                waiting_on: None,
                follow_up_date: None,
                reason: reason.clone(),
            }),
            _ => None,
        }
    }
}

/// Why a selected TODO is left out. The text is shown next to its name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkipReason {
    #[error("完了・中止したTODOの日付は変えません。")]
    Closed,

    #[error("すでにその日限です。")]
    SameDue,

    #[error("日限がありません。")]
    NoDue,

    #[error("予定期間がありません。")]
    NoPlan,

    #[error("すでにその優先度です。")]
    SamePriority,

    /// The transition table refuses it.
    #[error("{0}")]
    Status(TransitionError),

    /// An open child cannot be closed with it (as `change_status` says it).
    #[error("子TODO「{title}」を{}にできません。{error}", .to.label())]
    Child {
        title: String,
        to: Status,
        error: TransitionError,
    },
}

/// What the TODOs of a bulk change need besides themselves.
#[derive(Debug, Clone, Copy)]
pub struct BulkContext<'a> {
    /// The children of the selected TODOs (any status): closing a parent
    /// closes the open ones. Only read when closing.
    pub children: &'a [Todo],
    /// The repetitions of the selected TODOs (`next_rounds`).
    pub repeats: &'a RepeatsByTodo,
    /// What the selected TODOs wait for (`blocked`).
    pub dependencies: &'a Dependencies,
}

/// What a bulk change does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BulkPlan {
    /// The TODOs to change, parents before children (the order of the save).
    pub targets: Vec<i64>,
    /// For the dates and the 優先度: each target as it is to be saved, in
    /// the order of `targets`. Empty for a status.
    pub edits: Vec<Todo>,
    /// The TODOs left out, in the order selected.
    pub skipped: Vec<(i64, SkipReason)>,
    /// To 作業中: the targets that wait for unfinished TODOs, with those.
    pub blocked: Vec<(i64, Vec<BlockingTodo>)>,
    /// To 完了・中止: the open children closed with their parent (selected
    /// or not), parent by parent.
    pub closing_children: Vec<i64>,
    /// To 完了・中止: the repeating targets that make their next round (not
    /// those whose repetition ends here).
    pub next_rounds: usize,
}

/// What `change` (checked) does to the `selected` TODOs (each once, in the
/// list's order) today.
pub fn plan(
    change: &BulkChange,
    selected: &[Todo],
    context: BulkContext<'_>,
    today: NaiveDate,
    rules: &Rules,
) -> BulkPlan {
    match change.status_change() {
        Some(status) => plan_status(&status, selected, context, today, rules),
        None => plan_edits(change, selected),
    }
}

/// The dates and the 優先度: each TODO by itself.
fn plan_edits(change: &BulkChange, selected: &[Todo]) -> BulkPlan {
    let mut out = BulkPlan::default();
    for t in selected {
        match edited(t, change) {
            Ok(after) => {
                out.targets.push(t.id);
                out.edits.push(after);
            }
            Err(reason) => out.skipped.push((t.id, reason)),
        }
    }
    out
}

/// `todo` with a date or 優先度 change, or why it is left out.
fn edited(todo: &Todo, change: &BulkChange) -> Result<Todo, SkipReason> {
    let mut after = todo.clone();
    let dates = matches!(
        change,
        BulkChange::SetDue { .. } | BulkChange::ShiftDue { .. } | BulkChange::ShiftPlanned { .. }
    );
    if dates && todo.status.is_closed() {
        return Err(SkipReason::Closed);
    }
    match *change {
        BulkChange::SetDue { date } => {
            if todo.due_date == date {
                return Err(SkipReason::SameDue);
            }
            after.due_date = date;
        }
        BulkChange::ShiftDue { days } => {
            let due = todo.due_date.ok_or(SkipReason::NoDue)?;
            after.due_date = shift(Some(due), days).or(Some(due));
        }
        BulkChange::ShiftPlanned { days } => {
            if todo.planned_start.is_none() && todo.planned_end.is_none() {
                return Err(SkipReason::NoPlan);
            }
            after.planned_start = shift(todo.planned_start, days).or(todo.planned_start);
            after.planned_end = shift(todo.planned_end, days).or(todo.planned_end);
        }
        BulkChange::Priority { priority } => {
            if todo.priority == priority {
                return Err(SkipReason::SamePriority);
            }
            after.priority = priority;
        }
        BulkChange::Status { .. } => unreachable!("a status goes through plan_status"),
    }
    Ok(after)
}

/// The status: the transition table, one TODO at a time.
fn plan_status(
    change: &StatusChange,
    selected: &[Todo],
    context: BulkContext<'_>,
    today: NaiveDate,
    rules: &Rules,
) -> BulkPlan {
    let to = change.to;
    let closing = to.is_closed();
    // The time is only written into 完了・中止 (closed_at); the checks do not
    // read it.
    let now = placeholder_now(today);
    let passes = |t: &Todo| transition(&StatusFields::of(t), change, today, now).map(|_| ());
    // The parent's 中止 reason goes to its children; nothing about 待ち.
    let for_children = StatusChange {
        to,
        waiting_on: None,
        follow_up_date: None,
        reason: change.reason.clone(),
    };

    let mut children: HashMap<i64, Vec<&Todo>> = HashMap::new();
    if closing {
        for c in context.children {
            if let Some(parent) = c.parent_id {
                children.entry(parent).or_default().push(c);
            }
        }
    }
    let open_children = |t: &Todo| -> Vec<&Todo> {
        children
            .get(&t.id)
            .map(|cs| cs.iter().copied().filter(|c| c.status.is_open()).collect())
            .unwrap_or_default()
    };

    // Each one's own verdict; a parent that closes also needs every open
    // child to close.
    let verdicts: Vec<(&Todo, Result<(), SkipReason>)> = selected
        .iter()
        .map(|t| {
            let own = passes(t).map_err(SkipReason::Status).and_then(|()| {
                for c in open_children(t) {
                    transition(&StatusFields::of(c), &for_children, today, now).map_err(
                        |error| SkipReason::Child {
                            title: c.title.clone(),
                            to,
                            error,
                        },
                    )?;
                }
                Ok(())
            });
            (t, own)
        })
        .collect();

    let mut out = BulkPlan::default();
    if closing {
        for (t, v) in &verdicts {
            if v.is_ok() {
                out.closing_children
                    .extend(open_children(t).into_iter().map(|c| c.id));
            }
        }
    }
    let closed_with_parent: HashSet<i64> = out.closing_children.iter().copied().collect();
    let mut targets: Vec<&Todo> = Vec::new();
    for (t, v) in verdicts {
        if closed_with_parent.contains(&t.id) {
            continue;
        }
        match v {
            Ok(()) => targets.push(t),
            Err(reason) => out.skipped.push((t.id, reason)),
        }
    }
    // Parents first (the children of a parent closed with it are not
    // targets; reopening both changes the parent first).
    targets.sort_by_key(|t| t.parent_id.is_some());

    for t in &targets {
        if to == Status::Doing {
            let blocking = context.dependencies.blocked_by(t);
            if !blocking.is_empty() {
                out.blocked.push((t.id, blocking));
            }
        }
        if closing
            && context
                .repeats
                .get(&t.id)
                .is_some_and(|r| r.next_round_due(t.due_date, today, rules).is_some())
        {
            out.next_rounds += 1;
        }
    }
    out.targets = targets.iter().map(|t| t.id).collect();
    out
}

fn placeholder_now(today: NaiveDate) -> DateTime<Utc> {
    today.and_time(NaiveTime::MIN).and_utc()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dependency::Prerequisite;
    use crate::domain::repeat::{RepeatRule, TodoRepeat};
    use crate::domain::testing::{d, rules, todo, ts};

    fn today() -> NaiveDate {
        d("2026-11-19")
    }

    fn t(id: i64, status: Status) -> Todo {
        Todo {
            id,
            title: format!("T{id}"),
            ..todo(status)
        }
    }

    fn child(id: i64, parent: i64, status: Status) -> Todo {
        Todo {
            parent_id: Some(parent),
            ..t(id, status)
        }
    }

    fn run(change: BulkChange, selected: &[Todo]) -> BulkPlan {
        run_with(
            change,
            selected,
            &[],
            &RepeatsByTodo::new(),
            &Dependencies::default(),
        )
    }

    fn run_with(
        change: BulkChange,
        selected: &[Todo],
        children: &[Todo],
        repeats: &RepeatsByTodo,
        dependencies: &Dependencies,
    ) -> BulkPlan {
        let change = change.checked().unwrap();
        plan(
            &change,
            selected,
            BulkContext {
                children,
                repeats,
                dependencies,
            },
            today(),
            &rules(3),
        )
    }

    fn reasons(p: &BulkPlan) -> Vec<(i64, String)> {
        p.skipped
            .iter()
            .map(|(id, r)| (*id, r.to_string()))
            .collect()
    }

    fn status(to: Status) -> BulkChange {
        BulkChange::Status {
            status: to,
            reason: None,
        }
    }

    // ---- input ------------------------------------------------------------

    #[test]
    fn the_input_is_checked() {
        use BulkInputError::*;
        let bad = |c: BulkChange| c.checked().unwrap_err();
        assert_eq!(bad(BulkChange::SetDue { date: None }), DateRequired);
        assert_eq!(bad(BulkChange::ShiftDue { days: 0 }), ZeroDays);
        assert_eq!(bad(BulkChange::ShiftPlanned { days: 0 }), ZeroDays);
        assert_eq!(bad(BulkChange::ShiftDue { days: 366 }), DaysOutOfRange);
        assert_eq!(bad(BulkChange::ShiftPlanned { days: -366 }), DaysOutOfRange);
        assert_eq!(bad(status(Status::Waiting)), Waiting);
        assert_eq!(bad(status(Status::Cancelled)), ReasonRequired);
        assert_eq!(
            bad(BulkChange::Status {
                status: Status::Cancelled,
                reason: Some(" \n".into())
            }),
            ReasonRequired
        );
        assert_eq!(
            ZeroDays.to_string(),
            "ずらす日数を、0以外の整数で入力してください。"
        );
        assert_eq!(
            DaysOutOfRange.to_string(),
            "ずらす日数は-365〜365の範囲で入力してください。"
        );

        for ok in [365, -365, 1, -1] {
            assert!(BulkChange::ShiftDue { days: ok }.checked().is_ok());
            assert!(BulkChange::ShiftPlanned { days: ok }.checked().is_ok());
        }
        assert_eq!(
            BulkChange::Status {
                status: Status::Cancelled,
                reason: Some("  不要になったため ".into())
            }
            .checked()
            .unwrap(),
            BulkChange::Status {
                status: Status::Cancelled,
                reason: Some("不要になったため".into())
            }
        );
        // The reason is only for 中止.
        assert_eq!(
            BulkChange::Status {
                status: Status::Done,
                reason: Some("x".into())
            }
            .checked()
            .unwrap(),
            status(Status::Done)
        );
    }

    #[test]
    fn the_change_reads_from_json_by_kind() {
        let c: BulkChange = serde_json::from_str(r#"{"kind":"shift_due","days":-3}"#).unwrap();
        assert_eq!(c, BulkChange::ShiftDue { days: -3 });
        let c: BulkChange =
            serde_json::from_str(r#"{"kind":"set_due","date":"2026-11-30"}"#).unwrap();
        assert_eq!(
            c,
            BulkChange::SetDue {
                date: Some(d("2026-11-30"))
            }
        );
        let c: BulkChange =
            serde_json::from_str(r#"{"kind":"status","status":"done","reason":null}"#).unwrap();
        assert_eq!(c, status(Status::Done));
    }

    // ---- dates and 優先度 ---------------------------------------------------

    #[test]
    fn set_due_leaves_out_closed_todos_and_the_same_date() {
        let due = |id, status, date: Option<&str>| Todo {
            due_date: date.map(d),
            ..t(id, status)
        };
        let selected = [
            due(1, Status::Todo, Some("2026-11-20")),
            due(2, Status::Done, Some("2026-11-20")),
            due(3, Status::Doing, Some("2026-11-30")),
            due(4, Status::Hold, None),
            due(5, Status::Cancelled, None),
        ];
        let p = run(
            BulkChange::SetDue {
                date: Some(d("2026-11-30")),
            },
            &selected,
        );
        assert_eq!(p.targets, vec![1, 4]);
        assert_eq!(
            p.edits.iter().map(|e| e.due_date).collect::<Vec<_>>(),
            vec![Some(d("2026-11-30")); 2]
        );
        assert_eq!(
            reasons(&p),
            vec![
                (2, "完了・中止したTODOの日付は変えません。".into()),
                (3, "すでにその日限です。".into()),
                (5, "完了・中止したTODOの日付は変えません。".into()),
            ]
        );
    }

    #[test]
    fn shift_due_moves_by_calendar_days_across_months_and_years() {
        let due = |id, date: Option<&str>| Todo {
            due_date: date.map(d),
            ..t(id, Status::Todo)
        };
        let selected = [
            due(1, Some("2026-11-29")),
            due(2, Some("2026-12-30")),
            due(3, None),
        ];
        let p = run(BulkChange::ShiftDue { days: 3 }, &selected);
        assert_eq!(p.targets, vec![1, 2]);
        assert_eq!(p.edits[0].due_date, Some(d("2026-12-02")));
        assert_eq!(p.edits[1].due_date, Some(d("2027-01-02")));
        assert_eq!(reasons(&p), vec![(3, "日限がありません。".into())]);

        let p = run(BulkChange::ShiftDue { days: -30 }, &selected[..1]);
        assert_eq!(p.edits[0].due_date, Some(d("2026-10-30")));
        // Nothing else changes.
        assert_eq!(
            Todo {
                due_date: selected[0].due_date,
                ..p.edits[0].clone()
            },
            selected[0]
        );
    }

    #[test]
    fn shift_planned_moves_what_is_set() {
        let plan_of = |id, start: Option<&str>, end: Option<&str>| Todo {
            planned_start: start.map(d),
            planned_end: end.map(d),
            ..t(id, Status::Doing)
        };
        let selected = [
            plan_of(1, Some("2026-11-17"), Some("2026-11-20")),
            plan_of(2, Some("2026-11-17"), None),
            plan_of(3, None, Some("2026-11-20")),
            plan_of(4, None, None),
            Todo {
                status: Status::Done,
                ..plan_of(5, Some("2026-11-17"), Some("2026-11-20"))
            },
        ];
        let p = run(BulkChange::ShiftPlanned { days: -2 }, &selected);
        assert_eq!(p.targets, vec![1, 2, 3]);
        let spans: Vec<_> = p
            .edits
            .iter()
            .map(|e| (e.planned_start, e.planned_end))
            .collect();
        assert_eq!(
            spans,
            vec![
                (Some(d("2026-11-15")), Some(d("2026-11-18"))),
                (Some(d("2026-11-15")), None),
                (None, Some(d("2026-11-18"))),
            ]
        );
        assert_eq!(
            reasons(&p),
            vec![
                (4, "予定期間がありません。".into()),
                (5, "完了・中止したTODOの日付は変えません。".into()),
            ]
        );
        // The 日限 stays.
        assert!(p.edits.iter().all(|e| e.due_date.is_none()));
    }

    #[test]
    fn priority_changes_closed_todos_too() {
        let with = |id, status, priority| Todo {
            priority,
            ..t(id, status)
        };
        let selected = [
            with(1, Status::Todo, Priority::Medium),
            with(2, Status::Done, Priority::Low),
            with(3, Status::Cancelled, Priority::High),
        ];
        let p = run(
            BulkChange::Priority {
                priority: Priority::High,
            },
            &selected,
        );
        assert_eq!(p.targets, vec![1, 2]);
        assert!(p.edits.iter().all(|e| e.priority == Priority::High));
        assert_eq!(reasons(&p), vec![(3, "すでにその優先度です。".into())]);
    }

    // ---- status -------------------------------------------------------------

    #[test]
    fn a_status_goes_through_the_transition_table() {
        let selected = [
            t(1, Status::Todo),
            t(2, Status::Done),
            t(3, Status::Cancelled),
            t(4, Status::Hold),
            t(5, Status::Waiting),
        ];
        let p = run(status(Status::Todo), &selected);
        assert_eq!(p.targets, vec![3, 4, 5]);
        assert_eq!(
            reasons(&p),
            vec![
                (1, "すでに「未着手」です。".into()),
                (
                    2,
                    "「完了」のTODOを戻せるのは「作業中」だけです。「作業中」にしてから、ステータスを変えてください。"
                        .into()
                ),
            ]
        );

        let p = run(status(Status::Done), &selected);
        assert_eq!(p.targets, vec![1, 4, 5]);
        assert_eq!(
            reasons(&p),
            vec![
                (2, "すでに「完了」です。".into()),
                (
                    3,
                    "「中止」から「完了」には直接変えられません。「未着手」に戻してから変えてください。"
                        .into()
                ),
            ]
        );
        assert!(p.edits.is_empty());
    }

    #[test]
    fn a_status_that_would_reverse_the_actual_period_is_left_out() {
        // Started after today (entered by hand): 完了 today would end it
        // before it starts.
        let later = Todo {
            actual_start: Some(d("2026-11-21")),
            ..t(1, Status::Doing)
        };
        let p = run(status(Status::Done), &[later, t(2, Status::Doing)]);
        assert_eq!(p.targets, vec![2]);
        assert_eq!(
            reasons(&p),
            vec![(
                1,
                "実績開始日(11月21日)が実績終了日(11月19日)より後になってしまいます。実績期間を直してから、もう一度ステータスを変えてください。"
                    .into()
            )]
        );
    }

    #[test]
    fn closing_a_parent_alone_closes_its_open_children() {
        let parent = t(1, Status::Doing);
        let children = [
            child(2, 1, Status::Todo),
            child(3, 1, Status::Done),
            child(4, 1, Status::Hold),
        ];
        let p = run_with(
            status(Status::Done),
            std::slice::from_ref(&parent),
            &children,
            &RepeatsByTodo::new(),
            &Dependencies::default(),
        );
        assert_eq!(p.targets, vec![1]);
        assert_eq!(p.closing_children, vec![2, 4]);
        assert!(p.skipped.is_empty());
    }

    #[test]
    fn a_child_selected_with_its_parent_is_closed_with_it() {
        let children = [child(2, 1, Status::Todo), child(3, 1, Status::Doing)];
        // The child comes first in the selection: still closed with its parent.
        let selected = [children[1].clone(), t(1, Status::Doing), t(9, Status::Todo)];
        let p = run_with(
            BulkChange::Status {
                status: Status::Cancelled,
                reason: Some("不要".into()),
            },
            &selected,
            &children,
            &RepeatsByTodo::new(),
            &Dependencies::default(),
        );
        assert_eq!(p.targets, vec![1, 9]);
        assert_eq!(p.closing_children, vec![2, 3]);
        assert!(p.skipped.is_empty());

        // A closed child selected with its parent is not closed with it: it
        // is judged by itself.
        let done = child(4, 1, Status::Done);
        let p = run_with(
            status(Status::Done),
            &[t(1, Status::Doing), done.clone()],
            &[children[0].clone(), done],
            &RepeatsByTodo::new(),
            &Dependencies::default(),
        );
        assert_eq!(p.targets, vec![1]);
        assert_eq!(p.closing_children, vec![2]);
        assert_eq!(reasons(&p), vec![(4, "すでに「完了」です。".into())]);
    }

    #[test]
    fn a_parent_with_a_child_that_cannot_close_is_left_out() {
        let stuck = Todo {
            actual_start: Some(d("2026-11-25")),
            ..child(2, 1, Status::Doing)
        };
        let children = [stuck.clone(), child(3, 1, Status::Todo)];
        let p = run_with(
            status(Status::Done),
            &[t(1, Status::Doing), stuck],
            &children,
            &RepeatsByTodo::new(),
            &Dependencies::default(),
        );
        assert!(p.targets.is_empty());
        assert!(p.closing_children.is_empty());
        assert_eq!(
            reasons(&p),
            vec![
                (
                    1,
                    "子TODO「T2」を完了にできません。実績開始日(11月25日)が実績終了日(11月19日)より後になってしまいます。実績期間を直してから、もう一度ステータスを変えてください。"
                        .into()
                ),
                (
                    2,
                    "実績開始日(11月25日)が実績終了日(11月19日)より後になってしまいます。実績期間を直してから、もう一度ステータスを変えてください。"
                        .into()
                ),
            ]
        );
    }

    #[test]
    fn a_child_whose_parent_is_left_out_changes_by_itself() {
        let children = [child(2, 1, Status::Todo)];
        let p = run_with(
            status(Status::Done),
            &[t(1, Status::Done), children[0].clone()],
            &children,
            &RepeatsByTodo::new(),
            &Dependencies::default(),
        );
        assert_eq!(p.targets, vec![2]);
        assert!(p.closing_children.is_empty());
        assert_eq!(reasons(&p), vec![(1, "すでに「完了」です。".into())]);
    }

    #[test]
    fn opening_changes_parents_before_children() {
        let selected = [
            child(2, 1, Status::Done),
            t(1, Status::Done),
            child(3, 1, Status::Done),
            t(4, Status::Done),
        ];
        let p = run(status(Status::Doing), &selected);
        assert_eq!(p.targets, vec![1, 4, 2, 3]);
        // Opening closes nothing.
        assert!(p.closing_children.is_empty());
    }

    #[test]
    fn closing_repeating_todos_counts_their_next_rounds() {
        let due = |id, date: &str| Todo {
            due_date: Some(d(date)),
            ..t(id, Status::Todo)
        };
        let repeat = |id, until: Option<&str>| TodoRepeat {
            todo_id: id,
            rule: RepeatRule::Daily,
            until_date: until.map(d),
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        };
        let repeats: RepeatsByTodo = [
            (1, repeat(1, None)),
            // Ends here: the next round would fall after its last day.
            (2, repeat(2, Some("2026-11-19"))),
            (3, repeat(3, None)),
        ]
        .into_iter()
        .collect();
        let selected = [
            due(1, "2026-11-19"),
            due(2, "2026-11-19"),
            Todo {
                status: Status::Done,
                ..due(3, "2026-11-19")
            },
            due(4, "2026-11-19"),
        ];
        let p = run_with(
            status(Status::Done),
            &selected,
            &[],
            &repeats,
            &Dependencies::default(),
        );
        assert_eq!(p.targets, vec![1, 2, 4]);
        assert_eq!(p.next_rounds, 1);

        // Opening makes no round.
        let p = run_with(
            status(Status::Hold),
            &selected,
            &[],
            &repeats,
            &Dependencies::default(),
        );
        assert_eq!(p.next_rounds, 0);
    }

    #[test]
    fn going_doing_lists_the_todos_that_wait_for_unfinished_ones() {
        let prerequisite = |later, first, status| Prerequisite {
            todo_id: later,
            id: first,
            title: format!("T{first}"),
            parent_title: None,
            project_name: "P".into(),
            status,
            planned_end: None,
        };
        let deps = Dependencies::new([
            prerequisite(1, 10, Status::Doing),
            prerequisite(1, 11, Status::Done),
            prerequisite(2, 12, Status::Cancelled),
            prerequisite(3, 13, Status::Todo),
        ]);
        let selected = [
            t(1, Status::Todo),
            t(2, Status::Todo),
            t(3, Status::Doing),
            t(4, Status::Hold),
        ];
        let p = run_with(
            status(Status::Doing),
            &selected,
            &[],
            &RepeatsByTodo::new(),
            &deps,
        );
        assert_eq!(p.targets, vec![1, 2, 4]);
        // 3 is already 作業中 (left out), 2 waits only for a finished one.
        assert_eq!(p.blocked.len(), 1);
        assert_eq!(p.blocked[0].0, 1);
        assert_eq!(
            p.blocked[0].1.iter().map(|b| b.id).collect::<Vec<_>>(),
            vec![10]
        );

        // Not going 作業中: nothing to list.
        let p = run_with(
            status(Status::Hold),
            &selected,
            &[],
            &RepeatsByTodo::new(),
            &deps,
        );
        assert!(p.blocked.is_empty());
    }
}
