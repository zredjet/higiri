//! Filtering and sorting of the TODO list (T-4). Pure.

use std::cmp::Ordering;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::date::add_days;
use super::model::Todo;
use super::urgency::{Rules, classify};

/// The 日限 filter of the list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DueFilter {
    /// すべて
    #[default]
    All,
    /// 日限注意だけ (any category of `classify`)
    Attention,
    /// 今日まで: due today or earlier (in 営業日, a due date on a holiday
    /// counts as the business day before it)
    UntilToday,
    /// 7日後まで: due within 7 calendar days from today, or earlier
    WithinWeek,
    /// 日限なし
    NoDue,
}

/// Days covered by [`DueFilter::WithinWeek`].
pub const WEEK_DAYS: u32 = 7;

/// Order of the list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TodoSort {
    /// 登録順: project order, then the order TODOs were added.
    #[default]
    Manual,
    /// 日限の近い順 (no due date last), then priority.
    DueDate,
    /// 優先度の高い順, then due date.
    Priority,
}

pub fn due_matches(todo: &Todo, filter: DueFilter, today: NaiveDate, rules: &Rules) -> bool {
    match filter {
        DueFilter::All => true,
        DueFilter::Attention => classify(todo, today, rules).is_some(),
        DueFilter::UntilToday => todo
            .due_date
            .is_some_and(|d| rules.calendar.effective_due(d) <= today),
        DueFilter::WithinWeek => todo
            .due_date
            .is_some_and(|d| d <= add_days(today, WEEK_DAYS)),
        DueFilter::NoDue => todo.due_date.is_none(),
    }
}

fn due_cmp(a: Option<NaiveDate>, b: Option<NaiveDate>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Sorts in place. `Manual` keeps the given order (the repository's).
pub fn sort_todos<T>(items: &mut [T], sort: TodoSort, todo: impl Fn(&T) -> &Todo) {
    match sort {
        TodoSort::Manual => {}
        TodoSort::DueDate => items.sort_by(|a, b| {
            let (a, b) = (todo(a), todo(b));
            due_cmp(a.due_date, b.due_date)
                .then_with(|| a.priority.cmp(&b.priority))
                .then_with(|| a.id.cmp(&b.id))
        }),
        TodoSort::Priority => items.sort_by(|a, b| {
            let (a, b) = (todo(a), todo(b));
            a.priority
                .cmp(&b.priority)
                .then_with(|| due_cmp(a.due_date, b.due_date))
                .then_with(|| a.id.cmp(&b.id))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{Priority, Status};
    use crate::domain::testing::{business_rules, d, rules, todo};

    fn t(id: i64, due: Option<&str>, priority: Priority) -> Todo {
        Todo {
            id,
            due_date: due.map(d),
            priority,
            ..todo(Status::Todo)
        }
    }

    #[test]
    fn due_filters() {
        let today = d("2026-11-19");
        let cases = [
            (Some("2026-11-18"), [true, true, true, true, false]),
            (Some("2026-11-19"), [true, true, true, true, false]),
            (Some("2026-11-22"), [true, true, false, true, false]),
            (Some("2026-11-26"), [true, false, false, true, false]),
            (Some("2026-11-27"), [true, false, false, false, false]),
            (None, [true, false, false, false, true]),
        ];
        let filters = [
            DueFilter::All,
            DueFilter::Attention,
            DueFilter::UntilToday,
            DueFilter::WithinWeek,
            DueFilter::NoDue,
        ];
        for (due, expected) in cases {
            let item = t(1, due, Priority::Medium);
            for (f, want) in filters.iter().zip(expected) {
                assert_eq!(
                    due_matches(&item, *f, today, &rules(3)),
                    want,
                    "{due:?} {f:?}"
                );
            }
        }
    }

    #[test]
    fn within_week_crosses_month_and_year() {
        assert!(due_matches(
            &t(1, Some("2027-01-05"), Priority::Medium),
            DueFilter::WithinWeek,
            d("2026-12-29"),
            &rules(3)
        ));
        assert!(!due_matches(
            &t(1, Some("2027-01-06"), Priority::Medium),
            DueFilter::WithinWeek,
            d("2026-12-29"),
            &rules(3)
        ));
    }

    #[test]
    fn until_today_uses_the_moved_due_date_in_business_days() {
        // Friday 11/20; due Sunday 11/22 and holiday Monday 11/23 count as Friday.
        let today = d("2026-11-20");
        let b = business_rules(3, vec![]);
        for (due, calendar, business) in [
            ("2026-11-20", true, true),
            ("2026-11-22", false, true),
            ("2026-11-23", false, true),
            ("2026-11-24", false, false),
        ] {
            let item = t(1, Some(due), Priority::Medium);
            assert_eq!(
                due_matches(&item, DueFilter::UntilToday, today, &rules(3)),
                calendar,
                "{due}"
            );
            assert_eq!(
                due_matches(&item, DueFilter::UntilToday, today, &b),
                business,
                "{due}"
            );
        }
        // 7日後まで stays in calendar days: 11/27 yes, 11/28 no.
        for (due, want) in [("2026-11-27", true), ("2026-11-28", false)] {
            let item = t(1, Some(due), Priority::Medium);
            assert_eq!(
                due_matches(&item, DueFilter::WithinWeek, today, &b),
                want,
                "{due}"
            );
        }
    }

    #[test]
    fn attention_filter_follows_classify() {
        let today = d("2026-11-19");
        let waiting = Todo {
            follow_up_date: Some(d("2026-11-19")),
            ..todo(Status::Waiting)
        };
        assert!(due_matches(
            &waiting,
            DueFilter::Attention,
            today,
            &rules(3)
        ));
        let done = Todo {
            due_date: Some(d("2026-11-01")),
            ..todo(Status::Done)
        };
        assert!(!due_matches(&done, DueFilter::Attention, today, &rules(3)));
        // Closed TODOs still match the date-only filters.
        assert!(due_matches(&done, DueFilter::UntilToday, today, &rules(3)));
    }

    #[test]
    fn sorts() {
        let items = vec![
            t(1, Some("2026-11-30"), Priority::Low),
            t(2, None, Priority::High),
            t(3, Some("2026-11-20"), Priority::Medium),
            t(4, Some("2026-11-20"), Priority::High),
        ];
        let ids = |v: &[Todo]| v.iter().map(|t| t.id).collect::<Vec<_>>();

        let mut manual = items.clone();
        sort_todos(&mut manual, TodoSort::Manual, |t| t);
        assert_eq!(ids(&manual), vec![1, 2, 3, 4]);

        let mut by_due = items.clone();
        sort_todos(&mut by_due, TodoSort::DueDate, |t| t);
        assert_eq!(ids(&by_due), vec![4, 3, 1, 2]);

        let mut by_priority = items;
        sort_todos(&mut by_priority, TodoSort::Priority, |t| t);
        assert_eq!(ids(&by_priority), vec![4, 2, 3, 1]);
    }
}
