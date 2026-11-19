//! Shapes returned to the screens, assembled from stored records. Pure.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;

use super::model::{Project, Status, Todo};
use super::urgency::{Assessment, assess};

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
    soon_days: u32,
) -> Vec<ProjectSummary> {
    let mut counts: HashMap<i64, (StatusCounts, i64)> = HashMap::new();
    for t in todos {
        let entry = counts.entry(t.project_id).or_default();
        entry.0.add(t.status);
        if super::urgency::classify(t, today, soon_days).is_some() {
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
}

pub fn todo_item(todo: Todo, project_name: String, today: NaiveDate, soon_days: u32) -> TodoItem {
    let assessment = assess(&todo, today, soon_days);
    TodoItem {
        todo,
        project_name,
        assessment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, todo, ts};

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
            3,
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
            d("2026-11-19"),
            3,
        );
        assert_eq!(item.project_name, "社内ツール整備");
        assert_eq!(item.assessment.days_until_due, Some(-2));
    }
}
