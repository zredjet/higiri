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
//!   (T-11): the dependencies between the children when it was saved (those
//!   of a left-out child are dropped), joined again when it is used. `[]`
//!   when the key is missing.
//!
//! A template is made from a TODO without a parent and its children
//! ([`TemplateBody::from_todos`]); a new TODO made from it gets the
//! template's memo and children ([`TemplateBody::new_children`]). Templates
//! belong to no project and point at no TODO.

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use super::date::days_between;
use super::dependency::positions_among;
use super::model::{Dependency, Priority, Status, Todo};

/// Longest name, in characters (after trimming).
pub const NAME_MAX: usize = 40;

/// Why a template name cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("テンプレートの名前を入力してください。")]
    Empty,

    #[error("テンプレートの名前は{}字までにしてください。", NAME_MAX)]
    TooLong,
}

/// The name without surrounding spaces: 1 to [`NAME_MAX`] characters.
pub fn checked_name(name: &str) -> Result<String, NameError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.chars().count() > NAME_MAX {
        return Err(NameError::TooLong);
    }
    Ok(name.to_owned())
}

/// A child TODO to create from a template: everything but the parent and
/// the project, which come from the new parent. It starts as 未着手 with no
/// planned period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChild {
    pub title: String,
    pub priority: Priority,
    pub memo_md: String,
    pub due_date: Option<NaiveDate>,
    /// Positions (in the list `new_children` returns) of the children this
    /// one waits for (T-11).
    pub depends_on: Vec<usize>,
}

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
    /// A template of `parent` (a TODO without a parent) and its `children`:
    /// title, priority and memo of each, the children in the order they were
    /// made (`id`), 中止 ones left out. A child's `due_offset_days` is its
    /// 日限 minus the parent's in calendar days (also with the 営業日
    /// setting), `None` when either is empty. `dependencies` may hold any
    /// dependencies: those between two kept children become `depends_on`
    /// (T-11). Not checked: see [`TemplateBody::checked`].
    pub fn from_todos(
        parent: &Todo,
        children: &[Todo],
        dependencies: &[Dependency],
    ) -> TemplateBody {
        let mut kept: Vec<&Todo> = children
            .iter()
            .filter(|c| c.status != Status::Cancelled)
            .collect();
        kept.sort_by_key(|c| c.id);
        let ids: Vec<i64> = kept.iter().map(|c| c.id).collect();
        let mut waits: Vec<Vec<usize>> = vec![Vec::new(); kept.len()];
        for (later, first) in positions_among(&ids, dependencies) {
            waits[later].push(first);
        }
        TemplateBody {
            title: parent.title.clone(),
            priority: parent.priority,
            memo_md: parent.memo_md.clone(),
            children: kept
                .into_iter()
                .zip(waits)
                .map(|(c, depends_on)| TemplateChild {
                    title: c.title.clone(),
                    priority: c.priority,
                    memo_md: c.memo_md.clone(),
                    due_offset_days: match (parent.due_date, c.due_date) {
                        (Some(p), Some(d)) => Some(days_between(p, d)),
                        _ => None,
                    },
                    depends_on,
                })
                .collect(),
        }
    }

    /// The children to create under a new parent due on `due`, in the
    /// template's order, each with the positions of the children it waits
    /// for. 子の日限 = `due` + `due_offset_days` (calendar days), empty when
    /// either is empty (or the date would be out of range).
    pub fn new_children(&self, due: Option<NaiveDate>) -> Vec<NewChild> {
        self.children
            .iter()
            .map(|c| NewChild {
                title: c.title.clone(),
                priority: c.priority,
                memo_md: c.memo_md.clone(),
                due_date: due
                    .zip(c.due_offset_days)
                    .and_then(|(due, days)| due.checked_add_signed(TimeDelta::try_days(days)?)),
                depends_on: c.depends_on.clone(),
            })
            .collect()
    }

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

/// A template as the screens list it (list_todo_templates): what the
/// 「TODOを追加」 dialog fills in and shows, and the settings card's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TemplateSummary {
    pub id: i64,
    pub name: String,
    /// The parent's title (the dialog puts it in the title field).
    pub title: String,
    pub priority: Priority,
    /// The children's titles, in the template's order.
    pub child_titles: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

impl TodoTemplate {
    pub fn summary(&self) -> TemplateSummary {
        TemplateSummary {
            id: self.id,
            name: self.name.clone(),
            title: self.body.title.clone(),
            priority: self.body.priority,
            child_titles: self.body.children.iter().map(|c| c.title.clone()).collect(),
            updated_at: self.updated_at,
        }
    }
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

