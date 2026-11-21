//! 後続TODOのずらし (G-8, v0.4.0). Pure.
//!
//! When a TODO's plan is moved so that it ends later (a Gantt drag), each
//! TODO waiting for it (後続TODO) that is planned to start before its new
//! 予定終了 moves later by just the days it lacks (「足りない分だけ」): its
//! 予定開始 becomes that day (the same day is fine, as in
//! `Dependencies::starts_before_dependency`), and its 予定終了 moves as many
//! days, so the plan keeps its length. Then the TODOs waiting for a moved
//! one are checked the same way (後続の後続).
//!
//! - Only a clash with a moved TODO is mended. A clash with a TODO that did
//!   not move stays (the warning 「先行TODOより前に始まる予定」 still shows).
//! - A TODO waiting for several moved TODOs starts on the latest new 予定終了.
//! - Closed TODOs and TODOs with no 予定開始 do not move, and nothing is
//!   followed past them (they cannot push what comes after).
//! - A TODO with a 予定開始 and no 予定終了 moves its 予定開始 only, and pushes
//!   nothing (it has no end for the next to clash with).
//! - Only the 予定期間 moves, never the 日限. Days are calendar days (as the
//!   drag itself counts them), whatever 日数の数え方 is set.
//! - Each TODO is decided once, after every TODO it waits for that may move
//!   (the dependencies' order). In a circle (the app makes none, but a hand
//!   edited JSON may) the TODOs of the circle, and those after them, are not
//!   moved: the walk always ends.
//!
//! The confirmation (`preview_dependent_shift`) and the save
//! (`shift_with_dependents`) both use [`dependent_shifts`], the save on the
//! data as it is then.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use chrono::NaiveDate;

use super::date::days_between;
use super::model::{Dependency, Todo};
use super::repeat::shift;

/// A later TODO moved along with the dragged one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shift {
    pub id: i64,
    /// The 予定期間 now.
    pub start: NaiveDate,
    pub end: Option<NaiveDate>,
    /// The 予定期間 once moved.
    pub new_start: NaiveDate,
    pub new_end: Option<NaiveDate>,
    /// Calendar days later (1 or more).
    pub days: i64,
}

/// The ids of every TODO waiting for `id`, directly or through others,
/// whatever their status or dates (the TODOs `dependent_shifts` may need).
/// `id` itself is not among them, even in a circle.
pub fn reachable(id: i64, dependencies: &[Dependency]) -> Vec<i64> {
    let after = dependents(dependencies);
    let mut seen = HashSet::from([id]);
    let mut next = vec![id];
    let mut out = Vec::new();
    while let Some(n) = next.pop() {
        for &d in after.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
            if seen.insert(d) {
                out.push(d);
                next.push(d);
            }
        }
    }
    out
}

