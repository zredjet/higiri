//! TODO間の依存関係 (T-11, v0.3.0). Pure.
//!
//! One kind only: 「Aが終わるまでBに取りかかれない」 (a row of
//! `todo_dependencies`: `todo_id` = B, `depends_on_id` = A). Nothing is
//! stopped and no date is moved: a TODO only shows the TODOs it still waits
//! for (`blocked_by`), and warns when its plan starts before one of them is
//! planned to end (`starts_before_dependency`).
//!
//! - 先のTODOが終わっていない = neither 完了 nor 中止 (a cancelled TODO counts
//!   as finished).
//! - Pairs may cross projects and may join closed TODOs.
//! - Refused: a TODO and itself, a pair twice, a circle, a parent and its own
//!   child ([`check_new`]).

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use serde::Serialize;

use super::model::{Dependency, Status, Todo};

/// A TODO that another one waits for, with what the later TODO shows about
/// it. Read with the dependency (`repo::dependencies::prerequisites`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prerequisite {
    /// The later TODO (the one that waits).
    pub todo_id: i64,
    /// The TODO to finish first.
    pub id: i64,
    pub title: String,
    /// Its parent's title, for a child (shown 「親 / 子」).
    pub parent_title: Option<String>,
    pub project_name: String,
    pub status: Status,
    pub planned_end: Option<NaiveDate>,
}

/// An unfinished TODO to finish first (`TodoItem.blocked_by`): the mark in
/// the list and the confirmation before 作業中.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BlockingTodo {
    pub id: i64,
    pub title: String,
    /// For a child: shown 「親 / 子」.
    pub parent_title: Option<String>,
    pub project_name: String,
}

/// The TODOs each TODO waits for, in the order they were added. Build it
/// from the prerequisites of every TODO whose item is shown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dependencies {
    by_todo: HashMap<i64, Vec<Prerequisite>>,
}

impl Dependencies {
    /// The same pair given twice is kept once.
    pub fn new(prerequisites: impl IntoIterator<Item = Prerequisite>) -> Dependencies {
        let mut by_todo: HashMap<i64, Vec<Prerequisite>> = HashMap::new();
        for p in prerequisites {
            let own = by_todo.entry(p.todo_id).or_default();
            if !own.iter().any(|q| q.id == p.id) {
                own.push(p);
            }
        }
        Dependencies { by_todo }
    }

    fn of(&self, todo_id: i64) -> &[Prerequisite] {
        self.by_todo.get(&todo_id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Ids of every TODO `todo_id` waits for, finished or not (the Gantt's
    /// arrows).
    pub fn depends_on(&self, todo_id: i64) -> Vec<i64> {
        self.of(todo_id).iter().map(|p| p.id).collect()
    }

    /// The unfinished TODOs `t` waits for. Empty for a closed TODO.
    pub fn blocked_by(&self, t: &Todo) -> Vec<BlockingTodo> {
        if t.status.is_closed() {
            return Vec::new();
        }
        self.of(t.id)
            .iter()
            .filter(|p| p.status.is_open())
            .map(|p| BlockingTodo {
                id: p.id,
                title: p.title.clone(),
                parent_title: p.parent_title.clone(),
                project_name: p.project_name.clone(),
            })
            .collect()
    }

    /// An open TODO planned to start before an unfinished TODO it waits for
    /// is planned to end (「先のTODOより前に始まる予定」). Both dates must be
    /// set; the same day is fine. A finished TODO no longer holds it up.
    pub fn starts_before_dependency(&self, t: &Todo) -> bool {
        let Some(start) = t.planned_start.filter(|_| t.status.is_open()) else {
            return false;
        };
        self.of(t.id)
            .iter()
            .filter(|p| p.status.is_open())
            .filter_map(|p| p.planned_end)
            .any(|end| start < end)
    }
}

/// Why `todo` cannot wait for `first`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DependencyError {
    #[error("自分自身は、先に終わらせるTODOにできません。ほかのTODOを選んでください。")]
    Itself,

    #[error("親のTODOと、その子TODOの間には依存を結べません。ほかのTODOを選んでください。")]
    ParentAndChild,

    #[error("「{0}」は、すでに先に終わらせるTODOに入っています。")]
    Twice(String),

    #[error(
        "「{0}」は、このTODOが終わるのを待っているため、先に終わらせるTODOにすると循環になります。ほかのTODOを選んでください。"
    )]
    Cycle(String),
}

/// Whether `todo` may wait for `first`, given every dependency there is now.
pub fn check_new(
    todo: &Todo,
    first: &Todo,
    existing: &[Dependency],
) -> Result<(), DependencyError> {
    if todo.id == first.id {
        return Err(DependencyError::Itself);
    }
    if todo.parent_id == Some(first.id) || first.parent_id == Some(todo.id) {
        return Err(DependencyError::ParentAndChild);
    }
    if existing
        .iter()
        .any(|d| d.todo_id == todo.id && d.depends_on_id == first.id)
    {
        return Err(DependencyError::Twice(first.title.clone()));
    }
    if creates_cycle(existing, todo.id, first.id) {
        return Err(DependencyError::Cycle(first.title.clone()));
    }
    Ok(())
}