    // ---- フェーズ20: making templates and TODOs from them -------------------

    use crate::domain::testing::{d, todo};

    fn task(id: i64, title: &str, status: Status, due: Option<&str>) -> Todo {
        Todo {
            id,
            title: title.into(),
            due_date: due.map(d),
            ..todo(status)
        }
    }

    #[test]
    fn a_template_keeps_the_parent_and_its_children_but_cancelled_ones() {
        let parent = Todo {
            priority: Priority::High,
            memo_md: "## 手順\n- 集計する".into(),
            planned_start: Some(d("2026-11-24")),
            ..task(1, "月次締め", Status::Doing, Some("2026-11-30"))
        };
        let mut done = task(4, "経費の確認", Status::Done, Some("2026-11-30"));
        done.memo_md = "- 領収書".into();
        done.priority = Priority::Low;
        // Given out of order: the template keeps the id order.
        let children = [
            task(5, "報告書の作成", Status::Todo, Some("2026-12-02")),
            task(3, "中止した子", Status::Cancelled, Some("2026-11-28")),
            done,
            task(2, "売上の集計", Status::Waiting, Some("2026-11-27")),
        ];
        let b = TemplateBody::from_todos(&parent, &children, &[]);
        assert_eq!(b.title, "月次締め");
        assert_eq!(b.priority, Priority::High);
        assert_eq!(b.memo_md, "## 手順\n- 集計する");
        let kept: Vec<(&str, Option<i64>)> = b
            .children
            .iter()
            .map(|c| (c.title.as_str(), c.due_offset_days))
            .collect();
        // Before, the same day, after (calendar days, across a month).
        assert_eq!(
            kept,
            [
                ("売上の集計", Some(-3)),
                ("経費の確認", Some(0)),
                ("報告書の作成", Some(2)),
            ]
        );
        assert_eq!(b.children[1].priority, Priority::Low);
        assert_eq!(b.children[1].memo_md, "- 領収書");
        assert!(b.children.iter().all(|c| c.depends_on.is_empty()));
        assert!(b.checked().is_ok());
    }

    #[test]
    fn a_template_keeps_the_dependencies_between_its_children() {
        let dep = |todo_id: i64, depends_on_id: i64| Dependency {
            todo_id,
            depends_on_id,
            created_at: crate::domain::testing::ts(2026, 11, 19, 0, 0),
        };
        let parent = task(1, "月次締め", Status::Todo, Some("2026-11-30"));
        let children = [
            task(5, "報告", Status::Todo, None),
            task(3, "中止した子", Status::Cancelled, None),
            task(2, "集計", Status::Done, None),
            task(4, "確認", Status::Todo, None),
        ];
        // 報告 waits for 確認 and 集計; 確認 waits for 集計 and the cancelled
        // child; 集計 waits for a TODO outside the family.
        let deps = [
            dep(5, 4),
            dep(5, 2),
            dep(4, 2),
            dep(4, 3),
            dep(2, 99),
            dep(3, 2),
        ];
        let b = TemplateBody::from_todos(&parent, &children, &deps);
        let shape: Vec<(&str, &[usize])> = b
            .children
            .iter()
            .map(|c| (c.title.as_str(), c.depends_on.as_slice()))
            .collect();
        // Positions: 集計 0, 確認 1, 報告 2.
        assert_eq!(
            shape,
            [("集計", &[][..]), ("確認", &[0][..]), ("報告", &[0, 1][..])]
        );
        let b = b.checked().unwrap();
        // Used again, the children wait for the same positions.
        let made = b.new_children(None);
        assert_eq!(
            made.iter()
                .map(|c| c.depends_on.clone())
                .collect::<Vec<_>>(),
            [vec![], vec![0], vec![0, 1]]
        );
    }

    #[test]
    fn an_empty_due_date_leaves_the_offset_out() {
        let parent = task(1, "親", Status::Todo, Some("2026-11-30"));
        let b = TemplateBody::from_todos(&parent, &[task(2, "日限なし", Status::Todo, None)], &[]);
        assert_eq!(b.children[0].due_offset_days, None);

        let undated = task(1, "親", Status::Todo, None);
        let b = TemplateBody::from_todos(
            &undated,
            &[task(2, "日限あり", Status::Todo, Some("2026-11-30"))],
            &[],
        );
        assert_eq!(b.children[0].due_offset_days, None);

        // No children at all: a template of one TODO.
        assert!(
            TemplateBody::from_todos(&parent, &[], &[])
                .children
                .is_empty()
        );
    }

