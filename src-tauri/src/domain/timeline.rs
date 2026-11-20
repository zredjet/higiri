//! Text of a change-history line in the detail panel's timeline (M-5).
//!
//! The wording follows the mock (docs/ui-mock/Todos.dc.html):
//! 「ステータスを「未着手」から「作業中」に変更。実績開始日に 11/17 を入力」.
//! The date of the line is shown separately by the screen.

use chrono::NaiveDate;

use super::date::{month_day_slash, parse_date};
use super::events::{EventKind, Period, TodoEvent};
use super::model::Status;

fn short(value: Option<&str>) -> Option<String> {
    value.and_then(parse_date).map(month_day_slash)
}

fn status_label(value: Option<&str>) -> &'static str {
    value
        .and_then(Status::parse)
        .map(Status::label)
        .unwrap_or("不明")
}

fn period_text(p: Period) -> String {
    let fmt = |d: Option<NaiveDate>| d.map(month_day_slash).unwrap_or_default();
    match (p.start, p.end) {
        (None, None) => "なし".to_owned(),
        _ => format!("{}〜{}", fmt(p.start), fmt(p.end)),
    }
}

/// 「{what}を 11/20 に設定」「{what}を 11/20 から 11/25 に変更」「{what}を空に戻す」
fn date_change(what: &str, old: Option<String>, new: Option<String>) -> String {
    match (old, new) {
        (None, Some(n)) => format!("{what}を {n} に設定"),
        (Some(o), Some(n)) => format!("{what}を {o} から {n} に変更"),
        (Some(_), None) => format!("{what}を空に戻す"),
        (None, None) => format!("{what}を変更"),
    }
}

fn period_change(what: &str, old: Option<&str>, new: Option<&str>) -> String {
    let read = |v: Option<&str>| v.and_then(Period::from_json).map(period_text);
    match (read(old), read(new)) {
        (Some(o), Some(n)) => format!("{what}を {o} から {n} に変更"),
        _ => format!("{what}を変更"),
    }
}