/// The later TODOs to move when the TODO `moved` is planned to end on
/// `end` (its 予定期間 once dragged; its 予定開始 pushes nothing), in the
/// dependencies' order (a TODO before those waiting for it; among TODOs
/// ready at once, the earlier 予定開始 first, then the smaller id). Empty when
/// its 予定終了 does not move later, and when nothing clashes.
///
/// `todos` must hold `moved` and the TODOs waiting for it (see
/// [`reachable`]); others are ignored, and a TODO missing from it is not
/// moved.
pub fn dependent_shifts<'a>(
    moved: i64,
    end: Option<NaiveDate>,
    todos: impl IntoIterator<Item = &'a Todo>,
    dependencies: &[Dependency],
) -> Vec<Shift> {
    let by_id: HashMap<i64, &Todo> = todos.into_iter().map(|t| (t.id, t)).collect();
    let Some(dragged) = by_id.get(&moved) else {
        return Vec::new();
    };
    // `None` is below every date: a new end where there was none moves later.
    let Some(end) = end.filter(|&e| Some(e) > dragged.planned_end) else {
        return Vec::new();
    };

    // The TODOs that may move: those reached from `moved` through TODOs that
    // can move (open, with a 予定開始).
    let after = dependents(dependencies);
    let movable = |id: i64| {
        id != moved
            && by_id
                .get(&id)
                .is_some_and(|t| t.status.is_open() && t.planned_start.is_some())
    };
    let mut reached: HashSet<i64> = HashSet::new();
    let mut next = vec![moved];
    while let Some(n) = next.pop() {
        for &d in after.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
            if movable(d) && reached.insert(d) {
                next.push(d);
            }
        }
    }
    let later_of = |n: i64| {
        after
            .get(&n)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|d| reached.contains(d))
    };

    // How many TODOs each one still waits for before it is decided (`moved`
    // is decided already; what waits for `moved` does not count).
    let mut waiting: HashMap<i64, usize> = reached.iter().map(|&id| (id, 0)).collect();
    for &n in reached.iter().chain([&moved]) {
        for d in later_of(n) {
            *waiting.entry(d).or_default() += 1;
        }
    }
    // The latest new 予定終了 among the moved TODOs each one waits for.
    let mut pushed: HashMap<i64, NaiveDate> = HashMap::new();
    let mut ready: BinaryHeap<Reverse<(NaiveDate, i64)>> = BinaryHeap::new();
    let decide = |n: i64,
                  new_end: Option<NaiveDate>,
                  waiting: &mut HashMap<i64, usize>,
                  pushed: &mut HashMap<i64, NaiveDate>,
                  ready: &mut BinaryHeap<Reverse<(NaiveDate, i64)>>| {
        for d in later_of(n) {
            if let Some(e) = new_end {
                let p = pushed.entry(d).or_insert(e);
                *p = (*p).max(e);
            }
            let left = waiting.entry(d).or_default();
            *left = left.saturating_sub(1);
            if *left == 0
                && let Some(s) = by_id.get(&d).and_then(|t| t.planned_start)
            {
                ready.push(Reverse((s, d)));
            }
        }
    };
    decide(moved, Some(end), &mut waiting, &mut pushed, &mut ready);

    let mut out = Vec::new();
    while let Some(Reverse((start, id))) = ready.pop() {
        let todo = by_id[&id];
        let shifted = pushed
            .get(&id)
            .copied()
            .filter(|&p| start < p)
            .and_then(|p| {
                let days = days_between(start, p);
                // A 予定終了 past the dates there are: not moved.
                let new_end = match todo.planned_end {
                    Some(e) => Some(shift(Some(e), days)?),
                    None => None,
                };
                Some(Shift {
                    id,
                    start,
                    end: todo.planned_end,
                    new_start: p,
                    new_end,
                    days,
                })
            });
        // A TODO that does not move pushes nothing: it is checked against
        // the TODOs waiting for it as they were.
        decide(
            id,
            shifted.and_then(|s| s.new_end),
            &mut waiting,
            &mut pushed,
            &mut ready,
        );
        out.extend(shifted);
    }
    out
}

