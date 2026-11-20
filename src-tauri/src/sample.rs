//! Development sample data (debug builds only).
//!
//! The UI mock's 3 projects and 11 TODOs (docs/ui-mock/README.md), with every
//! date shifted so that the mock's "today" (2026-11-19) becomes the real today.
//!
//! v0.3.0 adds TODOs the mock does not have, chosen so that the mock's
//! screens (the dashboard's 日限注意, the daily report of the mock's day) stay
//! as they are: a parent with three children (S-1) in 社内ツール整備, none
//! 作業中 or 待ち, none starting before the day after tomorrow, none due
//! within the 「間近」 days, none closed on the mock's day. A TODO template
//! (T-10) 「月次締め」 with three children: templates show on no screen until
//! one is chosen in 「TODOを追加」.
//! Status changes are replayed through `domain::status::transition`, so the
//! actual dates, waiting fields and change history come out exactly as the app
//! itself would record them.

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sqlx::SqlitePool;

use crate::domain::date::{days_between, parse_date};
use crate::domain::events;
use crate::domain::model::{Priority, Status};
use crate::domain::status::{StatusChange, StatusFields, transition};
use crate::domain::template::{TemplateBody, TemplateChild};
use crate::repo::projects::NewProject;
use crate::repo::templates::TemplateFields;
use crate::repo::todos::NewTodo;
use crate::repo::work_logs::NewWorkLog;
use crate::repo::{self, RepoError};

/// The day the mock was drawn.
const MOCK_TODAY: &str = "2026-11-19";

/// Day every sample TODO was created (mock dates).
const CREATED_ON: &str = "2026-11-09";

const PROJECTS: [&str; 3] = ["基幹システム更改", "社内ツール整備", "採用サイト改修"];