pub fn describe(event: &TodoEvent) -> String {
    let old = event.old_value.as_deref();
    let new = event.new_value.as_deref();
    match event.kind {
        EventKind::Created if event.note.repeated_from.is_some() => "繰り返しで作成".to_owned(),
        EventKind::Created => "TODOを作成".to_owned(),
        EventKind::DueDate => date_change("日限", short(old), short(new)),
        EventKind::FollowUp => date_change("確認日", short(old), short(new)),
        EventKind::Planned => period_change("予定期間", old, new),
        EventKind::Actual => period_change("実績期間", old, new),
        EventKind::Status => {
            let mut parts = vec![format!(
                "ステータスを「{}」から「{}」に変更",
                status_label(old),
                status_label(new)
            )];
            let note = &event.note;
            if let Some(on) = &note.waiting_on {
                let follow = note
                    .follow_up_date
                    .map(|d| format!("、確認日は {}", month_day_slash(d)))
                    .unwrap_or_default();
                parts.push(format!("相手は{on}{follow}"));
            }
            if let Some(reason) = &note.reason {
                parts.push(format!("理由は{reason}"));
            }
            if let Some(auto) = &note.auto {
                for (key, label) in [("actual_start", "実績開始日"), ("actual_end", "実績終了日")]
                {
                    match auto.get(key) {
                        Some(Some(v)) => {
                            if let Some(d) = short(Some(v)) {
                                parts.push(format!("{label}に {d} を入力"));
                            }
                        }
                        Some(None) => parts.push(format!("{label}を空に戻す")),
                        None => {}
                    }
                }
            }
            parts.join("。")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::{self, EventNote, NewEvent};
    use crate::domain::status::{StatusChange, StatusFields, transition};
    use crate::domain::testing::{d, todo, ts};

    fn stored(e: NewEvent) -> TodoEvent {
        TodoEvent {
            id: 1,
            todo_id: 1,
            event_date: e.event_date,
            kind: e.kind,
            old_value: e.old_value,
            new_value: e.new_value,
            note: e.note,
            occurred_at: e.occurred_at,
        }
    }

    fn status_event(from: StatusFields, change: StatusChange, today: &str) -> TodoEvent {
        stored(
            transition(&from, &change, d(today), ts(2026, 11, 19, 1, 0))
                .unwrap()
                .event,
        )
    }

    #[test]
    fn status_texts_match_the_mock() {
        let started = status_event(
            StatusFields::of(&todo(Status::Todo)),
            StatusChange::to(Status::Doing),
            "2026-11-17",
        );
        assert_eq!(
            describe(&started),
            "ステータスを「未着手」から「作業中」に変更。実績開始日に 11/17 を入力"
        );

        let doing = StatusFields {
            actual_start: Some(d("2026-11-17")),
            ..StatusFields::of(&todo(Status::Doing))
        };
        let waiting = status_event(
            doing,
            StatusChange::waiting("認証担当の回答", d("2026-11-20")),
            "2026-11-19",
        );
        assert_eq!(
            describe(&waiting),
            "ステータスを「作業中」から「待ち」に変更。相手は認証担当の回答、確認日は 11/20"
        );
    }

    #[test]
    fn closing_and_reopening_texts() {
        let doing = StatusFields {
            actual_start: Some(d("2026-11-17")),
            ..StatusFields::of(&todo(Status::Doing))
        };
        let done = transition(
            &doing,
            &StatusChange::to(Status::Done),
            d("2026-11-19"),
            ts(2026, 11, 19, 1, 0),
        )
        .unwrap();
        assert_eq!(
            describe(&stored(done.event.clone())),
            "ステータスを「作業中」から「完了」に変更。実績終了日に 11/19 を入力"
        );
        let reopened = status_event(done.fields, StatusChange::to(Status::Doing), "2026-11-20");
        assert_eq!(
            describe(&reopened),
            "ステータスを「完了」から「作業中」に変更。実績終了日を空に戻す"
        );

        let cancelled = status_event(
            StatusFields::of(&todo(Status::Todo)),
            StatusChange::cancel("旧帳票の廃止が決まったため"),
            "2026-11-19",
        );
        assert_eq!(
            describe(&cancelled),
            "ステータスを「未着手」から「中止」に変更。理由は旧帳票の廃止が決まったため"
        );
    }

    #[test]
    fn date_and_period_texts() {
        let now = ts(2026, 11, 19, 1, 0);
        let today = d("2026-11-19");
        let before = todo(Status::Todo);
        let after = crate::domain::model::Todo {
            due_date: Some(d("2026-11-20")),
            planned_start: Some(d("2026-11-16")),
            planned_end: Some(d("2026-11-19")),
            ..before.clone()
        };
        let texts: Vec<_> = events::edit_events(&before, &after, today, now)
            .into_iter()
            .map(|e| describe(&stored(e)))
            .collect();
        assert_eq!(
            texts,
            vec![
                "日限を 11/20 に設定",
                "予定期間を なし から 11/16〜11/19 に変更"
            ]
        );

        let later = crate::domain::model::Todo {
            due_date: None,
            planned_end: None,
            follow_up_date: Some(d("2026-11-24")),
            ..after.clone()
        };
        let texts: Vec<_> = events::edit_events(&after, &later, today, now)
            .into_iter()
            .map(|e| describe(&stored(e)))
            .collect();
        assert_eq!(
            texts,
            vec![
                "日限を空に戻す",
                "予定期間を 11/16〜11/19 から 11/16〜 に変更",
                "確認日を 11/24 に設定"
            ]
        );
    }

    #[test]
    fn created_and_unreadable_values() {
        let created = stored(events::created(
            Status::Todo,
            d("2026-11-19"),
            ts(2026, 11, 19, 0, 0),
        ));
        assert_eq!(describe(&created), "TODOを作成");
        let repeated = stored(events::repeated(
            7,
            Status::Todo,
            d("2026-11-19"),
            ts(2026, 11, 19, 0, 0),
        ));
        assert_eq!(describe(&repeated), "繰り返しで作成");

        let odd = TodoEvent {
            kind: EventKind::Planned,
            old_value: Some("broken".into()),
            new_value: None,
            note: EventNote::default(),
            ..created
        };
        assert_eq!(describe(&odd), "予定期間を変更");
    }
}
