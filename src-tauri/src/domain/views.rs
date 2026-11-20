//! Shapes returned to the screens, assembled from stored records. Pure.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;

use super::model::{Project, Status, Todo};
use super::repeat::{RepeatInfo, TodoRepeat};
use super::status::check_allowed;
use super::subtodo::{ChildCounts, Family};
use super::urgency::{Assessment, Rules, assess, classify};

/// Number of TODOs per status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct StatusCounts {
    pub todo: i64,
    pub doing: i64,
    pub waiting: i64,
    pub hold: i64,
    pub done: i64,
    pub cancelled: i64,
}

impl StatusCounts {
    pub fn add(&mut self, status: Status) {
        let slot = match status {
            Status::Todo => &mut self.todo,
            Status::Doing => &mut self.doing,
            Status::Waiting => &mut self.waiting,
            Status::Hold => &mut self.hold,
            Status::Done => &mut self.done,
            Status::Cancelled => &mut self.cancelled,
        };
        *slot += 1;
    }

    pub fn total(&self) -> i64 {
        self.todo + self.doing + self.waiting + self.hold + self.done + self.cancelled
    }
}

/// A project with its counts (P-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ProjectSummary {
    pub project: Project,
    pub counts: StatusCounts,
    /// All TODOs, 中止 included (the 「すべて」 tab, delete confirmation).
    pub todo_count: i64,
    /// Denominator of the progress: all TODOs except 中止.
    pub progress_total: i64,
    /// Numerator of the progress: 完了.
    pub progress_done: i64,
    /// TODOs in any 日限注意 category.
    pub attention_count: i64,
}

/// Builds the summaries in the order of `projects`. `todos` may contain TODOs
/// of other projects; they are ignored.
pub fn project_summaries(
    projects: Vec<Project>,
    todos: &[Todo],
    today: NaiveDate,
    rules: &Rules,
) -> Vec<ProjectSummary> {
    let mut counts: HashMap<i64, (StatusCounts, i64)> = HashMap::new();
    for t in todos {
        let entry = counts.entry(t.project_id).or_default();
        entry.0.add(t.status);
        if classify(t, today, rules).is_some() {
            entry.1 += 1;
        }
    }
    projects
        .into_iter()
        .map(|project| {
            let (c, attention) = counts.get(&project.id).copied().unwrap_or_default();
            ProjectSummary {
                todo_count: c.total(),
                progress_total: c.total() - c.cancelled,
                progress_done: c.done,
                attention_count: attention,
                counts: c,
                project,
            }
        })
        .collect()
}

/// A TODO as shown in lists and the detail panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TodoItem {
    pub todo: Todo,
    pub project_name: String,
    pub assessment: Assessment,
    /// Statuses `change_status` accepts from the current one (the status
    /// select offers only these).
    pub next_statuses: Vec<Status>,
    /// The follow-up date offered when this TODO goes 待ち today: the
    /// setting's days (business days in 営業日) after today.
    pub follow_up_default: NaiveDate,
    /// The parent's title, for a child (S-1). Shown as 「親 / 子」.
    pub parent_title: Option<String>,
    /// The children counts, for a TODO with children (S-1).
    pub children: Option<ChildCounts>,
    /// An open child due after its parent (「親の日限より後」).
    pub due_after_parent: bool,
    /// Its repetition (T-12), with the next round's 日限 if it were closed
    /// today. `null` = it does not repeat.
    pub repeat: Option<RepeatInfo>,
}

/// `family` gives the parent's title and the children counts: build it from
/// the TODOs around this one (see [`Family`]). `repeat` is this TODO's
/// repetition, if it has one.
pub fn todo_item(
    todo: Todo,
    project_name: String,
    family: &Family,
    repeat: Option<&TodoRepeat>,
    today: NaiveDate,
    rules: &Rules,
) -> TodoItem {
    let assessment = assess(&todo, today, rules);
    let next_statuses = Status::ALL
        .into_iter()
        .filter(|to| check_allowed(todo.status, *to).is_ok())
        .collect();
    TodoItem {
        parent_title: family.parent_title(&todo).map(str::to_owned),
        children: family.children(&todo),
        due_after_parent: family.due_after_parent(&todo),
        repeat: repeat.map(|r| r.info(todo.due_date, today, rules)),
        todo,
        project_name,
        assessment,
        next_statuses,
        follow_up_default: rules.follow_up_default(today),
    }
}

/// A row of the TODO list, in tree order (S-1): a parent, then its children.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TodoListRow {
    pub item: TodoItem,
    /// Shown indented under its parent.
    pub child: bool,
    /// A parent that does not match the filters, shown (dimmed) only to
    /// hold its matching children. Not counted in the tabs.
    pub bridge: bool,
}

