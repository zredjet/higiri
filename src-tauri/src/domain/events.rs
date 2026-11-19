//! Change history (`todo_events`): event kinds, the JSON stored in `note`, and
//! the events produced when dates are edited.
//!
//! Value formats (also listed in CLAUDE.md, 「データの決めごと」):
//! - `status`: `old_value` / `new_value` are status names (`todo`, `doing`, …).
//! - `due_date`, `follow_up`: a `YYYY-MM-DD` date, or NULL when empty.
//! - `planned`, `actual`: JSON `{"start": date|null, "end": date|null}`.
//! - `created`: `old_value` NULL, `new_value` the initial status name.
//! - `note` is always a JSON object (see [`EventNote`]).

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::date::format_date;
use super::model::{Status, Todo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Created,
    Status,
    DueDate,
    Planned,
    Actual,
    FollowUp,
}

impl EventKind {
    pub const ALL: [EventKind; 6] = [
        EventKind::Created,
        EventKind::Status,
        EventKind::DueDate,
        EventKind::Planned,
        EventKind::Actual,
        EventKind::FollowUp,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Created => "created",
            EventKind::Status => "status",
            EventKind::DueDate => "due_date",
            EventKind::Planned => "planned",
            EventKind::Actual => "actual",
            EventKind::FollowUp => "follow_up",
        }
    }

    pub fn parse(s: &str) -> Option<EventKind> {
        EventKind::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// The JSON object stored in `todo_events.note`.
///
/// - `waiting_on`, `follow_up_date`: what the user entered when moving to 待ち.
/// - `reason`: what the user entered when moving to 中止.
/// - `auto`: on `status` rows only (always present there, `{}` when nothing
///   changed). Columns of `todos` the transition changed by itself; key = column
///   name, value = new value (dates `YYYY-MM-DD`, `closed_at` UTC ISO 8601), or
///   `null` when the column was cleared.
///
/// Other rows store `{}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct EventNote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_up_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<BTreeMap<String, Option<String>>>,
}

impl EventNote {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    pub fn from_json(s: &str) -> Option<EventNote> {
        serde_json::from_str(s).ok()
    }
}

/// Value of `planned` / `actual` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Period {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}

impl Period {
    pub fn to_json(self) -> String {
        serde_json::to_string(&self).unwrap_or_else(|_| "{}".to_owned())
    }

    pub fn from_json(s: &str) -> Option<Period> {
        serde_json::from_str(s).ok()
    }
}

/// An event to be written. `id` / `todo_id` are added by the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent {
    pub event_date: NaiveDate,
    pub kind: EventKind,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub note: EventNote,
    pub occurred_at: DateTime<Utc>,
}

/// A stored event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TodoEvent {
    pub id: i64,
    pub todo_id: i64,
    pub event_date: NaiveDate,
    pub kind: EventKind,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub note: EventNote,
    pub occurred_at: DateTime<Utc>,
}

/// The `created` event for a new TODO.
pub fn created(status: Status, today: NaiveDate, now: DateTime<Utc>) -> NewEvent {
    NewEvent {
        event_date: today,
        kind: EventKind::Created,
        old_value: None,
        new_value: Some(status.as_str().to_owned()),
        note: EventNote::default(),
        occurred_at: now,
    }
}