    #[test]
    fn children_are_due_the_offset_from_the_new_due_date() {
        let mut b = body(vec![
            child("売上の集計", vec![]),
            child("報告書の作成", vec![]),
            child("日限なし", vec![]),
            child("後ろ", vec![]),
        ]);
        b.children[0].due_offset_days = Some(-3);
        b.children[0].priority = Priority::High;
        b.children[0].memo_md = "- 前月分".into();
        b.children[1].due_offset_days = Some(0);
        b.children[3].due_offset_days = Some(5);

        // Across a month and a year.
        let made = b.new_children(Some(d("2026-12-29")));
        let dues: Vec<(&str, Option<NaiveDate>)> = made
            .iter()
            .map(|c| (c.title.as_str(), c.due_date))
            .collect();
        assert_eq!(
            dues,
            [
                ("売上の集計", Some(d("2026-12-26"))),
                ("報告書の作成", Some(d("2026-12-29"))),
                ("日限なし", None),
                ("後ろ", Some(d("2027-01-03"))),
            ]
        );
        assert_eq!(made[0].priority, Priority::High);
        assert_eq!(made[0].memo_md, "- 前月分");
        // A day back from 3/1 lands on 2/29 in a leap year (calendar days).
        b.children[0].due_offset_days = Some(-1);
        assert_eq!(
            b.new_children(Some(d("2028-03-01")))[0].due_date,
            Some(d("2028-02-29"))
        );

        // No due date entered: no child gets one.
        assert!(b.new_children(None).iter().all(|c| c.due_date.is_none()));
    }

    #[test]
    fn an_offset_out_of_range_gives_no_due_date() {
        let mut b = body(vec![child("a", vec![]), child("b", vec![])]);
        b.children[0].due_offset_days = Some(i64::MAX);
        b.children[1].due_offset_days = Some(-1_000_000_000);
        assert!(
            b.new_children(Some(d("2026-11-30")))
                .iter()
                .all(|c| c.due_date.is_none())
        );
    }

    #[test]
    fn a_template_and_back_gives_the_same_dates() {
        let parent = task(1, "月次締め", Status::Done, Some("2026-11-30"));
        let children = [
            task(2, "a", Status::Done, Some("2026-11-25")),
            task(3, "b", Status::Todo, Some("2026-12-01")),
        ];
        let made = TemplateBody::from_todos(&parent, &children, &[]).new_children(parent.due_date);
        assert_eq!(
            made.iter().map(|c| c.due_date).collect::<Vec<_>>(),
            [Some(d("2026-11-25")), Some(d("2026-12-01"))]
        );
    }

    #[test]
    fn names_are_trimmed_and_1_to_40_characters() {
        assert_eq!(checked_name("  月次締め \t").unwrap(), "月次締め");
        assert_eq!(checked_name(" \u{3000} "), Err(NameError::Empty));
        assert_eq!(checked_name(""), Err(NameError::Empty));
        // Characters, not bytes: 40 kanji pass, 41 do not.
        let forty = "締".repeat(40);
        assert_eq!(checked_name(&format!(" {forty} ")).unwrap(), forty);
        assert_eq!(checked_name(&"締".repeat(41)), Err(NameError::TooLong));
        assert_eq!(
            NameError::TooLong.to_string(),
            "テンプレートの名前は40字までにしてください。"
        );
    }

    #[test]
    fn the_summary_lists_the_children_titles() {
        let mut b = body(vec![
            child("売上の集計", vec![]),
            child("報告書の作成", vec![0]),
        ]);
        b.priority = Priority::High;
        let t = TodoTemplate {
            id: 7,
            name: "月次".into(),
            body: b,
            created_at: crate::domain::testing::ts(2026, 11, 19, 0, 0),
            updated_at: crate::domain::testing::ts(2026, 11, 20, 0, 0),
        };
        let s = t.summary();
        assert_eq!(s.id, 7);
        assert_eq!(s.name, "月次");
        assert_eq!(s.title, "月次締め");
        assert_eq!(s.priority, Priority::High);
        assert_eq!(s.child_titles, ["売上の集計", "報告書の作成"]);
        assert_eq!(s.updated_at, t.updated_at);
    }
}