/// The TODOs waiting for each TODO (each pair once, none waiting for itself).
fn dependents(dependencies: &[Dependency]) -> HashMap<i64, Vec<i64>> {
    let mut out: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut seen = HashSet::new();
    for d in dependencies {
        if d.todo_id != d.depends_on_id && seen.insert((d.depends_on_id, d.todo_id)) {
            out.entry(d.depends_on_id).or_default().push(d.todo_id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Status;
    use crate::domain::testing::{d, todo, ts};

    /// An open TODO planned `start`〜`end` (`""` = no date).
    fn planned(id: i64, start: &str, end: &str) -> Todo {
        let date = |s: &str| (!s.is_empty()).then(|| d(s));
        Todo {
            id,
            title: format!("T{id}"),
            planned_start: date(start),
            planned_end: date(end),
            ..todo(Status::Todo)
        }
    }

    /// `later` waits for `first`.
    fn dep(later: i64, first: i64) -> Dependency {
        Dependency {
            todo_id: later,
            depends_on_id: first,
            created_at: ts(2026, 11, 1, 0, 0),
        }
    }

    /// (id, new 予定期間, days) of each move, `""` for no 予定終了.
    /// TODO 1 dragged to `_start`〜`end` (the start pushes nothing).
    fn moves(
        todos: &[Todo],
        deps: &[Dependency],
        _start: &str,
        end: &str,
    ) -> Vec<(i64, String, String, i64)> {
        dependent_shifts(1, Some(d(end)), todos, deps)
            .into_iter()
            .map(|s| {
                (
                    s.id,
                    s.new_start.to_string(),
                    s.new_end.map(|e| e.to_string()).unwrap_or_default(),
                    s.days,
                )
            })
            .collect()
    }

    fn mv(id: i64, start: &str, end: &str, days: i64) -> (i64, String, String, i64) {
        (id, start.into(), end.into(), days)
    }

    #[test]
    fn nothing_moves_when_nothing_clashes() {
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-25", "2026-11-27"),
        ];
        let deps = [dep(2, 1)];
        assert_eq!(moves(&todos, &deps, "2026-11-18", "2026-11-22"), vec![]);
        // No TODO waits for it.
        assert_eq!(moves(&todos, &[], "2026-11-26", "2026-11-30"), vec![]);
    }

    #[test]
    fn a_later_todo_moves_by_the_days_it_lacks() {
        // The design's example: A 〜11/20 → 〜11/25, B 11/22〜11/24 → 3 days.
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-22", "2026-11-24"),
        ];
        let deps = [dep(2, 1)];
        let got = dependent_shifts(1, Some(d("2026-11-25")), &todos, &deps);
        assert_eq!(
            got,
            vec![Shift {
                id: 2,
                start: d("2026-11-22"),
                end: Some(d("2026-11-24")),
                new_start: d("2026-11-25"),
                new_end: Some(d("2026-11-27")),
                days: 3,
            }]
        );
    }

    #[test]
    fn the_same_day_is_enough() {
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-22", "2026-11-24"),
        ];
        let deps = [dep(2, 1)];
        // Ends the day B starts: no clash.
        assert_eq!(moves(&todos, &deps, "2026-11-18", "2026-11-22"), vec![]);
        // One day more: B starts on A's new end.
        assert_eq!(
            moves(&todos, &deps, "2026-11-19", "2026-11-23"),
            vec![mv(2, "2026-11-23", "2026-11-25", 1)]
        );
    }

    #[test]
    fn only_an_end_moved_later_pushes() {
        // B already starts before A ends (the warning). Moving A earlier, or
        // only its start, mends nothing.
        let todos = [
            planned(1, "2026-11-16", "2026-11-25"),
            planned(2, "2026-11-22", "2026-11-24"),
        ];
        let deps = [dep(2, 1)];
        assert_eq!(moves(&todos, &deps, "2026-11-14", "2026-11-23"), vec![]);
        assert_eq!(moves(&todos, &deps, "2026-11-10", "2026-11-25"), vec![]);
        // Later by one day: B moves to the new end (the old clash too).
        assert_eq!(
            moves(&todos, &deps, "2026-11-17", "2026-11-26"),
            vec![mv(2, "2026-11-26", "2026-11-28", 4)]
        );
        // No end: nothing pushes.
        assert_eq!(dependent_shifts(1, None, &todos, &deps), vec![]);
    }

    #[test]
    fn a_todo_that_had_no_end_moves_later() {
        let todos = [
            planned(1, "2026-11-16", ""),
            planned(2, "2026-11-22", "2026-11-24"),
        ];
        let deps = [dep(2, 1)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-16", "2026-11-23"),
            vec![mv(2, "2026-11-23", "2026-11-25", 1)]
        );
    }

    #[test]
    fn later_todos_of_later_todos_move_only_when_pushed_into() {
        // The design's example: C (11/28〜) starts after B's new end.
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-22", "2026-11-24"),
            planned(3, "2026-11-28", "2026-11-30"),
        ];
        let deps = [dep(2, 1), dep(3, 2)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-21", "2026-11-25"),
            vec![mv(2, "2026-11-25", "2026-11-27", 3)]
        );
        // Further: C lacks 2 days once B ends 11/30.
        assert_eq!(
            moves(&todos, &deps, "2026-11-24", "2026-11-28"),
            vec![
                mv(2, "2026-11-28", "2026-11-30", 6),
                mv(3, "2026-11-30", "2026-12-02", 2),
            ]
        );
    }

    #[test]
    fn a_todo_waiting_for_two_moved_todos_starts_after_the_later() {
        // 1 → 2 and 1 → 3 → 4, 2 → 4: 4 waits for the moved 2 and 3.
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-20", "2026-11-22"),
            planned(3, "2026-11-20", "2026-11-25"),
            planned(4, "2026-11-25", "2026-11-26"),
        ];
        let deps = [dep(2, 1), dep(3, 1), dep(4, 2), dep(4, 3)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-18", "2026-11-22"),
            vec![
                mv(2, "2026-11-22", "2026-11-24", 2),
                mv(3, "2026-11-22", "2026-11-27", 2),
                mv(4, "2026-11-27", "2026-11-28", 2),
            ]
        );
        // The dragged TODO and another moved one both waited for.
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-20", "2026-11-22"),
            planned(3, "2026-11-23", "2026-11-24"),
        ];
        let deps = [dep(2, 1), dep(3, 1), dep(3, 2)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-18", "2026-11-24"),
            vec![
                mv(2, "2026-11-24", "2026-11-26", 4),
                mv(3, "2026-11-26", "2026-11-27", 3),
            ]
        );
    }

    #[test]
    fn an_unmoved_todo_waited_for_is_not_mended() {
        // 3 waits for 1 and for 2 (not moved, already clashing: 2 ends 11/30).
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-16", "2026-11-30"),
            planned(3, "2026-11-22", "2026-11-24"),
        ];
        let deps = [dep(3, 1), dep(3, 2)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-18", "2026-11-23"),
            vec![mv(3, "2026-11-23", "2026-11-25", 1)]
        );
        // A later TODO with room does not move; what waits for it, pushed by
        // nothing that moved, stays even though it clashes now.
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-25", "2026-11-30"),
            planned(3, "2026-11-26", "2026-11-28"),
        ];
        let deps = [dep(2, 1), dep(3, 2)];
        assert_eq!(moves(&todos, &deps, "2026-11-18", "2026-11-22"), vec![]);
    }

    #[test]
    fn closed_and_unplanned_todos_stop_the_walk() {
        let closed = |id, status| Todo {
            status,
            ..planned(id, "2026-11-21", "2026-11-22")
        };
        for stop in [
            closed(2, Status::Done),
            closed(2, Status::Cancelled),
            planned(2, "", "2026-11-22"),
            planned(2, "", ""),
        ] {
            let todos = [
                planned(1, "2026-11-16", "2026-11-20"),
                stop,
                planned(3, "2026-11-22", "2026-11-23"),
            ];
            let deps = [dep(2, 1), dep(3, 2)];
            assert_eq!(moves(&todos, &deps, "2026-11-20", "2026-11-26"), vec![]);
        }
        // Every open status moves.
        for status in [Status::Todo, Status::Doing, Status::Waiting, Status::Hold] {
            let todos = [
                planned(1, "2026-11-16", "2026-11-20"),
                Todo {
                    status,
                    ..planned(2, "2026-11-21", "2026-11-22")
                },
            ];
            assert_eq!(
                moves(&todos, &[dep(2, 1)], "2026-11-16", "2026-11-23"),
                vec![mv(2, "2026-11-23", "2026-11-24", 2)]
            );
        }
    }

    #[test]
    fn a_todo_with_a_start_only_moves_its_start_and_pushes_nothing() {
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-21", ""),
            planned(3, "2026-11-21", "2026-11-22"),
        ];
        let deps = [dep(2, 1), dep(3, 2)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-18", "2026-11-24"),
            vec![mv(2, "2026-11-24", "", 3)]
        );
    }

    #[test]
    fn later_todos_of_other_projects_and_children_move_too() {
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            Todo {
                project_id: 2,
                ..planned(2, "2026-11-20", "2026-11-21")
            },
            Todo {
                project_id: 3,
                parent_id: Some(9),
                ..planned(3, "2026-11-21", "2026-11-22")
            },
        ];
        let deps = [dep(2, 1), dep(3, 2)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-17", "2026-11-21"),
            vec![
                mv(2, "2026-11-21", "2026-11-22", 1),
                mv(3, "2026-11-22", "2026-11-23", 1),
            ]
        );
    }

    #[test]
    fn the_order_is_the_dependencies_order() {
        // 3, 4 and 5 wait for 1; 2 waits for 3. 2 has the smaller id and the
        // earliest start, yet comes after 3. Among the TODOs ready at once,
        // the earlier 予定開始, then the smaller id.
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-17", "2026-11-17"),
            planned(3, "2026-11-18", "2026-11-19"),
            planned(4, "2026-11-19", "2026-11-19"),
            planned(5, "2026-11-18", "2026-11-18"),
        ];
        let deps = [dep(3, 1), dep(2, 3), dep(4, 1), dep(5, 1)];
        let got: Vec<i64> = dependent_shifts(1, Some(d("2026-11-21")), &todos, &deps)
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(got, vec![3, 2, 5, 4]);
    }

    #[test]
    fn a_circle_does_not_hang() {
        // 1 → 2 → 3 → 2: the circle 2, 3 is not decided, nor what follows it.
        let todos = [
            planned(1, "2026-11-16", "2026-11-20"),
            planned(2, "2026-11-20", "2026-11-21"),
            planned(3, "2026-11-21", "2026-11-22"),
            planned(4, "2026-11-22", "2026-11-23"),
        ];
        let deps = [dep(2, 1), dep(3, 2), dep(2, 3), dep(4, 3)];
        assert_eq!(moves(&todos, &deps, "2026-11-18", "2026-11-25"), vec![]);
        // A circle through the dragged TODO: it keeps its new plan.
        let deps = [dep(2, 1), dep(1, 2)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-18", "2026-11-25"),
            vec![mv(2, "2026-11-25", "2026-11-26", 5)]
        );
        // A TODO waiting for itself, and a pair given twice.
        let deps = [dep(2, 1), dep(2, 1), dep(2, 2)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-18", "2026-11-25"),
            vec![mv(2, "2026-11-25", "2026-11-26", 5)]
        );
        assert_eq!(reachable(1, &[dep(2, 1), dep(1, 2), dep(3, 2)]), vec![2, 3]);
    }

    #[test]
    fn calendar_days_across_months_and_years() {
        // Calendar days, weekends and holidays included.
        let todos = [
            planned(1, "2026-11-25", "2026-11-28"),
            planned(2, "2026-11-29", "2026-12-02"),
            planned(3, "2026-12-30", "2027-01-02"),
        ];
        let deps = [dep(2, 1), dep(3, 2)];
        assert_eq!(
            moves(&todos, &deps, "2026-11-29", "2026-12-31"),
            vec![
                mv(2, "2026-12-31", "2027-01-03", 32),
                mv(3, "2027-01-03", "2027-01-06", 4),
            ]
        );
    }

    #[test]
    fn missing_todos_are_not_moved() {
        // The dragged TODO not given, and a later TODO not given.
        let todos = [planned(2, "2026-11-20", "2026-11-21")];
        assert_eq!(
            moves(&todos, &[dep(2, 1)], "2026-11-18", "2026-11-25"),
            vec![]
        );
        let todos = [planned(1, "2026-11-16", "2026-11-20")];
        assert_eq!(
            moves(&todos, &[dep(2, 1)], "2026-11-18", "2026-11-25"),
            vec![]
        );
    }

    #[test]
    fn reachable_follows_every_dependency() {
        let deps = [dep(2, 1), dep(3, 2), dep(4, 9), dep(5, 1)];
        let mut got = reachable(1, &deps);
        got.sort_unstable();
        assert_eq!(got, vec![2, 3, 5]);
        assert_eq!(reachable(4, &deps), Vec::<i64>::new());
    }
}
