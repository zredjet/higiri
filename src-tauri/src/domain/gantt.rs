//! Gantt rows (G-1, G-4, G-7). Pure.
//!
//! Spans are whole days, both ends included. The screen turns them into
//! coordinates (src/features/gantt/geometry.ts).
//!
//! Rows follow the list's tree order (S-1): a parent, then its children.

use std::collections::HashMap;

use chrono::{Days, NaiveDate};
use serde::Serialize;

use super::events::{EventKind, TodoEvent};
use super::listing::tree_rows;
use super::model::{Project, Status, Todo};
use super::subtodo::Family;
use super::urgency::Rules;
use super::views::{Links, TodoItem, todo_item};

/// Days `start..=end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct DaySpan {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct GanttRow {
    pub item: TodoItem,
    /// 予定: both planned dates set.
    pub planned: Option<DaySpan>,
    /// 実績: from the actual start; to the actual end, or to today while
    /// 作業中・待ち (G-4), or just the start day otherwise.
    pub actual: Option<DaySpan>,
    /// 待ち spans inside the actual bar (G-7).
    pub waiting: Vec<DaySpan>,
    /// Shown indented under its parent (S-1).
    pub child: bool,
    /// A hidden (完了・中止) parent shown only to hold its shown children:
    /// no bars and no due mark (all spans are empty).
    pub bridge: bool,
    /// The TODOs this one waits for (T-11), finished or not: an arrow from
    /// the end of each one's planned bar to the start of this one's, when
    /// both rows are shown with a planned bar.
    pub depends_on: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct GanttProject {
    pub project: Project,
    pub rows: Vec<GanttRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Gantt {
    pub today: NaiveDate,
    pub projects: Vec<GanttProject>,
}

pub fn planned_span(t: &Todo) -> Option<DaySpan> {
    match (t.planned_start, t.planned_end) {
        (Some(start), Some(end)) if start <= end => Some(DaySpan { start, end }),
        _ => None,
    }
}

pub fn actual_span(t: &Todo, today: NaiveDate) -> Option<DaySpan> {
    let start = t.actual_start?;
    let end = match t.actual_end {
        Some(end) => end,
        None if matches!(t.status, Status::Doing | Status::Waiting) => today.max(start),
        None => start,
    };
    Some(DaySpan {
        start,
        end: end.max(start),
    })
}

/// Waiting spans from the status history: from the day the TODO entered 待ち
/// to the day before it left, or to today while it still waits. A TODO that
/// entered and left on the same day has no span for that stay.
pub fn waiting_spans(t: &Todo, events: &[&TodoEvent], today: NaiveDate) -> Vec<DaySpan> {
    let mut sorted: Vec<&TodoEvent> = events
        .iter()
        .copied()
        .filter(|e| e.kind == EventKind::Status)
        .collect();
    sorted.sort_by_key(|e| (e.occurred_at, e.id));

    let mut out = Vec::new();
    let mut open: Option<NaiveDate> = None;
    for e in sorted {
        if e.new_value.as_deref() == Some(Status::Waiting.as_str()) {
            open = Some(e.event_date);
        } else if e.old_value.as_deref() == Some(Status::Waiting.as_str())
            // Leaving 待ち closes the open span (take() clears it either way).
            && let Some(start) = open.take()
            && let Some(end) = e.event_date.checked_sub_days(Days::new(1))
            && end >= start
        {
            out.push(DaySpan { start, end });
        }
    }
    if t.status == Status::Waiting {
        // Fall back to waiting_since if the history has no entry.
        if let Some(start) = open.or(t.waiting_since) {
            out.push(DaySpan {
                start,
                end: today.max(start),
            });
        }
    }
    out
}

pub fn row(item: TodoItem, events: &[&TodoEvent], today: NaiveDate) -> GanttRow {
    let planned = planned_span(&item.todo);
    let actual = actual_span(&item.todo, today);
    let waiting = waiting_spans(&item.todo, events, today);
    GanttRow {
        item,
        planned,
        actual,
        waiting,
        child: false,
        bridge: false,
        depends_on: Vec::new(),
    }
}

/// A bridge row: the TODO without its bars.
fn bridge_row(item: TodoItem) -> GanttRow {
    GanttRow {
        item,
        planned: None,
        actual: None,
        waiting: Vec::new(),
        child: false,
        bridge: true,
        depends_on: Vec::new(),
    }
}

/// Which TODOs the chart shows (G-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GanttFilter {
    pub include_done: bool,
    pub include_cancelled: bool,
}

impl GanttFilter {
    pub fn shows(&self, status: Status) -> bool {
        match status {
            Status::Done => self.include_done,
            Status::Cancelled => self.include_cancelled,
            _ => true,
        }
    }
}

