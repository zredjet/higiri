//! サブTODO (S-1, v0.3.0): how parents and children may be put together, and
//! what a TODO shows about its family. Pure.
//!
//! - One level only: a parent has no parent, a child has no children.
//! - A parent and its children are in the same project.
//!
//! The database does not enforce either (`todos.parent_id` only points at a
//! TODO), so the app checks them here. Data that breaks them anyway (an
//! imported file) is still shown: see [`Family`] and
//! `listing::tree_rows`.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use serde::Serialize;

use super::model::{Status, Todo};

/// Why a TODO cannot get this parent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParentError {
    #[error("自分自身を親にはできません。ほかのTODOを親に選んでください。")]
    Itself,

    #[error("別のプロジェクトのTODOは親にできません。同じプロジェクトのTODOを選んでください。")]
    OtherProject,

    #[error("子TODOは親にできません(親子は1段だけです)。親のないTODOを選んでください。")]
    ParentIsChild,

    #[error(
        "子TODOのあるTODOは、ほかのTODOの子にできません(親子は1段だけです)。先に子TODOを親から外してください。"
    )]
    HasChildren,
}

/// Whether `parent` may become the parent of `todo`. `has_children`: `todo`
/// has children now.
pub fn check_parent(todo: &Todo, has_children: bool, parent: &Todo) -> Result<(), ParentError> {
    if parent.id == todo.id {
        return Err(ParentError::Itself);
    }
    check_new_child(todo.project_id, parent)?;
    if has_children {
        return Err(ParentError::HasChildren);
    }
    Ok(())
}

/// Whether a TODO of `project_id` may be added under `parent` (create_todo
/// with a parent).
pub fn check_new_child(project_id: i64, parent: &Todo) -> Result<(), ParentError> {
    if parent.project_id != project_id {
        return Err(ParentError::OtherProject);
    }
    if parent.parent_id.is_some() {
        return Err(ParentError::ParentIsChild);
    }
    Ok(())
}

/// Children of a parent: `total` and `done` leave 中止 out (the progress
/// 「2/3 完了」), `open` counts the open ones (the confirmation when the
/// parent is closed).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct ChildCounts {
    pub total: i64,
    pub done: i64,
    pub open: i64,
}

