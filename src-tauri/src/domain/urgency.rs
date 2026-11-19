//! 日限注意の判定 (urgency classification).
//!
//! `classify` is the single source of truth used by the dashboard, the startup
//! reminder, the list badges and the daily report.

use std::cmp::Ordering;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::date::{add_days, days_between};
use super::model::{Status, Todo};

/// Urgency category. Declared in severity order (most severe first), so the
/// derived `Ord` sorts 超過 → 本日 → 間近 → 要確認 → 着手遅れ.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    /// 超過: due date is before today.
    Overdue,
    /// 本日: due date is today.
    DueToday,
    /// 間近: due date is between tomorrow and `soon_days` days from today.
    DueSoon,
    /// 要確認: waiting, and the follow-up date is today or earlier.
    FollowUp,
    /// 着手遅れ: not started, and the planned start is before today.
    LateStart,
}

impl Urgency {
    pub const ALL: [Urgency; 5] = [
        Urgency::Overdue,
        Urgency::DueToday,
        Urgency::DueSoon,
        Urgency::FollowUp,
        Urgency::LateStart,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Urgency::Overdue => "超過",
            Urgency::DueToday => "本日",
            Urgency::DueSoon => "間近",
            Urgency::FollowUp => "要確認",
            Urgency::LateStart => "着手遅れ",
        }
    }
}

/// Classifies a TODO. Closed TODOs (完了・中止) are never urgent. The rules are
/// checked top to bottom and the first match wins. 保留 is classified like any
/// other open status (open question in the design, kept as assumed).
pub fn classify(todo: &Todo, today: NaiveDate, soon_days: u32) -> Option<Urgency> {
    if todo.status.is_closed() {
        return None;
    }
    if let Some(due) = todo.due_date {
        if due < today {
            return Some(Urgency::Overdue);
        }
        if due == today {
            return Some(Urgency::DueToday);
        }
        if due <= add_days(today, soon_days) {
            return Some(Urgency::DueSoon);
        }
    }
    if todo.status == Status::Waiting && todo.follow_up_date.is_some_and(|f| f <= today) {
        return Some(Urgency::FollowUp);
    }
    if todo.status == Status::Todo && todo.planned_start.is_some_and(|s| s < today) {
        return Some(Urgency::LateStart);
    }
    None
}

/// The two small warning icons shown in the list and the Gantt chart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct Warnings {
    /// 計画が間に合わない: planned end is after the due date.
    pub plan_misses_due: bool,
    /// 予定超過: doing or waiting, and the planned end is before today.
    pub plan_overrun: bool,
}

/// Warnings apply to open TODOs only.
pub fn warnings(todo: &Todo, today: NaiveDate) -> Warnings {
    if todo.status.is_closed() {
        return Warnings::default();
    }
    Warnings {
        plan_misses_due: plan_misses_due(todo.planned_end, todo.due_date),
        plan_overrun: matches!(todo.status, Status::Doing | Status::Waiting)
            && todo.planned_end.is_some_and(|end| end < today),
    }
}

/// True when the planned end is after the due date (T-6 input warning).
pub fn plan_misses_due(planned_end: Option<NaiveDate>, due_date: Option<NaiveDate>) -> bool {
    matches!((planned_end, due_date), (Some(end), Some(due)) if end > due)
}

/// Ordering of urgent TODOs: category severity, then due date (oldest first,
/// missing last), then priority (high first), then id.
pub fn attention_order(a: (&Todo, Urgency), b: (&Todo, Urgency)) -> Ordering {
    let (ta, ua) = a;
    let (tb, ub) = b;
    ua.cmp(&ub)
        .then_with(|| match (ta.due_date, tb.due_date) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| ta.priority.cmp(&tb.priority))
        .then_with(|| ta.id.cmp(&tb.id))
}

/// Everything the screens show about a TODO's urgency, computed once here so
/// the frontend never recalculates categories or day counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct Assessment {
    pub urgency: Option<Urgency>,
    pub warnings: Warnings,
    /// Days from today to the due date; negative when overdue (「2日超過」 is -2,
    /// 「あと1日」 is 1). Open TODOs with a due date only.
    pub days_until_due: Option<i64>,
    /// Days since the planned start (「予定開始から2日経過」). 着手遅れ only.
    pub days_since_planned_start: Option<i64>,
    /// Day of waiting, the first day being 1 (「3日目」). 待ち only.
    pub waiting_day: Option<i64>,
    /// Days since the follow-up date (0 = 「確認日が今日」). 要確認 only.
    pub days_since_follow_up: Option<i64>,
}

