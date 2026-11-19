//! ステータスの遷移と実績日の自動入力 (status transitions).
//!
//! - The four open statuses (未着手・作業中・待ち・保留) move freely between
//!   each other, and any of them can move to 完了 or 中止.
//! - 完了 can only go back to 作業中 (再開); 中止 can only go back to 未着手.
//!   完了 ⇄ 中止 directly is rejected.
//! - Leaving 待ち clears the three waiting fields.
//!
//! [`transition`] returns the new field values and the single `status` event to
//! record; the caller writes both in one transaction.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::date::{add_days, format_date, format_timestamp, month_day_ja};
use super::events::{EventKind, EventNote, NewEvent};
use super::model::{Status, Todo};

/// The columns of `todos` that a status change may touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusFields {
    pub status: Status,
    pub actual_start: Option<NaiveDate>,
    pub actual_end: Option<NaiveDate>,
    pub waiting_on: Option<String>,
    pub waiting_since: Option<NaiveDate>,
    pub follow_up_date: Option<NaiveDate>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_reason: Option<String>,
}

impl StatusFields {
    pub fn of(todo: &Todo) -> StatusFields {
        StatusFields {
            status: todo.status,
            actual_start: todo.actual_start,
            actual_end: todo.actual_end,
            waiting_on: todo.waiting_on.clone(),
            waiting_since: todo.waiting_since,
            follow_up_date: todo.follow_up_date,
            closed_at: todo.closed_at,
            close_reason: todo.close_reason.clone(),
        }
    }

    pub fn apply_to(&self, todo: &mut Todo) {
        todo.status = self.status;
        todo.actual_start = self.actual_start;
        todo.actual_end = self.actual_end;
        todo.waiting_on = self.waiting_on.clone();
        todo.waiting_since = self.waiting_since;
        todo.follow_up_date = self.follow_up_date;
        todo.closed_at = self.closed_at;
        todo.close_reason = self.close_reason.clone();
    }
}

/// A requested status change, with the extra input some targets need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct StatusChange {
    pub to: Status,
    /// Required when moving to 待ち.
    pub waiting_on: Option<String>,
    /// Required when moving to 待ち.
    pub follow_up_date: Option<NaiveDate>,
    /// Required when moving to 中止.
    pub reason: Option<String>,
}

impl StatusChange {
    pub fn to(status: Status) -> StatusChange {
        StatusChange {
            to: status,
            waiting_on: None,
            follow_up_date: None,
            reason: None,
        }
    }

    pub fn waiting(waiting_on: &str, follow_up_date: NaiveDate) -> StatusChange {
        StatusChange {
            waiting_on: Some(waiting_on.to_owned()),
            follow_up_date: Some(follow_up_date),
            ..StatusChange::to(Status::Waiting)
        }
    }

    pub fn cancel(reason: &str) -> StatusChange {
        StatusChange {
            reason: Some(reason.to_owned()),
            ..StatusChange::to(Status::Cancelled)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    #[error("すでに「{}」です。", .0.label())]
    SameStatus(Status),

    #[error(
        "「{}」から「{}」には直接変えられません。「{}」に戻してから変えてください。",
        .from.label(), .to.label(), reopen_target(*.from).label()
    )]
    BetweenClosed { from: Status, to: Status },

    #[error(
        "「{}」のTODOを戻せるのは「{}」だけです。「{}」にしてから、ステータスを変えてください。",
        .from.label(), reopen_target(*.from).label(), reopen_target(*.from).label()
    )]
    ReopenOnly { from: Status, to: Status },

    #[error("待っている相手を入力してください。")]
    WaitingOnRequired,

    #[error("確認日を入力してください。")]
    FollowUpDateRequired,

    #[error("中止の理由を入力してください。")]
    ReasonRequired,

    #[error(
        "実績開始日({})が実績終了日({})より後になってしまいます。実績期間を直してから、もう一度ステータスを変えてください。",
        month_day_ja(*.start), month_day_ja(*.end)
    )]
    ActualPeriodInvalid { start: NaiveDate, end: NaiveDate },
}

/// The only status a closed TODO can go back to. Open statuses map to
/// themselves (not used for them).
pub fn reopen_target(from: Status) -> Status {
    match from {
        Status::Done => Status::Doing,
        Status::Cancelled => Status::Todo,
        other => other,
    }
}