enum Step {
    /// (date, status) — a plain status change.
    To(&'static str, Status),
    /// (date, waiting on, follow-up date)
    Wait(&'static str, &'static str, &'static str),
    /// (date, reason)
    Cancel(&'static str, &'static str),
}

struct TodoSpec {
    project: usize,
    /// Title of the parent (an earlier entry), for a child.
    parent: Option<&'static str>,
    title: &'static str,
    priority: Priority,
    due: &'static str,
    planned: (&'static str, &'static str),
    history: &'static [Step],
}

const TODOS: [TodoSpec; 15] = [
    TodoSpec {
        project: 0,
        parent: None,
        title: "移行手順書のドラフト",
        priority: Priority::Medium,
        due: "2026-11-19",
        planned: ("2026-11-11", "2026-11-18"),
        history: &[Step::To("2026-11-12", Status::Doing)],
    },
    TodoSpec {
        project: 0,
        parent: None,
        title: "API設計書のレビュー対応",
        priority: Priority::High,
        due: "2026-11-20",
        planned: ("2026-11-16", "2026-11-19"),
        history: &[
            Step::To("2026-11-17", Status::Doing),
            Step::Wait("2026-11-19", "認証担当の回答", "2026-11-20"),
        ],
    },
    TodoSpec {
        project: 0,
        parent: None,
        title: "結合テスト計画の作成",
        priority: Priority::Medium,
        due: "2026-11-30",
        planned: ("2026-11-20", "2026-11-27"),
        history: &[],
    },
    TodoSpec {
        project: 0,
        parent: None,
        title: "性能試験の環境準備",
        priority: Priority::Medium,
        due: "2026-12-04",
        planned: ("2026-11-17", "2026-11-20"),
        history: &[],
    },
    TodoSpec {
        project: 0,
        parent: None,
        title: "要件定義書の改訂",
        priority: Priority::Medium,
        due: "2026-11-13",
        planned: ("2026-11-09", "2026-11-13"),
        history: &[
            Step::To("2026-11-10", Status::Doing),
            Step::To("2026-11-13", Status::Done),
        ],
    },
    TodoSpec {
        project: 0,
        parent: None,
        title: "旧帳票の移行検証",
        priority: Priority::Medium,
        due: "2026-11-27",
        planned: ("2026-11-24", "2026-11-26"),
        history: &[Step::Cancel("2026-11-19", "旧帳票の廃止が決まったため")],
    },
    TodoSpec {
        project: 1,
        parent: None,
        title: "ライセンス棚卸し",
        priority: Priority::Medium,
        due: "2026-11-17",
        planned: ("2026-11-12", "2026-11-13"),
        history: &[],
    },
    TodoSpec {
        project: 1,
        parent: None,
        title: "デプロイ手順書の更新",
        priority: Priority::Medium,
        due: "2026-11-24",
        planned: ("2026-11-18", "2026-11-19"),
        history: &[
            Step::To("2026-11-18", Status::Doing),
            Step::To("2026-11-19", Status::Done),
        ],
    },
    TodoSpec {
        project: 1,
        parent: None,
        title: "監視ダッシュボードの整理",
        priority: Priority::Medium,
        due: "2026-12-03",
        planned: ("2026-11-24", "2026-11-27"),
        history: &[Step::To("2026-11-18", Status::Hold)],
    },
    TodoSpec {
        project: 2,
        parent: None,
        title: "募集要項ページの原稿確認",
        priority: Priority::Medium,
        due: "2026-11-27",
        planned: ("2026-11-16", "2026-11-20"),
        history: &[
            Step::To("2026-11-16", Status::Doing),
            Step::Wait("2026-11-17", "人事部の原稿確認", "2026-11-19"),
        ],
    },
    TodoSpec {
        project: 2,
        parent: None,
        title: "応募フォームの項目見直し",
        priority: Priority::Medium,
        due: "2026-12-02",
        planned: ("2026-11-25", "2026-11-27"),
        history: &[],
    },
    // サブTODO (S-1): 1 of 3 children done.
    TodoSpec {
        project: 1,
        parent: None,
        title: "開発環境の標準化",
        priority: Priority::Medium,
        due: "2026-12-11",
        planned: ("2026-11-24", "2026-12-10"),
        history: &[],
    },
    TodoSpec {
        project: 1,
        parent: Some("開発環境の標準化"),
        title: "必要なツールの洗い出し",
        priority: Priority::Medium,
        due: "2026-11-18",
        planned: ("2026-11-16", "2026-11-18"),
        history: &[
            Step::To("2026-11-16", Status::Doing),
            Step::To("2026-11-18", Status::Done),
        ],
    },
    TodoSpec {
        project: 1,
        parent: Some("開発環境の標準化"),
        title: "セットアップ手順書の作成",
        priority: Priority::Medium,
        due: "2026-12-04",
        planned: ("2026-11-24", "2026-12-02"),
        history: &[],
    },
    TodoSpec {
        project: 1,
        parent: Some("開発環境の標準化"),
        title: "新メンバーでの試行",
        priority: Priority::Medium,
        due: "2026-12-10",
        planned: ("2026-12-03", "2026-12-09"),
        history: &[],
    },
];

/// (TODO title, date, minutes, body)
const WORK_LOGS: [(&str, &str, i64, &str); 4] = [
    (
        "API設計書のレビュー対応",
        "2026-11-18",
        120,
        "レビュー指摘の一覧を整理し、対応方針を決めた。\n\n- 仕様の見直しが必要な指摘は3件\n- 文言の修正だけで済む指摘は9件",
    ),
    (
        "API設計書のレビュー対応",
        "2026-11-19",
        90,
        "- 指摘12件のうち9件を反映\n- 残り3件は認証まわり。認証担当に質問を送り、回答待ち\n- [ ] 指摘10:トークン更新の手順を追記",
    ),
    (
        "移行手順書のドラフト",
        "2026-11-19",
        120,
        "- 切り戻し手順の章を書き終え、レビューを依頼した",
    ),
    (
        "デプロイ手順書の更新",
        "2026-11-19",
        30,
        "- 新しい承認フローに合わせて手順4〜6を書き換えた",
    ),
];

/// The TODO template (T-10): 「月次締め」, its children due 3 and 2 days
/// before it and on the same day. No dependencies yet (T-11).
fn template() -> TemplateFields {
    let child = |title: &str, priority, days| TemplateChild {
        title: title.into(),
        priority,
        memo_md: String::new(),
        due_offset_days: Some(days),
        depends_on: Vec::new(),
    };
    TemplateFields {
        name: "月次締め".into(),
        body: TemplateBody {
            title: "月次締め".into(),
            priority: Priority::Medium,
            memo_md: "## 手順\n- 前月分の売上と経費を集計する\n- 報告書にまとめて共有する".into(),
            children: vec![
                child("売上の集計", Priority::High, -3),
                child("経費の確認", Priority::Medium, -2),
                child("報告書の作成", Priority::Medium, 0),
            ],
        },
    }
}

/// Shifts mock dates so that the mock's today lands on `today`.
struct Shift {
    days: i64,
}

impl Shift {
    fn new(today: NaiveDate) -> Shift {
        Shift {
            days: days_between(mock(MOCK_TODAY), today),
        }
    }

    fn date(&self, mock_date: &str) -> NaiveDate {
        mock(mock_date) + chrono::Duration::days(self.days)
    }

    /// 09:00 UTC on the shifted day, plus `minute` to keep events ordered.
    fn at(&self, mock_date: &str, minute: u32) -> DateTime<Utc> {
        self.date(mock_date)
            .and_time(NaiveTime::from_hms_opt(9, minute, 0).expect("valid time"))
            .and_utc()
    }
}

fn mock(s: &str) -> NaiveDate {
    parse_date(s).expect("sample dates are valid")
}

/// Inserts the sample data when the database has no project yet.
/// Returns whether anything was inserted.
pub async fn seed_if_empty(pool: &SqlitePool, today: NaiveDate) -> Result<bool, RepoError> {
    let mut tx = pool.begin().await?;
    if !repo::projects::list(&mut tx, true).await?.is_empty() {
        return Ok(false);
    }
    let shift = Shift::new(today);

    let mut project_ids = Vec::new();
    for name in PROJECTS {
        let p = repo::projects::insert(
            &mut tx,
            &NewProject {
                name: name.into(),
                color: None,
                description: String::new(),
            },
            shift.at(CREATED_ON, 0),
        )
        .await?;
        project_ids.push(p.id);
    }

    let mut todo_ids = Vec::new();
    for (i, spec) in TODOS.iter().enumerate() {
        let minute = u32::try_from(i).unwrap_or(0);
        let created_at = shift.at(CREATED_ON, minute);
        let parent_id = spec.parent.map(|title| {
            todo_ids
                .iter()
                .find(|(t, _)| *t == title)
                .map(|(_, id)| *id)
                .expect("a parent comes before its children")
        });
        let mut todo = repo::todos::insert(
            &mut tx,
            &NewTodo {
                project_id: project_ids[spec.project],
                parent_id,
                title: spec.title.into(),
                priority: spec.priority,
                due_date: Some(shift.date(spec.due)),
                planned_start: Some(shift.date(spec.planned.0)),
                planned_end: Some(shift.date(spec.planned.1)),
                memo_md: String::new(),
            },
            created_at,
        )
        .await?;
        let created = events::created(Status::Todo, shift.date(CREATED_ON), created_at);
        repo::events::insert(&mut tx, todo.id, &created).await?;

        for step in spec.history {
            let (date, change) = match *step {
                Step::To(date, status) => (date, StatusChange::to(status)),
                Step::Wait(date, on, follow_up) => {
                    (date, StatusChange::waiting(on, shift.date(follow_up)))
                }
                Step::Cancel(date, reason) => (date, StatusChange::cancel(reason)),
            };
            let at = shift.at(date, minute);
            let t = transition(&StatusFields::of(&todo), &change, shift.date(date), at)
                .expect("sample history follows the transition table");
            todo = repo::todos::write_status(&mut tx, todo.id, &t.fields, at).await?;
            repo::events::insert(&mut tx, todo.id, &t.event).await?;
        }
        todo_ids.push((spec.title, todo.id));
    }

    for (title, date, minutes, body) in WORK_LOGS {
        let todo_id = todo_ids
            .iter()
            .find(|(t, _)| *t == title)
            .map(|(_, id)| *id)
            .expect("work log refers to a sample TODO");
        repo::work_logs::insert(
            &mut tx,
            &NewWorkLog {
                todo_id,
                log_date: shift.date(date),
                body_md: body.into(),
                minutes: Some(minutes),
            },
            shift.at(date, 30),
        )
        .await?;
    }

    repo::templates::insert(&mut tx, &template(), shift.at(CREATED_ON, 50)).await?;

    tx.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::EventKind;
    use crate::domain::model::Todo;
    use crate::domain::testing::{d, rules};
    use crate::domain::urgency::{Urgency, classify};
    use crate::domain::views::project_summaries;
    use crate::repo::db;
    use crate::repo::todos::TodoFilter;

    async fn urgency_counts(pool: &SqlitePool, today: NaiveDate) -> Vec<(Urgency, usize)> {
        let mut c = pool.acquire().await.unwrap();
        let todos = repo::todos::list(&mut c, &TodoFilter::default())
            .await
            .unwrap();
        Urgency::ALL
            .iter()
            .map(|u| {
                let n = todos
                    .iter()
                    .filter(|t| classify(t, today, &rules(3)) == Some(*u))
                    .count();
                (*u, n)
            })
            .collect()
    }

    #[tokio::test]
    async fn sample_matches_the_mock_on_its_own_day() {
        let pool = db::open_in_memory().await.unwrap();
        let today = d("2026-11-19");
        assert!(seed_if_empty(&pool, today).await.unwrap());

        // 日限注意 5件: 超過1、本日1、間近1、要確認1、着手遅れ1.
        assert_eq!(
            urgency_counts(&pool, today).await,
            Urgency::ALL.iter().map(|u| (*u, 1)).collect::<Vec<_>>()
        );

        // Per project: attention 3 / 1 / 1, progress 1/5, 2/7 (with the
        // parent and its children), 0/2.
        let mut c = pool.acquire().await.unwrap();
        let projects = repo::projects::list(&mut c, false).await.unwrap();
        let todos = repo::todos::list(&mut c, &TodoFilter::default())
            .await
            .unwrap();
        let s: Vec<_> = project_summaries(projects, &todos, today, &rules(3))
            .into_iter()
            .map(|s| (s.attention_count, s.progress_done, s.progress_total))
            .collect();
        assert_eq!(s, vec![(3, 1, 5), (1, 2, 7), (1, 0, 2)]);

        // The parent has 3 children; their dates sit inside its due date.
        let parent = todos
            .iter()
            .find(|t| t.title == "開発環境の標準化")
            .unwrap();
        let children: Vec<&Todo> = todos
            .iter()
            .filter(|t| t.parent_id == Some(parent.id))
            .collect();
        assert_eq!(children.len(), 3);
        assert!(children.iter().all(|c| c.due_date <= parent.due_date));

        // The API review TODO went 未着手 → 作業中 (11/17) → 待ち (11/19).
        let api = todos
            .iter()
            .find(|t| t.title == "API設計書のレビュー対応")
            .unwrap();
        assert_eq!(api.status, Status::Waiting);
        assert_eq!(api.actual_start, Some(d("2026-11-17")));
        assert_eq!(api.waiting_since, Some(d("2026-11-19")));
        let evs = repo::events::list_by_todo(&mut c, api.id).await.unwrap();
        let kinds: Vec<_> = evs
            .iter()
            .map(|e| (e.kind, e.new_value.as_deref()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (EventKind::Created, Some("todo")),
                (EventKind::Status, Some("doing")),
                (EventKind::Status, Some("waiting")),
            ]
        );
        assert_eq!(
            repo::work_logs::list_by_todo(&mut c, api.id)
                .await
                .unwrap()
                .len(),
            2
        );

        // One template with three children, due before and on its day.
        let templates = repo::templates::list(&mut c).await.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "月次締め");
        let days: Vec<Option<i64>> = templates[0]
            .body
            .children
            .iter()
            .map(|c| c.due_offset_days)
            .collect();
        assert_eq!(days, [Some(-3), Some(-2), Some(0)]);
    }

    #[tokio::test]
    async fn sample_keeps_the_categories_on_any_day() {
        let pool = db::open_in_memory().await.unwrap();
        let today = d("2027-03-01"); // crosses a month and a year from the mock
        seed_if_empty(&pool, today).await.unwrap();
        assert_eq!(
            urgency_counts(&pool, today).await,
            Urgency::ALL.iter().map(|u| (*u, 1)).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn seeding_twice_inserts_nothing_more() {
        let pool = db::open_in_memory().await.unwrap();
        let today = d("2026-11-19");
        assert!(seed_if_empty(&pool, today).await.unwrap());
        assert!(!seed_if_empty(&pool, today).await.unwrap());
        let mut c = pool.acquire().await.unwrap();
        assert_eq!(repo::projects::list(&mut c, true).await.unwrap().len(), 3);
    }
}