/// The TODO list with the counts for its status tabs. The counts cover the
/// project, 日限 and 優先度 filters but not the status filter, so every tab
/// shows how many rows it would list (bridge rows are not counted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TodoList {
    pub rows: Vec<TodoListRow>,
    pub counts: StatusCounts,
    /// The 「すべて」 tab.
    pub total: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repeat::{DayOfWeek, RepeatRule};
    use crate::domain::testing::{business_rules, d, rules, todo, ts};

    fn project(id: i64, name: &str) -> Project {
        Project {
            id,
            name: name.into(),
            color: "#3B6FD4".into(),
            description: String::new(),
            sort_order: id,
            archived_at: None,
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        }
    }

    fn t(project_id: i64, status: Status, due: Option<&str>) -> Todo {
        Todo {
            project_id,
            due_date: due.map(d),
            ..todo(status)
        }
    }

    #[test]
    fn summaries_count_progress_without_cancelled() {
        let today = d("2026-11-19");
        // 基幹システム更改 in the mock: 6 TODOs, 1 done, 1 cancelled, 3 urgent.
        let todos = vec![
            t(1, Status::Doing, Some("2026-11-19")),
            t(1, Status::Waiting, Some("2026-11-20")),
            t(1, Status::Todo, Some("2026-11-30")),
            Todo {
                planned_start: Some(d("2026-11-17")),
                ..t(1, Status::Todo, Some("2026-12-04"))
            },
            t(1, Status::Done, Some("2026-11-13")),
            t(1, Status::Cancelled, Some("2026-11-27")),
            t(2, Status::Todo, Some("2026-11-17")),
        ];
        let out = project_summaries(
            vec![
                project(1, "基幹システム更改"),
                project(3, "空のプロジェクト"),
            ],
            &todos,
            today,
            &rules(3),
        );
        assert_eq!(out.len(), 2);
        let s = &out[0];
        assert_eq!(s.project.name, "基幹システム更改");
        assert_eq!(s.counts.total(), 6);
        assert_eq!(s.todo_count, 6);
        assert_eq!((s.progress_done, s.progress_total), (1, 5));
        assert_eq!(s.counts.cancelled, 1);
        assert_eq!(s.attention_count, 3);

        let empty = &out[1];
        assert_eq!(empty.counts, StatusCounts::default());
        assert_eq!(
            (
                empty.progress_done,
                empty.progress_total,
                empty.attention_count
            ),
            (0, 0, 0)
        );
    }

    #[test]
    fn todo_item_carries_the_assessment() {
        let item = todo_item(
            t(1, Status::Todo, Some("2026-11-17")),
            "社内ツール整備".into(),
            &Family::default(),
            None,
            d("2026-11-19"),
            &rules(3),
        );
        assert_eq!(item.project_name, "社内ツール整備");
        assert_eq!(item.assessment.days_until_due, Some(-2));
        assert_eq!(
            (item.parent_title, item.children, item.due_after_parent),
            (None, None, false)
        );
        assert_eq!(item.repeat, None);
        assert_eq!(
            item.next_statuses,
            vec![
                Status::Doing,
                Status::Waiting,
                Status::Hold,
                Status::Done,
                Status::Cancelled
            ]
        );
        let done = todo_item(
            t(1, Status::Done, None),
            "x".into(),
            &Family::default(),
            None,
            d("2026-11-19"),
            &rules(3),
        );
        assert_eq!(done.next_statuses, vec![Status::Doing]);
        let cancelled = todo_item(
            t(1, Status::Cancelled, None),
            "x".into(),
            &Family::default(),
            None,
            d("2026-11-19"),
            &rules(3),
        );
        assert_eq!(cancelled.next_statuses, vec![Status::Todo]);
    }

    #[test]
    fn todo_item_carries_the_family() {
        let today = d("2026-11-19");
        let parent = Todo {
            id: 1,
            title: "親".into(),
            ..t(1, Status::Doing, Some("2026-11-20"))
        };
        let child = Todo {
            id: 2,
            parent_id: Some(1),
            title: "子".into(),
            ..t(1, Status::Todo, Some("2026-11-21"))
        };
        let done = Todo {
            id: 3,
            parent_id: Some(1),
            ..t(1, Status::Done, None)
        };
        let family = Family::new([&parent, &child, &done]);
        let p = todo_item(parent.clone(), "P".into(), &family, None, today, &rules(3));
        assert_eq!(p.parent_title, None);
        assert_eq!(
            p.children,
            Some(ChildCounts {
                total: 2,
                done: 1,
                open: 1
            })
        );
        assert!(!p.due_after_parent);
        let c = todo_item(child, "P".into(), &family, None, today, &rules(3));
        assert_eq!(c.parent_title.as_deref(), Some("親"));
        assert_eq!(c.children, None);
        assert!(c.due_after_parent);
    }

    #[test]
    fn todo_item_carries_the_repetition_and_its_next_round() {
        let today = d("2026-11-19");
        let repeat = TodoRepeat {
            todo_id: 1,
            rule: RepeatRule::Weekly {
                weekdays: vec![DayOfWeek::Mon],
            },
            until_date: Some(d("2026-12-31")),
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        };
        let item = |due: &str| {
            todo_item(
                t(1, Status::Todo, Some(due)),
                "x".into(),
                &Family::default(),
                Some(&repeat),
                today,
                &rules(3),
            )
            .repeat
            .unwrap()
        };
        let info = item("2026-11-23");
        assert_eq!(info.rule, repeat.rule);
        assert_eq!(info.until_date, Some(d("2026-12-31")));
        assert_eq!(info.next_due, Some(d("2026-11-30")));
        // Overdue: counted from today.
        assert_eq!(item("2026-11-02").next_due, Some(d("2026-11-23")));
        // The last round.
        assert_eq!(item("2026-12-28").next_due, None);
    }

    #[test]
    fn follow_up_default_counts_like_the_setting() {
        let today = d("2026-11-19"); // Thursday
        let item = |r: &Rules| {
            todo_item(
                t(1, Status::Doing, None),
                "x".into(),
                &Family::default(),
                None,
                today,
                r,
            )
        };
        assert_eq!(item(&rules(3)).follow_up_default, d("2026-11-22"));
        // 11/20, 11/24, 11/25 (11/21–11/23 are off).
        assert_eq!(
            item(&business_rules(3, vec![])).follow_up_default,
            d("2026-11-25")
        );
    }
}
