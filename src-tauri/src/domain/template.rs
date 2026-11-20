//! TODOテンプレート (T-10, v0.3.0). Pure.
//!
//! A template is a parent TODO and its children, stored as JSON in
//! `todo_templates.body`:
//!
//! ```json
//! { "title": "月次締め", "priority": "medium", "memo_md": "## 手順\n…",
//!   "children": [
//!     { "title": "売上の集計", "priority": "high", "memo_md": "", "due_offset_days": -3, "depends_on": [] },
//!     { "title": "報告書の作成", "priority": "medium", "memo_md": "", "due_offset_days": 0, "depends_on": [0] } ] }
//! ```
//!
//! - `due_offset_days`: the child's 日限 minus the parent's, in calendar days;
//!   `null` when either was empty when the template was saved.
//! - `depends_on`: positions (from 0) of the children this child waits for
//!   (T-11). `[]` when the key is missing.
//!
//! This phase has the types and their checks only; making a template from
//! TODOs and TODOs from a template come with phase 20.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::model::Priority;

/// The parent TODO of a template (`todo_templates.body`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateBody {
    pub title: String,
    pub priority: Priority,
    pub memo_md: String,
    /// In the order they were saved (the children's `id` order).
    pub children: Vec<TemplateChild>,
}

/// A child TODO of a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateChild {
    pub title: String,
    pub priority: Priority,
    pub memo_md: String,
    /// Child's 日限 − parent's 日限, in calendar days (negative = before).
    pub due_offset_days: Option<i64>,
    /// Positions of the children this one waits for.
    #[serde(default)]
    pub depends_on: Vec<usize>,
}

/// What is wrong with a template body. Positions count children from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateBodyError {
    /// The parent's title is empty (after trimming).
    EmptyTitle,
    /// That child's title is empty (after trimming).
    EmptyChildTitle(usize),
    /// `depends_on` of `child` names a position with no child.
    DependsOnMissing { child: usize, target: usize },
    /// A child waits for itself.
    DependsOnItself(usize),
    /// `depends_on` of `child` names `target` twice.
    DependsOnTwice { child: usize, target: usize },
    /// The children wait for each other in a circle.
    DependencyCycle,
}

impl TemplateBody {
    /// The body with its titles trimmed, or what is wrong with it. Titles are
    /// required as a TODO's are; `depends_on` names other children, each once,
    /// without a circle.
    pub fn checked(mut self) -> Result<TemplateBody, TemplateBodyError> {
        self.title = self.title.trim().to_owned();
        if self.title.is_empty() {
            return Err(TemplateBodyError::EmptyTitle);
        }
        let count = self.children.len();
        for (i, child) in self.children.iter_mut().enumerate() {
            child.title = child.title.trim().to_owned();
            if child.title.is_empty() {
                return Err(TemplateBodyError::EmptyChildTitle(i));
            }
            for (k, &target) in child.depends_on.iter().enumerate() {
                if target >= count {
                    return Err(TemplateBodyError::DependsOnMissing { child: i, target });
                }
                if target == i {
                    return Err(TemplateBodyError::DependsOnItself(i));
                }
                if child.depends_on[..k].contains(&target) {
                    return Err(TemplateBodyError::DependsOnTwice { child: i, target });
                }
            }
        }
        if has_cycle(&self.children) {
            return Err(TemplateBodyError::DependencyCycle);
        }
        Ok(self)
    }

    /// The stored JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Reads stored JSON. `None` when it is not a body or does not pass
    /// [`TemplateBody::checked`].
    pub fn from_json(s: &str) -> Option<TemplateBody> {
        serde_json::from_str::<TemplateBody>(s).ok()?.checked().ok()
    }
}

/// True when following `depends_on` from some child comes back to it.
/// Expects every position to be in range.
fn has_cycle(children: &[TemplateChild]) -> bool {
    // Kahn's algorithm: repeatedly take the children that wait for nothing
    // left; a circle is what never gets taken.
    let mut waiting: Vec<usize> = children.iter().map(|c| c.depends_on.len()).collect();
    let mut ready: Vec<usize> = (0..children.len()).filter(|&i| waiting[i] == 0).collect();
    let mut taken = 0;
    while let Some(done) = ready.pop() {
        taken += 1;
        for (i, child) in children.iter().enumerate() {
            for _ in child.depends_on.iter().filter(|&&t| t == done) {
                waiting[i] -= 1;
                if waiting[i] == 0 {
                    ready.push(i);
                }
            }
        }
    }
    taken < children.len()
}