pub fn assess(todo: &Todo, today: NaiveDate, soon_days: u32) -> Assessment {
    let urgency = classify(todo, today, soon_days);
    let open = todo.status.is_open();
    Assessment {
        urgency,
        warnings: warnings(todo, today),
        days_until_due: todo
            .due_date
            .filter(|_| open)
            .map(|due| days_left(due, today)),
        days_since_planned_start: todo
            .planned_start
            .filter(|_| urgency == Some(Urgency::LateStart))
            .map(|start| days_since_planned_start(start, today)),
        waiting_day: todo
            .waiting_since
            .filter(|_| todo.status == Status::Waiting)
            .map(|since| waiting_day(since, today)),
        days_since_follow_up: todo
            .follow_up_date
            .filter(|_| urgency == Some(Urgency::FollowUp))
            .map(|f| days_between(f, today)),
    }
}

/// Days past the due date (「2日超過」). Positive when overdue.
pub fn days_overdue(due: NaiveDate, today: NaiveDate) -> i64 {
    days_between(due, today)
}

/// Days left until the due date (「あと1日」). Positive when in the future.
pub fn days_left(due: NaiveDate, today: NaiveDate) -> i64 {
    days_between(today, due)
}

/// Days since the planned start (「予定開始から2日経過」).
pub fn days_since_planned_start(planned_start: NaiveDate, today: NaiveDate) -> i64 {
    days_between(planned_start, today)
}