/// Events for date edits made outside a status change (M-5): 日限, 予定期間,
/// 実績期間 and 確認日. Returns nothing for fields that did not change.
pub fn edit_events(
    before: &Todo,
    after: &Todo,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Vec<NewEvent> {
    let event = |kind, old_value, new_value| NewEvent {
        event_date: today,
        kind,
        old_value,
        new_value,
        note: EventNote::default(),
        occurred_at: now,
    };
    let date = |v: Option<NaiveDate>| v.map(format_date);

    let mut out = Vec::new();
    if before.due_date != after.due_date {
        out.push(event(
            EventKind::DueDate,
            date(before.due_date),
            date(after.due_date),
        ));
    }
    let planned = |t: &Todo| Period {
        start: t.planned_start,
        end: t.planned_end,
    };
    if planned(before) != planned(after) {
        out.push(event(
            EventKind::Planned,
            Some(planned(before).to_json()),
            Some(planned(after).to_json()),
        ));
    }
    let actual = |t: &Todo| Period {
        start: t.actual_start,
        end: t.actual_end,
    };
    if actual(before) != actual(after) {
        out.push(event(
            EventKind::Actual,
            Some(actual(before).to_json()),
            Some(actual(after).to_json()),
        ));
    }
    if before.follow_up_date != after.follow_up_date {
        out.push(event(
            EventKind::FollowUp,
            date(before.follow_up_date),
            date(after.follow_up_date),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, todo, ts};

    #[test]
    fn kind_round_trips() {
        for k in EventKind::ALL {
            assert_eq!(EventKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(EventKind::parse("deleted"), None);
    }

    #[test]
    fn empty_note_is_an_empty_object() {
        assert_eq!(EventNote::default().to_json(), "{}");
        assert_eq!(EventNote::from_json("{}"), Some(EventNote::default()));
    }

    #[test]
    fn note_keeps_explicit_nulls_in_auto() {
        let mut auto = BTreeMap::new();
        auto.insert("actual_end".to_owned(), None);
        auto.insert("closed_at".to_owned(), None);
        let note = EventNote {
            auto: Some(auto),
            ..EventNote::default()
        };
        let json = note.to_json();
        assert_eq!(json, r#"{"auto":{"actual_end":null,"closed_at":null}}"#);
        assert_eq!(EventNote::from_json(&json), Some(note));
    }

    #[test]
    fn note_with_inputs_serializes_as_plain_utf8() {
        let note = EventNote {
            waiting_on: Some("認証担当の回答".into()),
            follow_up_date: Some(d("2026-11-20")),
            ..EventNote::default()
        };
        assert_eq!(
            note.to_json(),
            r#"{"waiting_on":"認証担当の回答","follow_up_date":"2026-11-20"}"#
        );
    }

    #[test]
    fn period_json_uses_null_for_missing_ends() {
        let p = Period {
            start: Some(d("2026-11-16")),
            end: None,
        };
        assert_eq!(p.to_json(), r#"{"start":"2026-11-16","end":null}"#);
        assert_eq!(Period::from_json(&p.to_json()), Some(p));
    }

    #[test]
    fn created_event_records_initial_status() {
        let e = created(Status::Todo, d("2026-11-19"), ts(2026, 11, 19, 1, 0));
        assert_eq!(e.kind, EventKind::Created);
        assert_eq!(e.old_value, None);
        assert_eq!(e.new_value.as_deref(), Some("todo"));
        assert_eq!(e.note.to_json(), "{}");
    }

    #[test]
    fn no_events_when_nothing_relevant_changed() {
        let before = todo(Status::Doing);
        let after = Todo {
            title: "別のタイトル".into(),
            memo_md: "メモ".into(),
            ..before.clone()
        };
        assert!(edit_events(&before, &after, d("2026-11-19"), ts(2026, 11, 19, 1, 0)).is_empty());
    }

    #[test]
    fn date_edits_produce_one_event_per_field_group() {
        let today = d("2026-11-19");
        let now = ts(2026, 11, 19, 1, 0);
        let before = Todo {
            due_date: Some(d("2026-11-20")),
            planned_start: Some(d("2026-11-16")),
            planned_end: Some(d("2026-11-19")),
            follow_up_date: Some(d("2026-11-20")),
            ..todo(Status::Waiting)
        };
        let after = Todo {
            due_date: Some(d("2026-11-25")),
            planned_end: Some(d("2026-11-21")),
            actual_start: Some(d("2026-11-17")),
            follow_up_date: None,
            ..before.clone()
        };
        let events = edit_events(&before, &after, today, now);
        let summary: Vec<_> = events
            .iter()
            .map(|e| (e.kind, e.old_value.as_deref(), e.new_value.as_deref()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (EventKind::DueDate, Some("2026-11-20"), Some("2026-11-25")),
                (
                    EventKind::Planned,
                    Some(r#"{"start":"2026-11-16","end":"2026-11-19"}"#),
                    Some(r#"{"start":"2026-11-16","end":"2026-11-21"}"#)
                ),
                (
                    EventKind::Actual,
                    Some(r#"{"start":null,"end":null}"#),
                    Some(r#"{"start":"2026-11-17","end":null}"#)
                ),
                (EventKind::FollowUp, Some("2026-11-20"), None),
            ]
        );
        assert!(
            events
                .iter()
                .all(|e| e.event_date == today && e.occurred_at == now)
        );
        assert!(events.iter().all(|e| e.note.to_json() == "{}"));
    }

    #[test]
    fn clearing_the_due_date_records_null() {
        let before = Todo {
            due_date: Some(d("2026-11-20")),
            ..todo(Status::Todo)
        };
        let after = Todo {
            due_date: None,
            ..before.clone()
        };
        let events = edit_events(&before, &after, d("2026-11-19"), ts(2026, 11, 19, 1, 0));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].old_value.as_deref(), Some("2026-11-20"));
        assert_eq!(events[0].new_value, None);
    }
}