/// Whether `from -> to` is allowed by the transition table.
pub fn check_allowed(from: Status, to: Status) -> Result<(), TransitionError> {
    if from == to {
        return Err(TransitionError::SameStatus(from));
    }
    if from.is_closed() {
        if to.is_closed() {
            return Err(TransitionError::BetweenClosed { from, to });
        }
        if to != reopen_target(from) {
            return Err(TransitionError::ReopenOnly { from, to });
        }
    }
    Ok(())
}

/// Default follow-up date when moving to 待ち: today + N days (setting).
pub fn default_follow_up_date(today: NaiveDate, follow_up_days: u32) -> NaiveDate {
    add_days(today, follow_up_days)
}

/// Result of a status change: the new field values and the event to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub fields: StatusFields,
    pub event: NewEvent,
}

/// Applies the transition table.
pub fn transition(
    current: &StatusFields,
    change: &StatusChange,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Result<Transition, TransitionError> {
    let from = current.status;
    let to = change.to;
    check_allowed(from, to)?;

    let mut next = current.clone();
    next.status = to;
    let mut note = EventNote::default();

    if from == Status::Waiting {
        next.waiting_on = None;
        next.waiting_since = None;
        next.follow_up_date = None;
    }

    match to {
        Status::Todo => {
            if from == Status::Cancelled {
                next.closed_at = None;
                next.close_reason = None;
                next.actual_end = None;
            }
        }
        Status::Doing => {
            if from == Status::Done {
                next.actual_end = None;
                next.closed_at = None;
            }
            if next.actual_start.is_none() {
                next.actual_start = Some(today);
            }
        }
        Status::Waiting => {
            let waiting_on = non_blank(change.waiting_on.as_deref())
                .ok_or(TransitionError::WaitingOnRequired)?;
            let follow_up_date = change
                .follow_up_date
                .ok_or(TransitionError::FollowUpDateRequired)?;
            next.waiting_on = Some(waiting_on.clone());
            next.follow_up_date = Some(follow_up_date);
            next.waiting_since = Some(today);
            note.waiting_on = Some(waiting_on);
            note.follow_up_date = Some(follow_up_date);
        }
        Status::Hold => {}
        Status::Done => {
            if next.actual_start.is_none() {
                next.actual_start = Some(today);
            }
            next.actual_end = Some(today);
            next.closed_at = Some(now);
        }
        Status::Cancelled => {
            let reason =
                non_blank(change.reason.as_deref()).ok_or(TransitionError::ReasonRequired)?;
            if next.actual_start.is_some() {
                next.actual_end = Some(today);
            }
            next.closed_at = Some(now);
            next.close_reason = Some(reason.clone());
            note.reason = Some(reason);
        }
    }

    if let (Some(start), Some(end)) = (next.actual_start, next.actual_end)
        && start > end
    {
        return Err(TransitionError::ActualPeriodInvalid { start, end });
    }

    note.auto = Some(auto_changes(current, &next));
    let event = NewEvent {
        event_date: today,
        kind: EventKind::Status,
        old_value: Some(from.as_str().to_owned()),
        new_value: Some(to.as_str().to_owned()),
        note,
        occurred_at: now,
    };
    Ok(Transition {
        fields: next,
        event,
    })
}

fn non_blank(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Columns the transition changed by itself. Columns filled from the user's
/// input (`waiting_on`, `follow_up_date`, `close_reason`) appear only when they
/// were cleared; their entered values are recorded at the top level of the note.
fn auto_changes(before: &StatusFields, after: &StatusFields) -> BTreeMap<String, Option<String>> {
    let mut auto = BTreeMap::new();
    let mut put = |key: &str, value: Option<String>| {
        auto.insert(key.to_owned(), value);
    };
    let date = |v: Option<NaiveDate>| v.map(format_date);

    if before.actual_start != after.actual_start {
        put("actual_start", date(after.actual_start));
    }
    if before.actual_end != after.actual_end {
        put("actual_end", date(after.actual_end));
    }
    if before.waiting_since != after.waiting_since {
        put("waiting_since", date(after.waiting_since));
    }
    if before.closed_at != after.closed_at {
        put("closed_at", after.closed_at.map(format_timestamp));
    }
    if before.waiting_on.is_some() && after.waiting_on.is_none() {
        put("waiting_on", None);
    }
    if before.follow_up_date.is_some() && after.follow_up_date.is_none() {
        put("follow_up_date", None);
    }
    if before.close_reason.is_some() && after.close_reason.is_none() {
        put("close_reason", None);
    }
    auto
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, todo, ts};

    fn today() -> NaiveDate {
        d("2026-11-19")
    }

    fn now() -> DateTime<Utc> {
        ts(2026, 11, 19, 1, 30)
    }

    fn fields(status: Status) -> StatusFields {
        StatusFields::of(&todo(status))
    }

    fn waiting_fields() -> StatusFields {
        StatusFields {
            actual_start: Some(d("2026-11-17")),
            waiting_on: Some("認証担当の回答".into()),
            waiting_since: Some(d("2026-11-18")),
            follow_up_date: Some(d("2026-11-20")),
            ..fields(Status::Waiting)
        }
    }

    fn done_fields() -> StatusFields {
        StatusFields {
            actual_start: Some(d("2026-11-10")),
            actual_end: Some(d("2026-11-13")),
            closed_at: Some(ts(2026, 11, 13, 8, 0)),
            ..fields(Status::Done)
        }
    }

    fn cancelled_fields(with_start: bool) -> StatusFields {
        StatusFields {
            actual_start: with_start.then(|| d("2026-11-10")),
            actual_end: with_start.then(|| d("2026-11-15")),
            closed_at: Some(ts(2026, 11, 15, 8, 0)),
            close_reason: Some("不要になったため".into()),
            ..fields(Status::Cancelled)
        }
    }

    fn run(current: &StatusFields, change: StatusChange) -> Transition {
        transition(current, &change, today(), now()).expect("transition should succeed")
    }

    fn auto_json(t: &Transition) -> String {
        serde_json::to_string(t.event.note.auto.as_ref().unwrap()).unwrap()
    }

    // ---- allowed / rejected pairs ----------------------------------------

    #[test]
    fn allowed_pairs_match_the_table() {
        use Status::*;
        let allowed = |from: Status, to: Status| -> bool {
            if from == to {
                return false;
            }
            match from {
                Done => to == Doing,
                Cancelled => to == Todo,
                _ => true,
            }
        };
        let mut count = 0;
        for from in Status::ALL {
            for to in Status::ALL {
                assert_eq!(
                    check_allowed(from, to).is_ok(),
                    allowed(from, to),
                    "{from:?} -> {to:?}"
                );
                if allowed(from, to) {
                    count += 1;
                }
            }
        }
        // 4 open statuses x 5 targets + 完了→作業中 + 中止→未着手
        assert_eq!(count, 22);
    }

    #[test]
    fn same_status_is_rejected() {
        for st in Status::ALL {
            assert_eq!(check_allowed(st, st), Err(TransitionError::SameStatus(st)));
        }
    }

    #[test]
    fn done_and_cancelled_cannot_switch_directly() {
        assert_eq!(
            check_allowed(Status::Done, Status::Cancelled),
            Err(TransitionError::BetweenClosed {
                from: Status::Done,
                to: Status::Cancelled
            })
        );
        assert_eq!(
            check_allowed(Status::Cancelled, Status::Done),
            Err(TransitionError::BetweenClosed {
                from: Status::Cancelled,
                to: Status::Done
            })
        );
    }

    #[test]
    fn closed_todos_reopen_to_one_status_only() {
        for to in [Status::Todo, Status::Waiting, Status::Hold] {
            assert_eq!(
                check_allowed(Status::Done, to),
                Err(TransitionError::ReopenOnly {
                    from: Status::Done,
                    to
                })
            );
        }
        for to in [Status::Doing, Status::Waiting, Status::Hold] {
            assert_eq!(
                check_allowed(Status::Cancelled, to),
                Err(TransitionError::ReopenOnly {
                    from: Status::Cancelled,
                    to
                })
            );
        }
    }

    #[test]
    fn error_messages_say_what_to_do() {
        assert_eq!(
            TransitionError::BetweenClosed {
                from: Status::Done,
                to: Status::Cancelled
            }
            .to_string(),
            "「完了」から「中止」には直接変えられません。「作業中」に戻してから変えてください。"
        );
        assert_eq!(
            TransitionError::ReopenOnly {
                from: Status::Cancelled,
                to: Status::Doing
            }
            .to_string(),
            "「中止」のTODOを戻せるのは「未着手」だけです。「未着手」にしてから、ステータスを変えてください。"
        );
        assert_eq!(
            TransitionError::ActualPeriodInvalid {
                start: d("2026-11-20"),
                end: d("2026-11-19")
            }
            .to_string(),
            "実績開始日(11月20日)が実績終了日(11月19日)より後になってしまいます。実績期間を直してから、もう一度ステータスを変えてください。"
        );
    }

    // ---- 作業中にする ------------------------------------------------------

    #[test]
    fn to_doing_fills_actual_start_when_empty() {
        let t = run(&fields(Status::Todo), StatusChange::to(Status::Doing));
        assert_eq!(t.fields.status, Status::Doing);
        assert_eq!(t.fields.actual_start, Some(today()));
        assert_eq!(auto_json(&t), r#"{"actual_start":"2026-11-19"}"#);
    }

    #[test]
    fn to_doing_keeps_existing_actual_start() {
        let current = StatusFields {
            actual_start: Some(d("2026-11-12")),
            ..fields(Status::Hold)
        };
        let t = run(&current, StatusChange::to(Status::Doing));
        assert_eq!(t.fields.actual_start, Some(d("2026-11-12")));
        assert_eq!(auto_json(&t), "{}");
    }

    #[test]
    fn to_doing_from_waiting_clears_waiting_fields() {
        let t = run(&waiting_fields(), StatusChange::to(Status::Doing));
        assert_eq!(t.fields.waiting_on, None);
        assert_eq!(t.fields.waiting_since, None);
        assert_eq!(t.fields.follow_up_date, None);
        assert_eq!(t.fields.actual_start, Some(d("2026-11-17")));
        assert_eq!(
            auto_json(&t),
            r#"{"follow_up_date":null,"waiting_on":null,"waiting_since":null}"#
        );
    }

    #[test]
    fn reopen_from_done_clears_actual_end_and_closed_at() {
        let t = run(&done_fields(), StatusChange::to(Status::Doing));
        assert_eq!(t.fields.status, Status::Doing);
        assert_eq!(t.fields.actual_start, Some(d("2026-11-10")));
        assert_eq!(t.fields.actual_end, None);
        assert_eq!(t.fields.closed_at, None);
        assert_eq!(auto_json(&t), r#"{"actual_end":null,"closed_at":null}"#);
    }

    // ---- 待ちにする --------------------------------------------------------

    #[test]
    fn to_waiting_records_inputs_and_waiting_since() {
        let current = StatusFields {
            actual_start: Some(d("2026-11-17")),
            ..fields(Status::Doing)
        };
        let t = run(
            &current,
            StatusChange::waiting("  認証担当の回答 ", d("2026-11-20")),
        );
        assert_eq!(t.fields.status, Status::Waiting);
        assert_eq!(t.fields.waiting_on.as_deref(), Some("認証担当の回答"));
        assert_eq!(t.fields.follow_up_date, Some(d("2026-11-20")));
        assert_eq!(t.fields.waiting_since, Some(today()));
        assert_eq!(
            t.event.note.to_json(),
            r#"{"waiting_on":"認証担当の回答","follow_up_date":"2026-11-20","auto":{"waiting_since":"2026-11-19"}}"#
        );
    }

    #[test]
    fn to_waiting_works_from_every_open_status() {
        for from in [Status::Todo, Status::Doing, Status::Hold] {
            let t = run(
                &fields(from),
                StatusChange::waiting("人事部", d("2026-11-22")),
            );
            assert_eq!(t.fields.status, Status::Waiting, "{from:?}");
            // Moving to 待ち does not touch the actual dates.
            assert_eq!(t.fields.actual_start, None, "{from:?}");
        }
    }

    #[test]
    fn to_waiting_requires_who_and_follow_up_date() {
        let current = fields(Status::Doing);
        let mut change = StatusChange::waiting("", d("2026-11-22"));
        assert_eq!(
            transition(&current, &change, today(), now()),
            Err(TransitionError::WaitingOnRequired)
        );
        change.waiting_on = Some("   ".into());
        assert_eq!(
            transition(&current, &change, today(), now()),
            Err(TransitionError::WaitingOnRequired)
        );
        change.waiting_on = None;
        assert_eq!(
            transition(&current, &change, today(), now()),
            Err(TransitionError::WaitingOnRequired)
        );
        change.waiting_on = Some("人事部".into());
        change.follow_up_date = None;
        assert_eq!(
            transition(&current, &change, today(), now()),
            Err(TransitionError::FollowUpDateRequired)
        );
    }

    #[test]
    fn default_follow_up_date_adds_setting_days() {
        assert_eq!(default_follow_up_date(d("2026-11-19"), 3), d("2026-11-22"));
        assert_eq!(default_follow_up_date(d("2026-12-30"), 3), d("2027-01-02"));
    }

    // ---- 保留にする --------------------------------------------------------

    #[test]
    fn to_hold_changes_no_dates() {
        let current = StatusFields {
            actual_start: Some(d("2026-11-12")),
            ..fields(Status::Doing)
        };
        let t = run(&current, StatusChange::to(Status::Hold));
        assert_eq!(
            t.fields,
            StatusFields {
                status: Status::Hold,
                ..current
            }
        );
        assert_eq!(auto_json(&t), "{}");
    }

    #[test]
    fn to_hold_from_waiting_clears_waiting_fields() {
        let t = run(&waiting_fields(), StatusChange::to(Status::Hold));
        assert_eq!(t.fields.waiting_on, None);
        assert_eq!(t.fields.waiting_since, None);
        assert_eq!(t.fields.follow_up_date, None);
    }

    // ---- 完了にする --------------------------------------------------------

    #[test]
    fn to_done_sets_actual_end_and_closed_at() {
        let current = StatusFields {
            actual_start: Some(d("2026-11-18")),
            ..fields(Status::Doing)
        };
        let t = run(&current, StatusChange::to(Status::Done));
        assert_eq!(t.fields.actual_start, Some(d("2026-11-18")));
        assert_eq!(t.fields.actual_end, Some(today()));
        assert_eq!(t.fields.closed_at, Some(now()));
        assert_eq!(
            auto_json(&t),
            r#"{"actual_end":"2026-11-19","closed_at":"2026-11-19T01:30:00.000Z"}"#
        );
    }

    #[test]
    fn to_done_without_start_fills_both_actual_dates() {
        let t = run(&fields(Status::Todo), StatusChange::to(Status::Done));
        assert_eq!(t.fields.actual_start, Some(today()));
        assert_eq!(t.fields.actual_end, Some(today()));
    }

    #[test]
    fn to_done_from_waiting_clears_waiting_fields() {
        let t = run(&waiting_fields(), StatusChange::to(Status::Done));
        assert_eq!(t.fields.waiting_on, None);
        assert_eq!(t.fields.waiting_since, None);
        assert_eq!(t.fields.follow_up_date, None);
        assert_eq!(t.fields.actual_end, Some(today()));
    }

    #[test]
    fn to_done_rejects_a_future_actual_start() {
        let current = StatusFields {
            actual_start: Some(d("2026-11-20")),
            ..fields(Status::Doing)
        };
        assert_eq!(
            transition(&current, &StatusChange::to(Status::Done), today(), now()),
            Err(TransitionError::ActualPeriodInvalid {
                start: d("2026-11-20"),
                end: today()
            })
        );
    }

    // ---- 中止にする --------------------------------------------------------

    #[test]
    fn to_cancelled_with_actual_start_sets_actual_end() {
        let current = StatusFields {
            actual_start: Some(d("2026-11-17")),
            ..fields(Status::Doing)
        };
        let t = run(
            &current,
            StatusChange::cancel(" 旧帳票の廃止が決まったため "),
        );
        assert_eq!(t.fields.status, Status::Cancelled);
        assert_eq!(t.fields.actual_end, Some(today()));
        assert_eq!(t.fields.closed_at, Some(now()));
        assert_eq!(
            t.fields.close_reason.as_deref(),
            Some("旧帳票の廃止が決まったため")
        );
        assert_eq!(
            t.event.note.to_json(),
            r#"{"reason":"旧帳票の廃止が決まったため","auto":{"actual_end":"2026-11-19","closed_at":"2026-11-19T01:30:00.000Z"}}"#
        );
    }

    #[test]
    fn to_cancelled_without_actual_start_leaves_actual_end_empty() {
        let t = run(
            &fields(Status::Todo),
            StatusChange::cancel("不要になったため"),
        );
        assert_eq!(t.fields.actual_start, None);
        assert_eq!(t.fields.actual_end, None);
        assert_eq!(t.fields.closed_at, Some(now()));
    }

    #[test]
    fn to_cancelled_requires_a_reason() {
        for reason in [None, Some(""), Some("  ")] {
            let change = StatusChange {
                reason: reason.map(str::to_owned),
                ..StatusChange::to(Status::Cancelled)
            };
            assert_eq!(
                transition(&fields(Status::Todo), &change, today(), now()),
                Err(TransitionError::ReasonRequired)
            );
        }
    }

    #[test]
    fn to_cancelled_from_waiting_clears_waiting_fields() {
        let t = run(
            &waiting_fields(),
            StatusChange::cancel("相手の案件が止まったため"),
        );
        assert_eq!(t.fields.waiting_on, None);
        assert_eq!(t.fields.waiting_since, None);
        assert_eq!(t.fields.follow_up_date, None);
        assert_eq!(t.fields.actual_end, Some(today()));
    }

    // ---- 未着手に戻す ------------------------------------------------------

    #[test]
    fn back_to_todo_from_cancelled_clears_close_fields() {
        let t = run(&cancelled_fields(true), StatusChange::to(Status::Todo));
        assert_eq!(t.fields.status, Status::Todo);
        assert_eq!(t.fields.closed_at, None);
        assert_eq!(t.fields.close_reason, None);
        assert_eq!(t.fields.actual_end, None);
        // The actual start stays.
        assert_eq!(t.fields.actual_start, Some(d("2026-11-10")));
        assert_eq!(
            auto_json(&t),
            r#"{"actual_end":null,"close_reason":null,"closed_at":null}"#
        );
    }

    #[test]
    fn back_to_todo_from_cancelled_without_dates() {
        let t = run(&cancelled_fields(false), StatusChange::to(Status::Todo));
        assert_eq!(auto_json(&t), r#"{"close_reason":null,"closed_at":null}"#);
    }

    #[test]
    fn back_to_todo_from_open_status_changes_no_dates() {
        let current = StatusFields {
            actual_start: Some(d("2026-11-12")),
            ..fields(Status::Doing)
        };
        let t = run(&current, StatusChange::to(Status::Todo));
        assert_eq!(t.fields.actual_start, Some(d("2026-11-12")));
        assert_eq!(auto_json(&t), "{}");
    }

    #[test]
    fn back_to_todo_from_waiting_clears_waiting_fields() {
        let t = run(&waiting_fields(), StatusChange::to(Status::Todo));
        assert_eq!(t.fields.waiting_on, None);
        assert_eq!(t.fields.waiting_since, None);
        assert_eq!(t.fields.follow_up_date, None);
    }

    // ---- the recorded event ------------------------------------------------

    #[test]
    fn every_transition_records_one_status_event() {
        let t = run(&fields(Status::Todo), StatusChange::to(Status::Doing));
        assert_eq!(t.event.kind, EventKind::Status);
        assert_eq!(t.event.old_value.as_deref(), Some("todo"));
        assert_eq!(t.event.new_value.as_deref(), Some("doing"));
        assert_eq!(t.event.event_date, today());
        assert_eq!(t.event.occurred_at, now());
    }

    #[test]
    fn inputs_for_other_targets_are_ignored() {
        let change = StatusChange {
            waiting_on: Some("誰か".into()),
            follow_up_date: Some(d("2026-11-25")),
            reason: Some("理由".into()),
            ..StatusChange::to(Status::Hold)
        };
        let t = transition(&fields(Status::Todo), &change, today(), now()).unwrap();
        assert_eq!(t.fields.waiting_on, None);
        assert_eq!(t.fields.close_reason, None);
        assert_eq!(t.event.note.to_json(), r#"{"auto":{}}"#);
    }

    #[test]
    fn fields_round_trip_through_a_todo() {
        let mut t = todo(Status::Todo);
        let f = waiting_fields();
        f.apply_to(&mut t);
        assert_eq!(StatusFields::of(&t), f);
    }
}