impl ChildCounts {
    fn add(&mut self, status: Status) {
        match status {
            Status::Cancelled => {}
            Status::Done => {
                self.total += 1;
                self.done += 1;
            }
            _ => {
                self.total += 1;
                self.open += 1;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParentInfo {
    title: String,
    due_date: Option<NaiveDate>,
}

/// The parents' titles and due dates and the children counts of a set of
/// TODOs. Build it from every TODO whose family is shown: a list's TODOs, or
/// one TODO with its parent and all its children.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Family {
    parents: HashMap<i64, ParentInfo>,
    children: HashMap<i64, ChildCounts>,
}

impl Family {
    /// A TODO given more than once is counted once.
    pub fn new<'a>(todos: impl IntoIterator<Item = &'a Todo>) -> Family {
        let mut seen = HashSet::new();
        let todos: Vec<&Todo> = todos.into_iter().filter(|t| seen.insert(t.id)).collect();
        let mut children: HashMap<i64, ChildCounts> = HashMap::new();
        for t in &todos {
            if let Some(p) = t.parent_id {
                children.entry(p).or_default().add(t.status);
            }
        }
        let parents = todos
            .iter()
            .filter(|t| children.contains_key(&t.id))
            .map(|t| {
                (
                    t.id,
                    ParentInfo {
                        title: t.title.clone(),
                        due_date: t.due_date,
                    },
                )
            })
            .collect();
        Family { parents, children }
    }

    /// The parent's title, for a child whose parent was given.
    pub fn parent_title(&self, t: &Todo) -> Option<&str> {
        let p = t.parent_id?;
        self.parents.get(&p).map(|i| i.title.as_str())
    }

    /// The children counts, for a TODO with children.
    pub fn children(&self, t: &Todo) -> Option<ChildCounts> {
        self.children.get(&t.id).copied()
    }

    /// An open child due after its parent (both due dates set; the dates as
    /// entered, before any move off a holiday). Same day is fine.
    pub fn due_after_parent(&self, t: &Todo) -> bool {
        let Some(info) = t.parent_id.and_then(|p| self.parents.get(&p)) else {
            return false;
        };
        match (t.due_date, info.due_date) {
            (Some(own), Some(parent)) => t.status.is_open() && own > parent,
            _ => false,
        }
    }
}

/// 「親 / 子」 for a child, the title alone otherwise (dashboard, reminder,
/// search, reports).
pub fn display_title(title: &str, parent_title: Option<&str>) -> String {
    match parent_title {
        Some(p) => format!("{p} / {title}"),
        None => title.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, todo};

    fn t(id: i64, parent: Option<i64>, status: Status) -> Todo {
        Todo {
            id,
            parent_id: parent,
            title: format!("T{id}"),
            ..todo(status)
        }
    }

    #[test]
    fn parent_checks() {
        let a = t(1, None, Status::Todo);
        let b = t(2, None, Status::Todo);
        assert_eq!(check_parent(&a, false, &b), Ok(()));
        // Itself.
        assert_eq!(check_parent(&a, false, &a), Err(ParentError::Itself));
        // Another project.
        let other = Todo {
            project_id: 2,
            ..t(3, None, Status::Todo)
        };
        assert_eq!(
            check_parent(&a, false, &other),
            Err(ParentError::OtherProject)
        );
        // A child cannot be a parent (one level).
        let child = t(4, Some(2), Status::Todo);
        assert_eq!(
            check_parent(&a, false, &child),
            Err(ParentError::ParentIsChild)
        );
        // A TODO with children cannot become a child.
        assert_eq!(check_parent(&a, true, &b), Err(ParentError::HasChildren));
        // A child may move to another parent.
        assert_eq!(check_parent(&child, false, &a), Ok(()));
        // Closed parents and children are fine.
        assert_eq!(
            check_parent(
                &t(5, None, Status::Done),
                false,
                &t(6, None, Status::Cancelled)
            ),
            Ok(())
        );
    }

    #[test]
    fn new_child_checks() {
        let parent = t(1, None, Status::Doing);
        assert_eq!(check_new_child(1, &parent), Ok(()));
        assert_eq!(check_new_child(2, &parent), Err(ParentError::OtherProject));
        assert_eq!(
            check_new_child(1, &t(2, Some(1), Status::Todo)),
            Err(ParentError::ParentIsChild)
        );
    }

    #[test]
    fn counts_leave_cancelled_out_of_the_progress() {
        let todos = [
            t(1, None, Status::Doing),
            t(2, Some(1), Status::Done),
            t(3, Some(1), Status::Cancelled),
            t(4, Some(1), Status::Waiting),
            t(5, Some(1), Status::Todo),
            t(6, None, Status::Todo),
        ];
        let f = Family::new(&todos);
        assert_eq!(
            f.children(&todos[0]),
            Some(ChildCounts {
                total: 3,
                done: 1,
                open: 2
            })
        );
        assert_eq!(f.children(&todos[5]), None);
        assert_eq!(f.children(&todos[1]), None);
        assert_eq!(f.parent_title(&todos[1]), Some("T1"));
        assert_eq!(f.parent_title(&todos[0]), None);
    }

    #[test]
    fn only_cancelled_children_still_count_as_children() {
        let todos = [t(1, None, Status::Doing), t(2, Some(1), Status::Cancelled)];
        assert_eq!(
            Family::new(&todos).children(&todos[0]),
            Some(ChildCounts::default())
        );
    }

    #[test]
    fn a_todo_given_twice_is_counted_once() {
        let parent = t(1, None, Status::Doing);
        let child = t(2, Some(1), Status::Todo);
        let f = Family::new([&parent, &child, &child, &parent]);
        assert_eq!(f.children(&parent).map(|c| c.total), Some(1));
    }

    #[test]
    fn missing_parent_has_no_title() {
        let child = t(2, Some(9), Status::Todo);
        let f = Family::new([&child]);
        assert_eq!(f.parent_title(&child), None);
        assert!(!f.due_after_parent(&child));
    }

    #[test]
    fn due_after_parent_needs_both_dates_and_an_open_child() {
        let parent = Todo {
            due_date: Some(d("2026-11-20")),
            ..t(1, None, Status::Doing)
        };
        let child = |due: Option<&str>, status: Status| Todo {
            due_date: due.map(d),
            ..t(2, Some(1), status)
        };
        let check = |c: Todo| Family::new([&parent, &c]).due_after_parent(&c);
        assert!(check(child(Some("2026-11-21"), Status::Todo)));
        assert!(check(child(Some("2026-12-01"), Status::Waiting)));
        // Same day: no warning.
        assert!(!check(child(Some("2026-11-20"), Status::Todo)));
        assert!(!check(child(Some("2026-11-19"), Status::Todo)));
        // Closed children are not warned about.
        assert!(!check(child(Some("2026-11-21"), Status::Done)));
        assert!(!check(child(Some("2026-11-21"), Status::Cancelled)));
        // Either date missing.
        assert!(!check(child(None, Status::Todo)));
        let no_due = Todo {
            due_date: None,
            ..parent.clone()
        };
        let c = child(Some("2026-11-21"), Status::Todo);
        assert!(!Family::new([&no_due, &c]).due_after_parent(&c));
        // A parent is never due after a parent.
        let f = Family::new([&parent, &c]);
        assert!(!f.due_after_parent(&parent));
    }

    #[test]
    fn display_title_joins_parent_and_child() {
        assert_eq!(display_title("子", Some("親")), "親 / 子");
        assert_eq!(display_title("単独", None), "単独");
    }
}
