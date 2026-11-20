//! Dashboard and startup reminder (D-1, D-2, D-5, D-8). Pure.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::Serialize;

use super::model::{Project, Status, Todo};
use super::settings::{ReminderFrequency, Settings};
use super::subtodo::Family;
use super::urgency::{Rules, Urgency, attention_order};
use super::views::{ProjectSummary, TodoItem, project_summaries, todo_item};

/// Number of TODOs in each 日限注意 category.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct UrgencyCounts {
    pub overdue: i64,
    pub due_today: i64,
    pub due_soon: i64,
    pub follow_up: i64,
    pub late_start: i64,
}

impl UrgencyCounts {
    fn add(&mut self, u: Urgency) {
        let slot = match u {
            Urgency::Overdue => &mut self.overdue,
            Urgency::DueToday => &mut self.due_today,
            Urgency::DueSoon => &mut self.due_soon,
            Urgency::FollowUp => &mut self.follow_up,
            Urgency::LateStart => &mut self.late_start,
        };
        *slot += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Dashboard {
    pub today: NaiveDate,
    /// 「N日以内に日限が来ます」.
    pub soon_days: u32,
    /// Day counts are business days (「N営業日以内」).
    pub business_days: bool,
    pub counts: UrgencyCounts,
    /// All 日限注意 (the side-nav badge of the dashboard).
    pub attention_total: i64,
    /// 日限注意, most severe category first, then oldest due date.
    pub attention: Vec<TodoItem>,
    /// 今日の作業: doing, then not-started TODOs whose planned period contains today.
    pub today_work: Vec<TodoItem>,
    /// 待ち: next follow-up date first.
    pub waiting: Vec<TodoItem>,
    /// プロジェクトの進捗 (active projects).
    pub projects: Vec<ProjectSummary>,
}

/// Whether today falls in the planned period. A missing end (or start) leaves
/// that side open; no planned dates at all means no.
pub fn in_planned_period(todo: &Todo, today: NaiveDate) -> bool {
    if todo.planned_start.is_none() && todo.planned_end.is_none() {
        return false;
    }
    todo.planned_start.is_none_or(|s| s <= today) && todo.planned_end.is_none_or(|e| today <= e)
}

/// D-5 as decided: 作業中, and 未着手 whose planned period contains today.
/// 待ち and 保留 are not included.
pub fn is_today_work(todo: &Todo, today: NaiveDate) -> bool {
    match todo.status {
        Status::Doing => true,
        Status::Todo => in_planned_period(todo, today),
        _ => false,
    }
}

fn items(
    todos: &[Todo],
    names: &HashMap<i64, String>,
    today: NaiveDate,
    rules: &Rules,
) -> Vec<TodoItem> {
    // Children show 「親 / 子」 (S-1).
    let family = Family::new(todos);
    todos
        .iter()
        .map(|t| {
            let name = names.get(&t.project_id).cloned().unwrap_or_default();
            todo_item(t.clone(), name, &family, today, rules)
        })
        .collect()
}

/// The 日限注意 list in display order.
pub fn attention_list(items: &[TodoItem]) -> Vec<TodoItem> {
    let mut out: Vec<TodoItem> = items
        .iter()
        .filter(|i| i.assessment.urgency.is_some())
        .cloned()
        .collect();
    out.sort_by(|a, b| {
        attention_order(
            (&a.todo, a.assessment.urgency.unwrap_or(Urgency::LateStart)),
            (&b.todo, b.assessment.urgency.unwrap_or(Urgency::LateStart)),
        )
    });
    out
}

/// Builds the dashboard from the active projects and their TODOs (in the
/// repository's order: project order, then TODO order).
pub fn build(projects: Vec<Project>, todos: &[Todo], today: NaiveDate, rules: &Rules) -> Dashboard {
    let names: HashMap<i64, String> = projects.iter().map(|p| (p.id, p.name.clone())).collect();
    let all = items(todos, &names, today, rules);

    let attention = attention_list(&all);
    let mut counts = UrgencyCounts::default();
    for i in &attention {
        if let Some(u) = i.assessment.urgency {
            counts.add(u);
        }
    }

    let mut today_work: Vec<TodoItem> = all
        .iter()
        .filter(|i| is_today_work(&i.todo, today))
        .cloned()
        .collect();
    today_work.sort_by_key(|i| i.todo.status != Status::Doing);

    let mut waiting: Vec<TodoItem> = all
        .iter()
        .filter(|i| i.todo.status == Status::Waiting)
        .cloned()
        .collect();
    waiting.sort_by(|a, b| {
        let key = |i: &TodoItem| {
            (
                i.todo.follow_up_date.is_none(),
                i.todo.follow_up_date,
                i.todo.waiting_since,
                i.todo.id,
            )
        };
        key(a).cmp(&key(b))
    });

    Dashboard {
        today,
        soon_days: rules.soon_days,
        business_days: rules.business(),
        attention_total: attention.len() as i64,
        counts,
        attention,
        today_work,
        waiting,
        projects: project_summaries(projects, todos, today, rules),
    }
}

/// What the startup reminder dialog shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct StartupReminder {
    pub today: NaiveDate,
    pub frequency: ReminderFrequency,
    /// 日限注意 in display order (never empty).
    pub items: Vec<TodoItem>,
}

/// Whether the frequency setting allows showing the reminder today.
pub fn reminder_allowed(settings: &Settings, today: NaiveDate) -> bool {
    match settings.reminder_frequency {
        ReminderFrequency::EveryLaunch => true,
        ReminderFrequency::OncePerDay => settings.last_reminded_on != Some(today),
    }
}

/// The reminder to show now, if any: allowed by the setting and at least one 日限注意.
pub fn startup_reminder(
    settings: &Settings,
    attention: Vec<TodoItem>,
    today: NaiveDate,
) -> Option<StartupReminder> {
    if !reminder_allowed(settings, today) || attention.is_empty() {
        return None;
    }
    Some(StartupReminder {
        today,
        frequency: settings.reminder_frequency,
        items: attention,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Priority;
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

    struct Row {
        id: i64,
        project: i64,
        title: &'static str,
        status: Status,
        due: &'static str,
        planned: (&'static str, &'static str),
        waiting: Option<(&'static str, &'static str)>,
    }

    /// The mock's 11 TODOs on 2026-11-19 (docs/ui-mock/README.md).
    fn mock_todos() -> Vec<Todo> {
        let rows = [
            Row {
                id: 1,
                project: 1,
                title: "移行手順書のドラフト",
                status: Status::Doing,
                due: "2026-11-19",
                planned: ("2026-11-11", "2026-11-18"),
                waiting: None,
            },
            Row {
                id: 2,
                project: 1,
                title: "API設計書のレビュー対応",
                status: Status::Waiting,
                due: "2026-11-20",
                planned: ("2026-11-16", "2026-11-19"),
                waiting: Some(("2026-11-19", "2026-11-20")),
            },
            Row {
                id: 3,
                project: 1,
                title: "結合テスト計画の作成",
                status: Status::Todo,
                due: "2026-11-30",
                planned: ("2026-11-20", "2026-11-27"),
                waiting: None,
            },
            Row {
                id: 4,
                project: 1,
                title: "性能試験の環境準備",
                status: Status::Todo,
                due: "2026-12-04",
                planned: ("2026-11-17", "2026-11-20"),
                waiting: None,
            },
            Row {
                id: 5,
                project: 1,
                title: "要件定義書の改訂",
                status: Status::Done,
                due: "2026-11-13",
                planned: ("2026-11-09", "2026-11-13"),
                waiting: None,
            },
            Row {
                id: 6,
                project: 1,
                title: "旧帳票の移行検証",
                status: Status::Cancelled,
                due: "2026-11-27",
                planned: ("2026-11-24", "2026-11-26"),
                waiting: None,
            },
            Row {
                id: 7,
                project: 2,
                title: "ライセンス棚卸し",
                status: Status::Todo,
                due: "2026-11-17",
                planned: ("2026-11-12", "2026-11-13"),
                waiting: None,
            },
            Row {
                id: 8,
                project: 2,
                title: "デプロイ手順書の更新",
                status: Status::Done,
                due: "2026-11-24",
                planned: ("2026-11-18", "2026-11-19"),
                waiting: None,
            },
            Row {
                id: 9,
                project: 2,
                title: "監視ダッシュボードの整理",
                status: Status::Hold,
                due: "2026-12-03",
                planned: ("2026-11-24", "2026-11-27"),
                waiting: None,
            },
            Row {
                id: 10,
                project: 3,
                title: "募集要項ページの原稿確認",
                status: Status::Waiting,
                due: "2026-11-27",
                planned: ("2026-11-16", "2026-11-20"),
                waiting: Some(("2026-11-17", "2026-11-19")),
            },
            Row {
                id: 11,
                project: 3,
                title: "応募フォームの項目見直し",
                status: Status::Todo,
                due: "2026-12-02",
                planned: ("2026-11-25", "2026-11-27"),
                waiting: None,
            },
        ];
        rows.into_iter()
            .map(|r| Todo {
                id: r.id,
                project_id: r.project,
                title: r.title.into(),
                status: r.status,
                priority: if r.id == 2 {
                    Priority::High
                } else {
                    Priority::Medium
                },
                due_date: Some(d(r.due)),
                planned_start: Some(d(r.planned.0)),
                planned_end: Some(d(r.planned.1)),
                waiting_on: r.waiting.map(|_| "相手".to_owned()),
                waiting_since: r.waiting.map(|w| d(w.0)),
                follow_up_date: r.waiting.map(|w| d(w.1)),
                ..todo(r.status)
            })
            .collect()
    }

    fn mock_dashboard() -> Dashboard {
        build(
            vec![
                project(1, "基幹システム更改"),
                project(2, "社内ツール整備"),
                project(3, "採用サイト改修"),
            ],
            &mock_todos(),
            d("2026-11-19"),
            &rules(3),
        )
    }

    #[test]
    fn business_days_keep_the_mock_counts_on_its_day() {
        let calendar = mock_dashboard();
        assert!(!calendar.business_days);
        let business = build(
            vec![
                project(1, "基幹システム更改"),
                project(2, "社内ツール整備"),
                project(3, "採用サイト改修"),
            ],
            &mock_todos(),
            d("2026-11-19"),
            &business_rules(3, vec![]),
        );
        assert!(business.business_days);
        assert_eq!(business.soon_days, 3);
        assert_eq!(business.counts, calendar.counts);
        assert!(
            business
                .attention
                .iter()
                .all(|i| i.assessment.business_days)
        );
    }

    fn titles(items: &[TodoItem]) -> Vec<&str> {
        items.iter().map(|i| i.todo.title.as_str()).collect()
    }

    #[test]
    fn attention_matches_the_mock() {
        let dash = mock_dashboard();
        assert_eq!(dash.attention_total, 5);
        assert_eq!(
            dash.counts,
            UrgencyCounts {
                overdue: 1,
                due_today: 1,
                due_soon: 1,
                follow_up: 1,
                late_start: 1
            }
        );
        assert_eq!(
            titles(&dash.attention),
            vec![
                "ライセンス棚卸し",
                "移行手順書のドラフト",
                "API設計書のレビュー対応",
                "募集要項ページの原稿確認",
                "性能試験の環境準備",
            ]
        );
        assert_eq!(dash.attention[0].project_name, "社内ツール整備");
        assert_eq!(dash.attention[3].assessment.waiting_day, Some(3));
        assert_eq!(dash.attention[3].assessment.days_since_follow_up, Some(0));
    }

    #[test]
    fn today_work_and_waiting_match_the_mock() {
        let dash = mock_dashboard();
        // Waiting (API, 募集要項) and on-hold TODOs are not today's work.
        assert_eq!(
            titles(&dash.today_work),
            vec!["移行手順書のドラフト", "性能試験の環境準備"]
        );
        assert_eq!(
            titles(&dash.waiting),
            vec!["募集要項ページの原稿確認", "API設計書のレビュー対応"]
        );
    }

    #[test]
    fn progress_matches_the_mock() {
        let dash = mock_dashboard();
        let p: Vec<_> = dash
            .projects
            .iter()
            .map(|s| {
                (
                    s.project.name.as_str(),
                    s.progress_done,
                    s.progress_total,
                    s.attention_count,
                )
            })
            .collect();
        assert_eq!(
            p,
            vec![
                ("基幹システム更改", 1, 5, 3),
                ("社内ツール整備", 1, 3, 1),
                ("採用サイト改修", 0, 2, 1)
            ]
        );
    }

    #[test]
    fn planned_period_with_open_ends() {
        let today = d("2026-11-19");
        let t = |s: Option<&str>, e: Option<&str>| Todo {
            planned_start: s.map(d),
            planned_end: e.map(d),
            ..todo(Status::Todo)
        };
        assert!(in_planned_period(
            &t(Some("2026-11-19"), Some("2026-11-19")),
            today
        ));
        assert!(in_planned_period(&t(Some("2026-11-10"), None), today));
        assert!(in_planned_period(&t(None, Some("2026-11-30")), today));
        assert!(!in_planned_period(&t(Some("2026-11-20"), None), today));
        assert!(!in_planned_period(&t(None, Some("2026-11-18")), today));
        assert!(!in_planned_period(&t(None, None), today));
    }

    #[test]
    fn hold_and_waiting_are_never_today_work() {
        let today = d("2026-11-19");
        for status in [
            Status::Hold,
            Status::Waiting,
            Status::Done,
            Status::Cancelled,
        ] {
            let t = Todo {
                planned_start: Some(d("2026-11-18")),
                planned_end: Some(d("2026-11-20")),
                ..todo(status)
            };
            assert!(!is_today_work(&t, today), "{status:?}");
        }
    }

    #[test]
    fn reminder_frequency() {
        let today = d("2026-11-19");
        let every = Settings::default();
        let daily = Settings {
            reminder_frequency: ReminderFrequency::OncePerDay,
            ..Settings::default()
        };
        assert!(reminder_allowed(&every, today));
        assert!(reminder_allowed(
            &Settings {
                last_reminded_on: Some(today),
                ..every.clone()
            },
            today
        ));
        assert!(reminder_allowed(&daily, today));
        assert!(reminder_allowed(
            &Settings {
                last_reminded_on: Some(d("2026-11-18")),
                ..daily.clone()
            },
            today
        ));
        assert!(!reminder_allowed(
            &Settings {
                last_reminded_on: Some(today),
                ..daily.clone()
            },
            today
        ));
    }

    #[test]
    fn no_reminder_without_attention() {
        let today = d("2026-11-19");
        assert_eq!(startup_reminder(&Settings::default(), vec![], today), None);
        let dash = mock_dashboard();
        let r = startup_reminder(&Settings::default(), dash.attention.clone(), today).unwrap();
        assert_eq!(r.items.len(), 5);
        assert_eq!(r.frequency, ReminderFrequency::EveryLaunch);
    }

    #[test]
    fn a_child_is_judged_by_itself_and_carries_its_parent() {
        let today = d("2026-11-19");
        let mut todos = mock_todos();
        let parent_id = todos[0].id;
        todos.push(Todo {
            id: 100,
            project_id: 1,
            parent_id: Some(parent_id),
            title: "切り戻し手順の確認".into(),
            due_date: Some(d("2026-11-18")),
            ..todo(Status::Todo)
        });
        let dash = build(
            vec![project(1, "基幹システム更改"), project(2, "社内ツール整備")],
            &todos,
            today,
            &rules(3),
        );
        assert_eq!(dash.counts.overdue, 2);
        let child = dash.attention.iter().find(|i| i.todo.id == 100).unwrap();
        assert_eq!(child.parent_title.as_deref(), Some("移行手順書のドラフト"));
        assert_eq!(child.assessment.urgency, Some(Urgency::Overdue));
        let parent = dash
            .attention
            .iter()
            .find(|i| i.todo.id == parent_id)
            .unwrap();
        assert_eq!(parent.children.map(|c| c.open), Some(1));
    }
}
