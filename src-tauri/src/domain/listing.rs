//! Filtering and sorting of the TODO list (T-4), and its tree order (S-1).
//! Pure.

use std::cmp::Ordering;
use std::collections::HashMap;

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

/// A row of the tree order: which item it shows, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeRow {
    /// Index into the items given to [`tree_rows`].
    pub index: usize,
    /// Shown under its parent.
    pub child: bool,
    /// A parent that does not match, shown only to hold its matching children.
    pub bridge: bool,
}

/// Puts sorted items in tree order (S-1): each parent, then its children,
/// both in the given order. `hit` says whether an item matches the filters
/// (each one is judged by itself); items that do not match are left out,
/// except a parent with matching children, which comes as a bridge row.
///
/// A child is listed under its parent only when the parent is among the
/// items and has no parent itself; otherwise (data the app would not make,
/// such as an imported file) it is listed as a parent.
pub fn tree_rows<T>(
    items: &[T],
    todo: impl Fn(&T) -> &Todo,
    hit: impl Fn(&T) -> bool,
) -> Vec<TreeRow> {
    let index: HashMap<i64, usize> = items
        .iter()
        .enumerate()
        .map(|(i, t)| (todo(t).id, i))
        .collect();
    let parent_of = |t: &Todo| -> Option<usize> {
        let p = *index.get(&t.parent_id?)?;
        let parent = todo(&items[p]);
        (parent.id != t.id && parent.parent_id.is_none()).then_some(p)
    };
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    let mut roots = Vec::new();
    for (i, t) in items.iter().enumerate() {
        match parent_of(todo(t)) {
            Some(p) => children[p].push(i),
            None => roots.push(i),
        }
    }
    let mut out = Vec::new();
    for r in roots {
        let matching: Vec<usize> = children[r]
            .iter()
            .copied()
            .filter(|&c| hit(&items[c]))
            .collect();
        let root_hit = hit(&items[r]);
        if !root_hit && matching.is_empty() {
            continue;
        }
        out.push(TreeRow {
            index: r,
            child: false,
            bridge: !root_hit,
        });
        out.extend(matching.into_iter().map(|c| TreeRow {
            index: c,
            child: true,
            bridge: false,
        }));
    }
    out
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

    fn node(id: i64, parent: Option<i64>, due: Option<&str>, priority: Priority) -> Todo {
        Todo {
            parent_id: parent,
            ..t(id, due, priority)
        }
    }

    /// (id, child, bridge) of the tree rows.
    fn tree(items: &[Todo], hit: impl Fn(&Todo) -> bool) -> Vec<(i64, bool, bool)> {
        tree_rows(items, |t| t, hit)
            .into_iter()
            .map(|r| (items[r.index].id, r.child, r.bridge))
            .collect()
    }

    fn family() -> Vec<Todo> {
        vec![
            node(1, None, Some("2026-11-30"), Priority::Low),
            node(2, Some(1), Some("2026-11-25"), Priority::Medium),
            node(3, None, Some("2026-11-20"), Priority::Medium),
            node(4, Some(1), Some("2026-11-21"), Priority::High),
            node(5, Some(3), None, Priority::High),
            node(6, None, None, Priority::High),
        ]
    }

    #[test]
    fn tree_puts_children_under_their_parent_in_each_sort() {
        let all = |_: &Todo| true;
        // 登録順.
        let manual = family();
        assert_eq!(
            tree(&manual, all),
            vec![
                (1, false, false),
                (2, true, false),
                (4, true, false),
                (3, false, false),
                (5, true, false),
                (6, false, false),
            ]
        );
        // 日限: parents by due date, children by due date under them.
        let mut by_due = family();
        sort_todos(&mut by_due, TodoSort::DueDate, |t| t);
        assert_eq!(
            tree(&by_due, all),
            vec![
                (3, false, false),
                (5, true, false),
                (1, false, false),
                (4, true, false),
                (2, true, false),
                (6, false, false),
            ]
        );
        // 優先度: high first, among parents and among children.
        let mut by_priority = family();
        sort_todos(&mut by_priority, TodoSort::Priority, |t| t);
        assert_eq!(
            tree(&by_priority, all),
            vec![
                (6, false, false),
                (3, false, false),
                (5, true, false),
                (1, false, false),
                (4, true, false),
                (2, true, false),
            ]
        );
    }

    #[test]
    fn a_matching_child_brings_its_parent_as_a_bridge() {
        let items = family();
        // Only 4 and 6 match: 1 comes as a bridge for 4; 2 does not match.
        assert_eq!(
            tree(&items, |t| matches!(t.id, 4 | 6)),
            vec![(1, false, true), (4, true, false), (6, false, false)]
        );
        // A matching parent shows only its matching children.
        assert_eq!(
            tree(&items, |t| matches!(t.id, 1 | 3)),
            vec![(1, false, false), (3, false, false)]
        );
        // Nothing matches.
        assert!(tree(&items, |_| false).is_empty());
    }

    #[test]
    fn tree_lists_broken_families_as_parents() {
        let items = vec![
            // Parent not among the items.
            node(1, Some(99), None, Priority::Medium),
            // Two levels (not made by the app): 3 is under 2, 4 under 3.
            node(2, None, None, Priority::Medium),
            node(3, Some(2), None, Priority::Medium),
            node(4, Some(3), None, Priority::Medium),
            // Its own parent.
            node(5, Some(5), None, Priority::Medium),
        ];
        assert_eq!(
            tree(&items, |_| true),
            vec![
                (1, false, false),
                (2, false, false),
                (3, true, false),
                (4, false, false),
                (5, false, false),
            ]
        );
    }
}