/// Groups the TODOs by project, in the order given (project order, then TODO
/// order), each parent followed by its children. `todos` should hold every
/// TODO of the projects (the hidden ones too), so the children counts and
/// the bridge rows come out right.
pub fn build(
    projects: Vec<Project>,
    todos: &[Todo],
    events: &[TodoEvent],
    links: &Links,
    filter: GanttFilter,
    today: NaiveDate,
    rules: &Rules,
) -> Gantt {
    let mut by_todo: HashMap<i64, Vec<&TodoEvent>> = HashMap::new();
    for e in events {
        by_todo.entry(e.todo_id).or_default().push(e);
    }
    let family = Family::new(todos);
    let projects = projects
        .into_iter()
        .map(|project| {
            let own: Vec<&Todo> = todos
                .iter()
                .filter(|t| t.project_id == project.id)
                .collect();
            let rows = tree_rows(&own, |t| t, |t| filter.shows(t.status))
                .into_iter()
                .map(|r| {
                    let t = own[r.index];
                    let item = todo_item(
                        t.clone(),
                        project.name.clone(),
                        &family,
                        links,
                        today,
                        rules,
                    );
                    if r.bridge {
                        return bridge_row(item);
                    }
                    let evs = by_todo.get(&t.id).map(Vec::as_slice).unwrap_or(&[]);
                    GanttRow {
                        child: r.child,
                        depends_on: links.dependencies.depends_on(t.id),
                        ..row(item, evs, today)
                    }
                })
                .collect();
            GanttProject { project, rows }
        })
        .collect();
    Gantt { today, projects }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::EventNote;
    use crate::domain::testing::{d, rules, todo};

    fn span(a: &str, b: &str) -> DaySpan {
        DaySpan {
            start: d(a),
            end: d(b),
        }
    }

    fn status_event(id: i64, date: &str, from: Status, to: Status) -> TodoEvent {
        let day = d(date);
        TodoEvent {
            id,
            todo_id: 1,
            event_date: day,
            kind: EventKind::Status,
            old_value: Some(from.as_str().into()),
            new_value: Some(to.as_str().into()),
            note: EventNote::default(),
            occurred_at: day
                .and_hms_opt(9, u32::try_from(id).unwrap_or(0), 0)
                .unwrap()
                .and_utc(),
        }
    }

    #[test]
    fn planned_needs_both_ends() {
        let t = Todo {
            planned_start: Some(d("2026-11-11")),
            planned_end: Some(d("2026-11-18")),
            ..todo(Status::Todo)
        };
        assert_eq!(planned_span(&t), Some(span("2026-11-11", "2026-11-18")));
        let half = Todo {
            planned_end: None,
            ..t
        };
        assert_eq!(planned_span(&half), None);
    }

    #[test]
    fn actual_extends_to_today_while_doing_or_waiting() {
        let today = d("2026-11-19");
        let doing = Todo {
            actual_start: Some(d("2026-11-12")),
            ..todo(Status::Doing)
        };
        assert_eq!(
            actual_span(&doing, today),
            Some(span("2026-11-12", "2026-11-19"))
        );
        let waiting = Todo {
            status: Status::Waiting,
            ..doing.clone()
        };
        assert_eq!(
            actual_span(&waiting, today),
            Some(span("2026-11-12", "2026-11-19"))
        );
        let hold = Todo {
            status: Status::Hold,
            ..doing.clone()
        };
        assert_eq!(
            actual_span(&hold, today),
            Some(span("2026-11-12", "2026-11-12"))
        );
        let done = Todo {
            actual_end: Some(d("2026-11-13")),
            status: Status::Done,
            ..doing
        };
        assert_eq!(
            actual_span(&done, today),
            Some(span("2026-11-12", "2026-11-13"))
        );
        assert_eq!(actual_span(&todo(Status::Todo), today), None);
    }

    #[test]
    fn waiting_spans_match_the_mock() {
        let today = d("2026-11-19");
        // 募集要項ページの原稿確認: doing 11/16, waiting since 11/17 → box 11/17–11/19.
        let t = Todo {
            waiting_since: Some(d("2026-11-17")),
            ..todo(Status::Waiting)
        };
        let evs = [
            status_event(1, "2026-11-16", Status::Todo, Status::Doing),
            status_event(2, "2026-11-17", Status::Doing, Status::Waiting),
        ];
        let refs: Vec<&TodoEvent> = evs.iter().collect();
        assert_eq!(
            waiting_spans(&t, &refs, today),
            vec![span("2026-11-17", "2026-11-19")]
        );

        // API設計書: waiting since today → a single day.
        let evs = [status_event(
            3,
            "2026-11-19",
            Status::Doing,
            Status::Waiting,
        )];
        let refs: Vec<&TodoEvent> = evs.iter().collect();
        assert_eq!(
            waiting_spans(&t, &refs, today),
            vec![span("2026-11-19", "2026-11-19")]
        );
    }

    #[test]
    fn closed_waiting_spans_end_the_day_before_leaving() {
        let today = d("2026-11-30");
        let t = todo(Status::Doing);
        let evs = [
            status_event(1, "2026-11-10", Status::Doing, Status::Waiting),
            status_event(2, "2026-11-13", Status::Waiting, Status::Doing),
            status_event(3, "2026-11-20", Status::Doing, Status::Waiting),
            status_event(4, "2026-11-20", Status::Waiting, Status::Hold), // same day: no span
            status_event(5, "2026-11-21", Status::Hold, Status::Doing),
        ];
        let refs: Vec<&TodoEvent> = evs.iter().collect();
        assert_eq!(
            waiting_spans(&t, &refs, today),
            vec![span("2026-11-10", "2026-11-12")]
        );
    }

    #[test]
    fn waiting_without_history_falls_back_to_waiting_since() {
        let t = Todo {
            waiting_since: Some(d("2026-11-18")),
            ..todo(Status::Waiting)
        };
        assert_eq!(
            waiting_spans(&t, &[], d("2026-11-19")),
            vec![span("2026-11-18", "2026-11-19")]
        );
    }

    #[test]
    fn filter_hides_closed_todos_as_asked() {
        let f = GanttFilter {
            include_done: true,
            include_cancelled: false,
        };
        assert!(f.shows(Status::Done));
        assert!(!f.shows(Status::Cancelled));
        assert!(f.shows(Status::Hold));
    }

    fn project(id: i64) -> Project {
        Project {
            id,
            name: format!("P{id}"),
            color: "#3B6FD4".into(),
            description: String::new(),
            sort_order: id,
            archived_at: None,
            created_at: crate::domain::testing::ts(2026, 11, 1, 0, 0),
            updated_at: crate::domain::testing::ts(2026, 11, 1, 0, 0),
        }
    }

    fn node(id: i64, parent: Option<i64>, status: Status) -> Todo {
        Todo {
            id,
            parent_id: parent,
            planned_start: Some(d("2026-11-16")),
            planned_end: Some(d("2026-11-20")),
            actual_start: Some(d("2026-11-16")),
            due_date: Some(d("2026-11-25")),
            ..todo(status)
        }
    }

    /// (id, child, bridge, has bars) of the rows.
    fn shape(g: &Gantt) -> Vec<(i64, bool, bool, bool)> {
        g.projects[0]
            .rows
            .iter()
            .map(|r| {
                (
                    r.item.todo.id,
                    r.child,
                    r.bridge,
                    r.planned.is_some() || r.actual.is_some(),
                )
            })
            .collect()
    }

    #[test]
    fn children_follow_their_parent() {
        let todos = vec![
            node(1, None, Status::Doing),
            node(2, None, Status::Todo),
            node(3, Some(1), Status::Doing),
            node(4, Some(1), Status::Done),
        ];
        let all = GanttFilter {
            include_done: true,
            include_cancelled: true,
        };
        let g = build(
            vec![project(1)],
            &todos,
            &[],
            &Links::default(),
            all,
            d("2026-11-19"),
            &rules(3),
        );
        assert_eq!(
            shape(&g),
            vec![
                (1, false, false, true),
                (3, true, false, true),
                (4, true, false, true),
                (2, false, false, true),
            ]
        );
        assert_eq!(
            g.projects[0].rows[0]
                .item
                .children
                .map(|c| (c.done, c.total)),
            Some((1, 2))
        );
        assert_eq!(
            g.projects[0].rows[1].item.parent_title.as_deref(),
            Some("テスト")
        );
    }

    #[test]
    fn hidden_parent_of_shown_children_is_a_bridge_without_bars() {
        let todos = vec![
            node(1, None, Status::Done),
            node(2, Some(1), Status::Doing),
            node(3, Some(1), Status::Done),
            node(4, None, Status::Done),
        ];
        let open_only = GanttFilter {
            include_done: false,
            include_cancelled: false,
        };
        let g = build(
            vec![project(1)],
            &todos,
            &[],
            &Links::default(),
            open_only,
            d("2026-11-19"),
            &rules(3),
        );
        assert_eq!(
            shape(&g),
            vec![(1, false, true, false), (2, true, false, true)]
        );
        let bridge = &g.projects[0].rows[0];
        assert!(bridge.waiting.is_empty());
        // Its counts still cover the hidden child.
        assert_eq!(
            bridge.item.children.map(|c| (c.done, c.total)),
            Some((1, 2))
        );
    }

    #[test]
    fn rows_carry_the_todos_they_wait_for() {
        use crate::domain::dependency::{Dependencies, Prerequisite};
        let todos = vec![node(1, None, Status::Done), node(2, None, Status::Todo)];
        let first = |id: i64, status: Status| Prerequisite {
            todo_id: 2,
            id,
            title: format!("T{id}"),
            parent_title: None,
            project_name: "P".into(),
            status,
            planned_end: None,
        };
        let links = Links {
            // 9 is in another project: listed, the screen draws no arrow to it.
            dependencies: Dependencies::new([first(1, Status::Done), first(9, Status::Todo)]),
            ..Links::default()
        };
        let all = GanttFilter {
            include_done: true,
            include_cancelled: true,
        };
        let g = build(
            vec![project(1)],
            &todos,
            &[],
            &links,
            all,
            d("2026-11-19"),
            &rules(3),
        );
        let rows = &g.projects[0].rows;
        assert!(rows[0].depends_on.is_empty());
        // Finished or not.
        assert_eq!(rows[1].depends_on, [1, 9]);
        assert_eq!(
            rows[1]
                .item
                .blocked_by
                .iter()
                .map(|b| b.id)
                .collect::<Vec<_>>(),
            [9]
        );
    }
}