/// True when making `todo_id` wait for `depends_on_id` closes a circle: when
/// `depends_on_id` already waits for `todo_id`, directly or through other
/// TODOs. Each dependency is followed at most once (time in proportion to
/// their number).
pub fn creates_cycle(existing: &[Dependency], todo_id: i64, depends_on_id: i64) -> bool {
    if todo_id == depends_on_id {
        return true;
    }
    let mut waits_for: HashMap<i64, Vec<i64>> = HashMap::new();
    for d in existing {
        waits_for
            .entry(d.todo_id)
            .or_default()
            .push(d.depends_on_id);
    }
    let mut seen = HashSet::from([depends_on_id]);
    let mut next = vec![depends_on_id];
    while let Some(id) = next.pop() {
        for &first in waits_for.get(&id).map(Vec::as_slice).unwrap_or(&[]) {
            if first == todo_id {
                return true;
            }
            if seen.insert(first) {
                next.push(first);
            }
        }
    }
    false
}

/// The dependencies between the TODOs `ids` (both ends among them), as
/// pairs of their positions in `ids`: what a template keeps of its children
/// and a repeated round copies. Sorted by position.
pub fn positions_among(ids: &[i64], dependencies: &[Dependency]) -> Vec<(usize, usize)> {
    let at: HashMap<i64, usize> = ids.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    let mut out: Vec<(usize, usize)> = dependencies
        .iter()
        .filter_map(|d| Some((*at.get(&d.todo_id)?, *at.get(&d.depends_on_id)?)))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, todo, ts};

    fn t(id: i64, status: Status) -> Todo {
        Todo {
            id,
            title: format!("T{id}"),
            ..todo(status)
        }
    }

    fn dep(todo_id: i64, depends_on_id: i64) -> Dependency {
        Dependency {
            todo_id,
            depends_on_id,
            created_at: ts(2026, 11, 19, 0, 0),
        }
    }

    fn pre(todo_id: i64, id: i64, status: Status, planned_end: Option<&str>) -> Prerequisite {
        Prerequisite {
            todo_id,
            id,
            title: format!("T{id}"),
            parent_title: None,
            project_name: "P".into(),
            status,
            planned_end: planned_end.map(d),
        }
    }

    #[test]
    fn itself_twice_and_parent_child_are_refused() {
        let a = t(1, Status::Todo);
        let b = t(2, Status::Todo);
        assert_eq!(check_new(&b, &a, &[]), Ok(()));
        assert_eq!(check_new(&a, &a, &[]), Err(DependencyError::Itself));
        assert_eq!(
            check_new(&b, &a, &[dep(2, 1)]),
            Err(DependencyError::Twice("T1".into()))
        );
        // The other direction of an existing pair is a circle, not a twin.
        assert_eq!(
            check_new(&a, &b, &[dep(2, 1)]),
            Err(DependencyError::Cycle("T2".into()))
        );
        // A parent and its own child, both ways.
        let child = Todo {
            parent_id: Some(1),
            ..t(3, Status::Todo)
        };
        assert_eq!(
            check_new(&child, &a, &[]),
            Err(DependencyError::ParentAndChild)
        );
        assert_eq!(
            check_new(&a, &child, &[]),
            Err(DependencyError::ParentAndChild)
        );
        // Siblings, and another parent's child, are fine.
        let sibling = Todo {
            parent_id: Some(1),
            ..t(4, Status::Todo)
        };
        assert_eq!(check_new(&child, &sibling, &[]), Ok(()));
        assert_eq!(check_new(&b, &child, &[]), Ok(()));
    }

    #[test]
    fn cross_project_and_closed_todos_can_be_joined() {
        let a = Todo {
            project_id: 2,
            ..t(1, Status::Done)
        };
        let b = t(2, Status::Cancelled);
        assert_eq!(check_new(&b, &a, &[]), Ok(()));
        assert_eq!(check_new(&a, &b, &[]), Ok(()));
    }

    #[test]
    fn circles_are_found_through_any_number_of_todos() {
        // B waits for A: A waiting for B is A→B→A.
        assert!(creates_cycle(&[dep(2, 1)], 1, 2));
        // C waits for B, B waits for A: A waiting for C is A→C→B→A.
        let chain = [dep(3, 2), dep(2, 1)];
        assert!(creates_cycle(&chain, 1, 3));
        assert!(creates_cycle(&chain, 1, 2));
        // The same direction is not a circle.
        assert!(!creates_cycle(&chain, 3, 1));
        assert!(!creates_cycle(&chain, 4, 3));
        // Itself.
        assert!(creates_cycle(&[], 5, 5));
        // A diamond (D waits for B and C, both wait for A) has no circle.
        let diamond = [dep(2, 1), dep(3, 1), dep(4, 2), dep(4, 3)];
        assert!(!creates_cycle(&diamond, 5, 4));
        assert!(creates_cycle(&diamond, 1, 4));
        // A circle elsewhere (imported data) does not loop forever.
        let elsewhere = [dep(7, 8), dep(8, 7), dep(2, 7)];
        assert!(!creates_cycle(&elsewhere, 9, 2));
    }

    #[test]
    fn circle_check_follows_a_long_chain() {
        // 20,000 TODOs in one line: each waits for the one before. Followed
        // without recursion (the time is measured in perf.rs).
        let chain: Vec<Dependency> = (2..=20_000).map(|i| dep(i, i - 1)).collect();
        assert!(creates_cycle(&chain, 1, 20_000));
        assert!(!creates_cycle(&chain, 20_001, 20_000));
    }

    #[test]
    fn blocked_by_lists_unfinished_firsts_of_open_todos_only() {
        let deps = Dependencies::new([
            pre(2, 1, Status::Doing, None),
            pre(2, 3, Status::Done, None),
            pre(2, 4, Status::Cancelled, None),
            pre(2, 5, Status::Waiting, None),
            // Given twice: kept once.
            pre(2, 1, Status::Doing, None),
        ]);
        let ids = |b: Vec<BlockingTodo>| b.into_iter().map(|b| b.id).collect::<Vec<_>>();
        // 中止 counts as finished.
        assert_eq!(ids(deps.blocked_by(&t(2, Status::Todo))), [1, 5]);
        assert_eq!(ids(deps.blocked_by(&t(2, Status::Hold))), [1, 5]);
        // Closed TODOs are not blocked.
        assert!(deps.blocked_by(&t(2, Status::Done)).is_empty());
        assert!(deps.blocked_by(&t(2, Status::Cancelled)).is_empty());
        // A TODO waiting for nothing.
        assert!(deps.blocked_by(&t(9, Status::Todo)).is_empty());
        // The Gantt draws every first TODO.
        assert_eq!(deps.depends_on(2), [1, 3, 4, 5]);
        assert!(deps.depends_on(9).is_empty());
    }

    #[test]
    fn blocked_by_carries_the_names_shown() {
        let deps = Dependencies::new([Prerequisite {
            parent_title: Some("親".into()),
            project_name: "社内ツール整備".into(),
            ..pre(2, 1, Status::Todo, None)
        }]);
        assert_eq!(
            deps.blocked_by(&t(2, Status::Todo)),
            [BlockingTodo {
                id: 1,
                title: "T1".into(),
                parent_title: Some("親".into()),
                project_name: "社内ツール整備".into(),
            }]
        );
    }

    #[test]
    fn starts_before_dependency_needs_both_dates_and_an_earlier_start() {
        let later = |start: Option<&str>, status: Status| Todo {
            planned_start: start.map(d),
            ..t(2, status)
        };
        let deps = Dependencies::new([pre(2, 1, Status::Doing, Some("2026-11-20"))]);
        assert!(deps.starts_before_dependency(&later(Some("2026-11-19"), Status::Todo)));
        // The same day is fine.
        assert!(!deps.starts_before_dependency(&later(Some("2026-11-20"), Status::Todo)));
        assert!(!deps.starts_before_dependency(&later(Some("2026-11-21"), Status::Todo)));
        // Month and year boundaries.
        let year = Dependencies::new([pre(2, 1, Status::Todo, Some("2027-01-01"))]);
        assert!(year.starts_before_dependency(&later(Some("2026-12-31"), Status::Todo)));
        assert!(!year.starts_before_dependency(&later(Some("2027-01-01"), Status::Todo)));
        // No planned start, or a closed later TODO.
        assert!(!deps.starts_before_dependency(&later(None, Status::Todo)));
        assert!(!deps.starts_before_dependency(&later(Some("2026-11-19"), Status::Done)));
        // No planned end on the first TODO.
        let no_end = Dependencies::new([pre(2, 1, Status::Todo, None)]);
        assert!(!no_end.starts_before_dependency(&later(Some("2026-11-01"), Status::Todo)));
        // A finished first TODO no longer holds it up.
        for closed in [Status::Done, Status::Cancelled] {
            let deps = Dependencies::new([pre(2, 1, closed, Some("2026-11-20"))]);
            assert!(!deps.starts_before_dependency(&later(Some("2026-11-19"), Status::Todo)));
        }
        // Any of several.
        let several = Dependencies::new([
            pre(2, 1, Status::Todo, Some("2026-11-10")),
            pre(2, 3, Status::Todo, Some("2026-11-25")),
        ]);
        assert!(several.starts_before_dependency(&later(Some("2026-11-19"), Status::Todo)));
    }

    #[test]
    fn positions_among_keeps_pairs_inside_the_set() {
        let deps = [
            dep(11, 10),
            dep(12, 11),
            dep(12, 99),
            dep(99, 10),
            dep(12, 10),
        ];
        assert_eq!(
            positions_among(&[10, 11, 12], &deps),
            [(1, 0), (2, 0), (2, 1)]
        );
        // Without 11 (a cancelled child left out).
        assert_eq!(positions_among(&[10, 12], &deps), [(1, 0)]);
        assert!(positions_among(&[], &deps).is_empty());
    }
}