/// Which day of waiting today is, counting the start day as day 1 (「3日目」).
pub fn waiting_day(waiting_since: NaiveDate, today: NaiveDate) -> i64 {
    days_between(waiting_since, today) + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Priority;
    use crate::domain::testing::{d, todo};

    const SOON: u32 = 3;

    fn with_due(status: Status, due: &str) -> Todo {
        Todo {
            due_date: Some(d(due)),
            ..todo(status)
        }
    }

    // ---- due-date boundaries -------------------------------------------

    #[test]
    fn due_date_boundaries() {
        let today = d("2026-11-19");
        let cases = [
            ("2026-11-18", Some(Urgency::Overdue)),  // yesterday
            ("2026-11-19", Some(Urgency::DueToday)), // today
            ("2026-11-20", Some(Urgency::DueSoon)),  // tomorrow
            ("2026-11-22", Some(Urgency::DueSoon)),  // exactly N days later
            ("2026-11-23", None),                    // N + 1 days later
            ("2025-01-01", Some(Urgency::Overdue)),  // long ago
        ];
        for (due, expected) in cases {
            assert_eq!(
                classify(&with_due(Status::Todo, due), today, SOON),
                expected,
                "due {due}"
            );
        }
    }

    #[test]
    fn due_soon_across_month_end() {
        let today = d("2026-11-29");
        assert_eq!(
            classify(&with_due(Status::Doing, "2026-12-02"), today, SOON),
            Some(Urgency::DueSoon)
        );
        assert_eq!(
            classify(&with_due(Status::Doing, "2026-12-03"), today, SOON),
            None
        );
        assert_eq!(
            classify(&with_due(Status::Doing, "2026-11-30"), today, SOON),
            Some(Urgency::DueSoon)
        );
    }

    #[test]
    fn due_soon_and_overdue_across_year_end() {
        let today = d("2026-12-30");
        assert_eq!(
            classify(&with_due(Status::Todo, "2027-01-02"), today, SOON),
            Some(Urgency::DueSoon)
        );
        assert_eq!(
            classify(&with_due(Status::Todo, "2027-01-03"), today, SOON),
            None
        );

        let new_year = d("2027-01-01");
        assert_eq!(
            classify(&with_due(Status::Todo, "2026-12-31"), new_year, SOON),
            Some(Urgency::Overdue)
        );
    }

    #[test]
    fn soon_days_setting_is_respected() {
        let today = d("2026-11-19");
        let t = with_due(Status::Todo, "2026-11-26"); // 7 days later
        assert_eq!(classify(&t, today, 3), None);
        assert_eq!(classify(&t, today, 7), Some(Urgency::DueSoon));
        // N = 0: nothing is "soon", but today and overdue still apply.
        assert_eq!(
            classify(&with_due(Status::Todo, "2026-11-20"), today, 0),
            None
        );
        assert_eq!(
            classify(&with_due(Status::Todo, "2026-11-19"), today, 0),
            Some(Urgency::DueToday)
        );
    }

    // ---- follow-up and late start ----------------------------------------

    #[test]
    fn follow_up_date_boundaries() {
        let today = d("2026-11-19");
        let waiting = |f: &str| Todo {
            follow_up_date: Some(d(f)),
            ..todo(Status::Waiting)
        };
        assert_eq!(
            classify(&waiting("2026-11-18"), today, SOON),
            Some(Urgency::FollowUp)
        );
        assert_eq!(
            classify(&waiting("2026-11-19"), today, SOON),
            Some(Urgency::FollowUp)
        );
        assert_eq!(classify(&waiting("2026-11-20"), today, SOON), None);
    }

    #[test]
    fn follow_up_only_applies_to_waiting() {
        let today = d("2026-11-19");
        for status in [Status::Todo, Status::Doing, Status::Hold] {
            let t = Todo {
                follow_up_date: Some(d("2026-11-18")),
                ..todo(status)
            };
            assert_eq!(classify(&t, today, SOON), None, "{status:?}");
        }
    }

    #[test]
    fn late_start_boundaries() {
        let today = d("2026-11-19");
        let planned = |s: &str| Todo {
            planned_start: Some(d(s)),
            ..todo(Status::Todo)
        };
        assert_eq!(
            classify(&planned("2026-11-18"), today, SOON),
            Some(Urgency::LateStart)
        );
        assert_eq!(classify(&planned("2026-11-19"), today, SOON), None);
        assert_eq!(classify(&planned("2026-11-20"), today, SOON), None);
    }

    #[test]
    fn late_start_only_applies_to_not_started() {
        let today = d("2026-11-19");
        for status in [Status::Doing, Status::Waiting, Status::Hold] {
            let t = Todo {
                planned_start: Some(d("2026-11-10")),
                ..todo(status)
            };
            assert_eq!(classify(&t, today, SOON), None, "{status:?}");
        }
    }

    // ---- priority between categories --------------------------------------

    #[test]
    fn first_matching_category_wins() {
        let today = d("2026-11-19");
        // Overdue beats late start.
        let t = Todo {
            due_date: Some(d("2026-11-17")),
            planned_start: Some(d("2026-11-12")),
            ..todo(Status::Todo)
        };
        assert_eq!(classify(&t, today, SOON), Some(Urgency::Overdue));

        // Waiting with a due date soon and a follow-up due: 間近 wins.
        let t = Todo {
            due_date: Some(d("2026-11-20")),
            follow_up_date: Some(d("2026-11-19")),
            ..todo(Status::Waiting)
        };
        assert_eq!(classify(&t, today, SOON), Some(Urgency::DueSoon));

        // Waiting, due today: 本日 wins over 要確認.
        let t = Todo {
            due_date: Some(d("2026-11-19")),
            follow_up_date: Some(d("2026-11-18")),
            ..todo(Status::Waiting)
        };
        assert_eq!(classify(&t, today, SOON), Some(Urgency::DueToday));

        // Waiting, due far away, follow-up due: 要確認.
        let t = Todo {
            due_date: Some(d("2026-12-25")),
            follow_up_date: Some(d("2026-11-19")),
            ..todo(Status::Waiting)
        };
        assert_eq!(classify(&t, today, SOON), Some(Urgency::FollowUp));
    }

    #[test]
    fn closed_todos_are_never_urgent() {
        let today = d("2026-11-19");
        for status in [Status::Done, Status::Cancelled] {
            let t = Todo {
                due_date: Some(d("2026-11-01")),
                planned_start: Some(d("2026-11-01")),
                follow_up_date: Some(d("2026-11-01")),
                ..todo(status)
            };
            assert_eq!(classify(&t, today, SOON), None, "{status:?}");
        }
    }

    #[test]
    fn hold_is_still_classified_by_due_date() {
        let today = d("2026-11-19");
        assert_eq!(
            classify(&with_due(Status::Hold, "2026-11-18"), today, SOON),
            Some(Urgency::Overdue)
        );
        assert_eq!(
            classify(&with_due(Status::Hold, "2026-11-21"), today, SOON),
            Some(Urgency::DueSoon)
        );
    }

    #[test]
    fn no_dates_means_not_urgent() {
        let today = d("2026-11-19");
        for status in Status::ALL {
            assert_eq!(classify(&todo(status), today, SOON), None, "{status:?}");
        }
    }

    // ---- the mock's sample data (today = 2026-11-19) ---------------------

    #[test]
    fn mock_sample_data_gives_one_of_each_category() {
        let today = d("2026-11-19");
        let mk = |status, due: &str, ps: &str, follow_up: Option<&str>| Todo {
            due_date: Some(d(due)),
            planned_start: Some(d(ps)),
            follow_up_date: follow_up.map(d),
            ..todo(status)
        };
        let cases = [
            // 移行手順書のドラフト
            (
                mk(Status::Doing, "2026-11-19", "2026-11-11", None),
                Some(Urgency::DueToday),
            ),
            // API設計書のレビュー対応
            (
                mk(
                    Status::Waiting,
                    "2026-11-20",
                    "2026-11-16",
                    Some("2026-11-20"),
                ),
                Some(Urgency::DueSoon),
            ),
            // 結合テスト計画の作成
            (mk(Status::Todo, "2026-11-30", "2026-11-20", None), None),
            // 性能試験の環境準備
            (
                mk(Status::Todo, "2026-12-04", "2026-11-17", None),
                Some(Urgency::LateStart),
            ),
            // 要件定義書の改訂
            (mk(Status::Done, "2026-11-13", "2026-11-09", None), None),
            // 旧帳票の移行検証
            (
                mk(Status::Cancelled, "2026-11-27", "2026-11-24", None),
                None,
            ),
            // ライセンス棚卸し
            (
                mk(Status::Todo, "2026-11-17", "2026-11-12", None),
                Some(Urgency::Overdue),
            ),
            // デプロイ手順書の更新
            (mk(Status::Done, "2026-11-24", "2026-11-18", None), None),
            // 監視ダッシュボードの整理
            (mk(Status::Hold, "2026-12-03", "2026-11-24", None), None),
            // 募集要項ページの原稿確認
            (
                mk(
                    Status::Waiting,
                    "2026-11-27",
                    "2026-11-16",
                    Some("2026-11-19"),
                ),
                Some(Urgency::FollowUp),
            ),
            // 応募フォームの項目見直し
            (mk(Status::Todo, "2026-12-02", "2026-11-25", None), None),
        ];
        for (i, (t, expected)) in cases.iter().enumerate() {
            assert_eq!(classify(t, today, SOON), *expected, "row {i}");
        }
    }

    // ---- warnings ----------------------------------------------------------

    #[test]
    fn plan_misses_due_compares_planned_end_with_due() {
        assert!(plan_misses_due(
            Some(d("2026-11-21")),
            Some(d("2026-11-20"))
        ));
        assert!(!plan_misses_due(
            Some(d("2026-11-20")),
            Some(d("2026-11-20"))
        ));
        assert!(!plan_misses_due(
            Some(d("2026-11-19")),
            Some(d("2026-11-20"))
        ));
        assert!(!plan_misses_due(None, Some(d("2026-11-20"))));
        assert!(!plan_misses_due(Some(d("2026-11-21")), None));
    }

    #[test]
    fn plan_overrun_for_doing_and_waiting_only() {
        let today = d("2026-11-19");
        let with_end = |status, end: &str| Todo {
            planned_end: Some(d(end)),
            ..todo(status)
        };
        assert!(warnings(&with_end(Status::Doing, "2026-11-18"), today).plan_overrun);
        assert!(warnings(&with_end(Status::Waiting, "2026-11-18"), today).plan_overrun);
        assert!(!warnings(&with_end(Status::Doing, "2026-11-19"), today).plan_overrun);
        assert!(!warnings(&with_end(Status::Todo, "2026-11-18"), today).plan_overrun);
        assert!(!warnings(&with_end(Status::Hold, "2026-11-18"), today).plan_overrun);
    }

    #[test]
    fn warnings_are_off_for_closed_todos() {
        let today = d("2026-11-19");
        for status in [Status::Done, Status::Cancelled] {
            let t = Todo {
                due_date: Some(d("2026-11-10")),
                planned_end: Some(d("2026-11-15")),
                ..todo(status)
            };
            assert_eq!(warnings(&t, today), Warnings::default());
        }
    }

    #[test]
    fn mock_migration_draft_has_plan_overrun() {
        // 移行手順書のドラフト: doing, planned 11/11–11/18, due 11/19.
        let t = Todo {
            due_date: Some(d("2026-11-19")),
            planned_start: Some(d("2026-11-11")),
            planned_end: Some(d("2026-11-18")),
            ..todo(Status::Doing)
        };
        assert_eq!(
            warnings(&t, d("2026-11-19")),
            Warnings {
                plan_misses_due: false,
                plan_overrun: true
            }
        );
    }

    // ---- ordering and day counts -------------------------------------------

    #[test]
    fn attention_order_sorts_by_category_then_due_then_priority() {
        let mk = |id, due: Option<&str>, priority| Todo {
            id,
            due_date: due.map(d),
            priority,
            ..todo(Status::Todo)
        };
        let a = mk(1, Some("2026-11-18"), Priority::Low);
        let b = mk(2, Some("2026-11-17"), Priority::Low);
        let c = mk(3, Some("2026-11-17"), Priority::High);
        let e = mk(4, None, Priority::High);
        let f = mk(5, Some("2026-11-19"), Priority::Low);
        let mut items = [
            (&f, Urgency::DueToday),
            (&e, Urgency::LateStart),
            (&a, Urgency::Overdue),
            (&b, Urgency::Overdue),
            (&c, Urgency::Overdue),
        ];
        items.sort_by(|x, y| attention_order(*x, *y));
        let ids: Vec<i64> = items.iter().map(|(t, _)| t.id).collect();
        assert_eq!(ids, vec![3, 2, 1, 5, 4]);

        // Missing due date sorts after present ones within a category.
        let g = mk(6, None, Priority::High);
        let h = mk(7, Some("2026-12-01"), Priority::Low);
        assert_eq!(
            attention_order((&g, Urgency::FollowUp), (&h, Urgency::FollowUp)),
            Ordering::Greater
        );
    }

    #[test]
    fn day_counts_match_the_mock_texts() {
        let today = d("2026-11-19");
        assert_eq!(days_overdue(d("2026-11-17"), today), 2); // 2日超過
        assert_eq!(days_left(d("2026-11-20"), today), 1); // あと1日
        assert_eq!(days_since_planned_start(d("2026-11-17"), today), 2); // 予定開始から2日経過
        assert_eq!(waiting_day(d("2026-11-17"), today), 3); // 待ち3日目
        assert_eq!(waiting_day(d("2026-11-19"), today), 1); // 待ち1日目
        assert_eq!(days_left(d("2027-01-01"), d("2026-12-31")), 1); // across year
    }

    #[test]
    fn assess_fills_day_counts_only_where_they_apply() {
        let today = d("2026-11-19");
        // 募集要項ページの原稿確認: waiting since 11/17, follow-up 11/19, due 11/27.
        let waiting = Todo {
            due_date: Some(d("2026-11-27")),
            planned_start: Some(d("2026-11-16")),
            waiting_since: Some(d("2026-11-17")),
            follow_up_date: Some(d("2026-11-19")),
            ..todo(Status::Waiting)
        };
        assert_eq!(
            assess(&waiting, today, SOON),
            Assessment {
                urgency: Some(Urgency::FollowUp),
                warnings: Warnings::default(),
                days_until_due: Some(8),
                days_since_planned_start: None,
                waiting_day: Some(3),
                days_since_follow_up: Some(0),
            }
        );

        // ライセンス棚卸し: overdue by 2 days; planned start passed too, but the
        // category is 超過, so no "since planned start" count.
        let overdue = Todo {
            due_date: Some(d("2026-11-17")),
            planned_start: Some(d("2026-11-12")),
            ..todo(Status::Todo)
        };
        let a = assess(&overdue, today, SOON);
        assert_eq!(a.urgency, Some(Urgency::Overdue));
        assert_eq!(a.days_until_due, Some(-2));
        assert_eq!(a.days_since_planned_start, None);

        // 性能試験の環境準備: late start by 2 days.
        let late = Todo {
            due_date: Some(d("2026-12-04")),
            planned_start: Some(d("2026-11-17")),
            ..todo(Status::Todo)
        };
        assert_eq!(assess(&late, today, SOON).days_since_planned_start, Some(2));

        // Closed TODOs carry no counts.
        let done = Todo {
            due_date: Some(d("2026-11-13")),
            ..todo(Status::Done)
        };
        assert_eq!(
            assess(&done, today, SOON),
            Assessment {
                urgency: None,
                warnings: Warnings::default(),
                days_until_due: None,
                days_since_planned_start: None,
                waiting_day: None,
                days_since_follow_up: None,
            }
        );
    }

    #[test]
    fn severity_order_matches_the_design() {
        let mut all = Urgency::ALL;
        all.reverse();
        all.sort();
        assert_eq!(all, Urgency::ALL);
        let labels: Vec<_> = Urgency::ALL.iter().map(|u| u.label()).collect();
        assert_eq!(labels, vec!["超過", "本日", "間近", "要確認", "着手遅れ"]);
    }
}