/// A saved template (`todo_templates`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoTemplate {
    pub id: i64,
    /// The name it is chosen by (unique).
    pub name: String,
    pub body: TemplateBody,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child(title: &str, depends_on: Vec<usize>) -> TemplateChild {
        TemplateChild {
            title: title.into(),
            priority: Priority::Medium,
            memo_md: String::new(),
            due_offset_days: None,
            depends_on,
        }
    }

    fn body(children: Vec<TemplateChild>) -> TemplateBody {
        TemplateBody {
            title: "月次締め".into(),
            priority: Priority::Medium,
            memo_md: String::new(),
            children,
        }
    }

    #[test]
    fn the_design_example_round_trips() {
        let json = r###"{"title":"月次締め","priority":"medium","memo_md":"## 手順\n- 前月分の売上を集計する","children":[{"title":"売上の集計","priority":"high","memo_md":"","due_offset_days":-3,"depends_on":[]},{"title":"報告書の作成","priority":"medium","memo_md":"","due_offset_days":0,"depends_on":[0]}]}"###;
        let b = TemplateBody::from_json(json).unwrap();
        assert_eq!(b.title, "月次締め");
        assert_eq!(b.priority, Priority::Medium);
        assert_eq!(b.memo_md, "## 手順\n- 前月分の売上を集計する");
        assert_eq!(b.children.len(), 2);
        assert_eq!(b.children[0].priority, Priority::High);
        assert_eq!(b.children[0].due_offset_days, Some(-3));
        assert_eq!(b.children[1].due_offset_days, Some(0));
        assert_eq!(b.children[1].depends_on, [0]);
        assert_eq!(b.to_json(), json);
    }

    #[test]
    fn missing_depends_on_is_empty_and_null_offset_is_none() {
        let json = r#"{"title":"T","priority":"low","memo_md":"","children":[
            {"title":"子","priority":"high","memo_md":"","due_offset_days":null}]}"#;
        let b = TemplateBody::from_json(json).unwrap();
        assert_eq!(b.priority, Priority::Low);
        assert_eq!(b.children[0].depends_on, Vec::<usize>::new());
        assert_eq!(b.children[0].due_offset_days, None);
        // Written back with the key.
        assert!(
            b.to_json()
                .contains(r#""due_offset_days":null,"depends_on":[]"#)
        );
        // No children at all is a template too.
        let alone = TemplateBody::from_json(
            r#"{"title":"T","priority":"medium","memo_md":"","children":[]}"#,
        )
        .unwrap();
        assert!(alone.children.is_empty());
    }

    #[test]
    fn json_of_another_shape_is_not_read() {
        for bad in [
            "",
            "{}",
            "[]",
            "null",
            r#"{"title":"T","priority":2,"memo_md":"","children":[]}"#,
            r#"{"title":"T","priority":"urgent","memo_md":"","children":[]}"#,
            r#"{"title":"T","priority":"medium","children":[]}"#,
            r#"{"title":"T","priority":"medium","memo_md":""}"#,
            r#"{"title":"T","priority":"medium","memo_md":"","children":[{"title":"子"}]}"#,
            r#"{"title":"T","priority":"medium","memo_md":"","children":[
                {"title":"子","priority":"high","memo_md":"","due_offset_days":1.5}]}"#,
            r#"{"title":"T","priority":"medium","memo_md":"","children":[
                {"title":"子","priority":"high","memo_md":"","due_offset_days":0,"depends_on":[-1]}]}"#,
        ] {
            assert_eq!(TemplateBody::from_json(bad), None, "{bad}");
        }
    }

    #[test]
    fn titles_are_trimmed_and_required() {
        let mut b = body(vec![child("  子 ", vec![])]);
        b.title = " 月次締め\n".into();
        let checked = b.checked().unwrap();
        assert_eq!(checked.title, "月次締め");
        assert_eq!(checked.children[0].title, "子");

        let mut b = body(vec![]);
        b.title = " \t".into();
        assert_eq!(b.checked(), Err(TemplateBodyError::EmptyTitle));
        assert_eq!(
            body(vec![child("a", vec![]), child("  ", vec![])]).checked(),
            Err(TemplateBodyError::EmptyChildTitle(1))
        );
        // A body that fails the check is not read either.
        assert_eq!(
            TemplateBody::from_json(
                r#"{"title":" ","priority":"medium","memo_md":"","children":[]}"#
            ),
            None
        );
    }

    #[test]
    fn depends_on_names_other_children_once() {
        // The last position is in range; one past it is not.
        assert!(
            body(vec![child("a", vec![]), child("b", vec![0])])
                .checked()
                .is_ok()
        );
        assert_eq!(
            body(vec![child("a", vec![2]), child("b", vec![])]).checked(),
            Err(TemplateBodyError::DependsOnMissing {
                child: 0,
                target: 2
            })
        );
        assert_eq!(
            body(vec![child("a", vec![]), child("b", vec![1])]).checked(),
            Err(TemplateBodyError::DependsOnItself(1))
        );
        assert_eq!(
            body(vec![
                child("a", vec![]),
                child("b", vec![]),
                child("c", vec![0, 1, 0])
            ])
            .checked(),
            Err(TemplateBodyError::DependsOnTwice {
                child: 2,
                target: 0
            })
        );
    }

    #[test]
    fn circles_are_refused_and_shared_dependencies_are_not() {
        // a ← b, a ← c, b ← d, c ← d: two paths, no circle.
        let diamond = body(vec![
            child("a", vec![]),
            child("b", vec![0]),
            child("c", vec![0]),
            child("d", vec![1, 2]),
        ]);
        assert!(diamond.checked().is_ok());
        // Earlier children may wait for later ones.
        assert!(
            body(vec![child("a", vec![1]), child("b", vec![])])
                .checked()
                .is_ok()
        );

        for children in [
            vec![child("a", vec![1]), child("b", vec![0])],
            vec![
                child("a", vec![2]),
                child("b", vec![0]),
                child("c", vec![1]),
            ],
            vec![
                child("x", vec![]),
                child("a", vec![0, 3]),
                child("b", vec![1]),
                child("c", vec![2]),
            ],
        ] {
            assert_eq!(
                body(children).checked(),
                Err(TemplateBodyError::DependencyCycle)
            );
        }
    }
}
