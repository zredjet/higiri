//! 一括変更 (T-14, v0.4.0): preview_bulk_change, apply_bulk_change.
//!
//! The TODOs selected in the list get one change (`domain::bulk`). The
//! confirmation lists what changes, what is left out and why, and for a
//! status what else happens (TODOs waiting for unfinished ones going 作業中,
//! children closed with their parent, next rounds). The save works it out
//! again on the data as it is then and changes every target in one
//! transaction, each as `update_todo` (dates, 優先度) or `change_status`
//! (status, parents first) does it. An unexpected failure refuses the whole
//! change.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};
use tauri::State;

use super::clock;
use super::error::{AppError, CmdResult};
use super::todos::{change_one, save_fields};
use crate::domain::bulk::{BulkChange, BulkContext, BulkPlan, plan};
use crate::domain::dependency::{BlockingTodo, Dependencies};
use crate::domain::model::{Status, Todo};
use crate::domain::repeat::RepeatsByTodo;
use crate::domain::urgency::Rules;
use crate::repo;
use crate::repo::db;

/// A selected TODO named in the confirmation (left out, or waiting).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BulkTodo {
    pub id: i64,
    pub title: String,
    /// For a child: shown 「親 / 子」.
    pub parent_title: Option<String>,
    pub project_id: i64,
    pub project_name: String,
}

/// A selected TODO left out, with why (「すでにその日限です。」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BulkSkipped {
    pub todo: BulkTodo,
    pub reason: String,
}

/// A TODO going 作業中 that waits for unfinished TODOs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BulkBlocked {
    pub todo: BulkTodo,
    pub blocked_by: Vec<BlockingTodo>,
}

/// What a bulk change would do (the confirmation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BulkPreview {
    /// The TODOs to change.
    pub targets: Vec<i64>,
    /// The TODOs left out, in the order selected.
    pub skipped: Vec<BulkSkipped>,
    /// To 作業中: those that wait for unfinished TODOs.
    pub blocked: Vec<BulkBlocked>,
    /// To 完了・中止: the open children closed with their parent.
    pub closing_children: i64,
    /// To 完了・中止: the repeating TODOs that make their next round.
    pub next_rounds: i64,
}

/// What a bulk change did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BulkResult {
    /// The TODOs changed (not the children closed with them).
    pub changed: Vec<i64>,
    /// The TODOs left out, worked out at the save.
    pub skipped: Vec<BulkSkipped>,
}

/// What `change` would do to the TODOs `ids` (the list's selection, in its
/// order). Nothing is written. A wrong input (0 days, no reason for 中止) is
/// an error.
#[tauri::command]
#[specta::specta]
pub async fn preview_bulk_change(
    pool: State<'_, SqlitePool>,
    ids: Vec<i64>,
    change: BulkChange,
) -> CmdResult<BulkPreview> {
    preview(&pool, &ids, change, clock::today()).await
}

/// Gives `change` to the TODOs `ids`, worked out again now, in one
/// transaction. What cannot take it is left out (`skipped`).
#[tauri::command]
#[specta::specta]
pub async fn apply_bulk_change(
    pool: State<'_, SqlitePool>,
    ids: Vec<i64>,
    change: BulkChange,
) -> CmdResult<BulkResult> {
    apply(&pool, &ids, change, clock::today(), clock::now()).await
}

/// The checked change, the selected TODOs (each once, in the order given;
/// ids no longer there are dropped) and what the change does to them.
struct Planned {
    change: BulkChange,
    selected: Vec<Todo>,
    plan: BulkPlan,
}

async fn planned(
    conn: &mut SqliteConnection,
    ids: &[i64],
    change: BulkChange,
    today: NaiveDate,
) -> CmdResult<Planned> {
    let change = change
        .checked()
        .map_err(|e| AppError::validation(e.to_string()))?;
    let selected = repo::todos::list_by_ids(conn, ids).await?;
    let ids: Vec<i64> = selected.iter().map(|t| t.id).collect();
    let to = match &change {
        BulkChange::Status { status, .. } => Some(*status),
        _ => None,
    };
    let closing = to.is_some_and(Status::is_closed);
    let children = if closing {
        repo::todos::list_children(conn, &ids).await?
    } else {
        Vec::new()
    };
    let repeats: RepeatsByTodo = if closing {
        let wanted: std::collections::HashSet<i64> = ids.iter().copied().collect();
        repo::repeats::list(conn)
            .await?
            .into_iter()
            .filter(|r| wanted.contains(&r.todo_id))
            .map(|r| (r.todo_id, r))
            .collect()
    } else {
        RepeatsByTodo::new()
    };
    let dependencies = if to == Some(Status::Doing) {
        Dependencies::new(repo::dependencies::prerequisites(conn, Some(&ids)).await?)
    } else {
        Dependencies::default()
    };
    let rules = Rules::from_settings(&repo::settings::load(conn).await?);
    let plan = plan(
        &change,
        &selected,
        BulkContext {
            children: &children,
            repeats: &repeats,
            dependencies: &dependencies,
        },
        today,
        &rules,
    );
    Ok(Planned {
        change,
        selected,
        plan,
    })
}

/// Names the TODOs `ids` of `selected` as the confirmation shows them.
async fn named(
    conn: &mut SqliteConnection,
    selected: &[Todo],
    ids: impl Iterator<Item = i64>,
) -> CmdResult<HashMap<i64, BulkTodo>> {
    let by_id: HashMap<i64, &Todo> = selected.iter().map(|t| (t.id, t)).collect();
    let wanted: Vec<&Todo> = ids.filter_map(|id| by_id.get(&id).copied()).collect();
    if wanted.is_empty() {
        return Ok(HashMap::new());
    }
    let projects: HashMap<i64, String> = repo::projects::list(conn, true)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let parent_ids: Vec<i64> = wanted.iter().filter_map(|t| t.parent_id).collect();
    let parents: HashMap<i64, String> = repo::todos::list_by_ids(conn, &parent_ids)
        .await?
        .into_iter()
        .map(|t| (t.id, t.title))
        .collect();
    Ok(wanted
        .into_iter()
        .map(|t| {
            (
                t.id,
                BulkTodo {
                    id: t.id,
                    title: t.title.clone(),
                    parent_title: t.parent_id.and_then(|p| parents.get(&p).cloned()),
                    project_id: t.project_id,
                    project_name: projects.get(&t.project_id).cloned().unwrap_or_default(),
                },
            )
        })
        .collect())
}

async fn skipped(conn: &mut SqliteConnection, p: &Planned) -> CmdResult<Vec<BulkSkipped>> {
    let mut names = named(conn, &p.selected, p.plan.skipped.iter().map(|(id, _)| *id)).await?;
    p.plan
        .skipped
        .iter()
        .map(|(id, reason)| {
            Ok(BulkSkipped {
                todo: names.remove(id).ok_or_else(AppError::internal)?,
                reason: reason.to_string(),
            })
        })
        .collect()
}

pub async fn preview(
    pool: &SqlitePool,
    ids: &[i64],
    change: BulkChange,
    today: NaiveDate,
) -> CmdResult<BulkPreview> {
    let mut conn = pool.acquire().await?;
    let p = planned(&mut conn, ids, change, today).await?;
    let skipped = skipped(&mut conn, &p).await?;
    let mut names = named(
        &mut conn,
        &p.selected,
        p.plan.blocked.iter().map(|(id, _)| *id),
    )
    .await?;
    let blocked = p
        .plan
        .blocked
        .iter()
        .map(|(id, blocked_by)| {
            Ok(BulkBlocked {
                todo: names.remove(id).ok_or_else(AppError::internal)?,
                blocked_by: blocked_by.clone(),
            })
        })
        .collect::<CmdResult<Vec<_>>>()?;
    Ok(BulkPreview {
        targets: p.plan.targets.clone(),
        skipped,
        blocked,
        closing_children: p.plan.closing_children.len() as i64,
        next_rounds: p.plan.next_rounds as i64,
    })
}

pub async fn apply(
    pool: &SqlitePool,
    ids: &[i64],
    change: BulkChange,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<BulkResult> {
    let mut tx = db::begin_write(pool).await?;
    let p = planned(&mut tx, ids, change, today).await?;
    let by_id: HashMap<i64, &Todo> = p.selected.iter().map(|t| (t.id, t)).collect();
    if let Some(status) = p.change.status_change() {
        // Parents first; a parent's open children close with it.
        for id in &p.plan.targets {
            let before = by_id.get(id).ok_or_else(AppError::internal)?;
            change_one(&mut tx, before, &status, today, now)
                .await
                .map_err(|e| {
                    failed(
                        e,
                        &format!("「{}」を{}にできません。", before.title, status.to.label()),
                    )
                })?;
        }
    } else {
        let what = match p.change {
            BulkChange::SetDue { .. } | BulkChange::ShiftDue { .. } => "日限",
            BulkChange::ShiftPlanned { .. } => "予定",
            _ => "優先度",
        };
        for after in &p.plan.edits {
            let before = by_id.get(&after.id).ok_or_else(AppError::internal)?;
            save_fields(&mut tx, before, after, today, now)
                .await
                .map_err(|e| {
                    failed(
                        e,
                        &format!("「{}」の{what}を変えられません。", before.title),
                    )
                })?;
        }
    }
    let skipped = skipped(&mut tx, &p).await?;
    tx.commit().await?;
    Ok(BulkResult {
        changed: p.plan.targets,
        skipped,
    })
}

/// An unexpected failure, naming the TODO it stopped at. Nothing is saved.
fn failed(e: AppError, which: &str) -> AppError {
    log::error!("bulk change stopped: {:?}", e.kind);
    AppError {
        kind: e.kind,
        message: format!("{which}{}", e.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::dependencies;
    use crate::commands::error::ErrorKind;
    use crate::commands::projects::{self, ProjectInput};
    use crate::commands::repeats::{self, RepeatInput};
    use crate::commands::todos::{self, NewTodoInput};
    use crate::domain::events::EventKind;
    use crate::domain::model::Priority;
    use crate::domain::repeat::RepeatRule;
    use crate::domain::status::StatusChange;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    fn today() -> NaiveDate {
        d("2026-11-19")
    }

    fn now() -> DateTime<Utc> {
        ts(2026, 11, 19, 1, 0)
    }

    fn created() -> DateTime<Utc> {
        ts(2026, 11, 1, 0, 0)
    }

    async fn project(pool: &SqlitePool, name: &str) -> i64 {
        projects::create(
            pool,
            ProjectInput {
                name: name.into(),
                color: None,
                description: String::new(),
            },
            created(),
        )
        .await
        .unwrap()
        .id
    }

    /// A TODO due `due` (`""` = none), planned `start`〜`end` (`""` = none).
    async fn add(
        pool: &SqlitePool,
        project_id: i64,
        parent: Option<i64>,
        title: &str,
        due: &str,
        start: &str,
        end: &str,
    ) -> Todo {
        let date = |s: &str| (!s.is_empty()).then(|| d(s));
        todos::create(
            pool,
            NewTodoInput {
                project_id,
                parent_id: parent,
                template_id: None,
                title: title.into(),
                priority: None,
                due_date: date(due),
                planned_start: date(start),
                planned_end: date(end),
                memo_md: None,
            },
            today(),
            created(),
        )
        .await
        .unwrap()
        .todo
    }

    async fn set_status(pool: &SqlitePool, t: &Todo, change: StatusChange) {
        todos::change_status_impl(pool, t.id, &change, today(), created())
            .await
            .unwrap();
    }

    async fn get(pool: &SqlitePool, id: i64) -> Todo {
        todos::get(pool, id, today()).await.unwrap().todo
    }

    /// The TODO's history after it was made: (kind, old, new, reason).
    async fn events(
        pool: &SqlitePool,
        id: i64,
    ) -> Vec<(EventKind, String, String, Option<String>)> {
        let mut c = pool.acquire().await.unwrap();
        repo::events::list_by_todo(&mut c, id)
            .await
            .unwrap()
            .into_iter()
            .filter(|e| e.kind != EventKind::Created)
            .map(|e| {
                (
                    e.kind,
                    e.old_value.unwrap_or_default(),
                    e.new_value.unwrap_or_default(),
                    e.note.reason,
                )
            })
            .collect()
    }

    fn fields_of(t: &Todo) -> todos::TodoFields {
        todos::TodoFields {
            title: t.title.clone(),
            priority: t.priority,
            due_date: t.due_date,
            planned_start: t.planned_start,
            planned_end: t.planned_end,
            actual_start: t.actual_start,
            actual_end: t.actual_end,
            waiting_on: t.waiting_on.clone(),
            follow_up_date: t.follow_up_date,
            close_reason: t.close_reason.clone(),
            memo_md: t.memo_md.clone(),
        }
    }

    fn status(to: Status) -> BulkChange {
        BulkChange::Status {
            status: to,
            reason: None,
        }
    }

    fn skipped_lines(skipped: &[BulkSkipped]) -> Vec<(i64, &str)> {
        skipped
            .iter()
            .map(|s| (s.todo.id, s.reason.as_str()))
            .collect()
    }

    async fn todo_count(pool: &SqlitePool) -> i64 {
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM todos")
            .fetch_one(pool)
            .await
            .unwrap();
        n
    }

    #[tokio::test]
    async fn preview_names_what_is_left_out_and_writes_nothing() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "基幹システム更改").await;
        let q = project(&pool, "社内ツール整備").await;
        let a = add(&pool, p, None, "API設計", "2026-11-20", "", "").await;
        let parent = add(&pool, q, None, "開発環境の標準化", "", "", "").await;
        let b = add(&pool, q, Some(parent.id), "手順書", "", "", "").await;
        let c = add(&pool, p, None, "移行手順書", "2026-11-25", "", "").await;
        set_status(&pool, &c, StatusChange::to(Status::Done)).await;

        let got = preview(
            &pool,
            &[a.id, b.id, c.id, 999],
            BulkChange::ShiftDue { days: 2 },
            today(),
        )
        .await
        .unwrap();
        assert_eq!(
            got,
            BulkPreview {
                targets: vec![a.id],
                skipped: vec![
                    BulkSkipped {
                        todo: BulkTodo {
                            id: b.id,
                            title: "手順書".into(),
                            parent_title: Some("開発環境の標準化".into()),
                            project_id: q,
                            project_name: "社内ツール整備".into(),
                        },
                        reason: "日限がありません。".into(),
                    },
                    BulkSkipped {
                        todo: BulkTodo {
                            id: c.id,
                            title: "移行手順書".into(),
                            parent_title: None,
                            project_id: p,
                            project_name: "基幹システム更改".into(),
                        },
                        reason: "完了・中止したTODOの日付は変えません。".into(),
                    },
                ],
                blocked: vec![],
                closing_children: 0,
                next_rounds: 0,
            }
        );
        assert_eq!(get(&pool, a.id).await.due_date, Some(d("2026-11-20")));
    }

    #[tokio::test]
    async fn a_wrong_input_is_refused() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add(&pool, p, None, "A", "2026-11-20", "", "").await;
        for (change, message) in [
            (
                BulkChange::ShiftDue { days: 0 },
                "ずらす日数を、0以外の整数で入力してください。",
            ),
            (
                BulkChange::ShiftPlanned { days: 400 },
                "ずらす日数は-365〜365の範囲で入力してください。",
            ),
            (
                BulkChange::SetDue { date: None },
                "日限を入力してください。",
            ),
            (status(Status::Cancelled), "中止の理由を入力してください。"),
            (
                status(Status::Waiting),
                "「待ち」には一括で変えられません。待っている相手と確認日を、1件ずつ詳細パネルで入力してください。",
            ),
        ] {
            let e = preview(&pool, &[a.id], change.clone(), today())
                .await
                .unwrap_err();
            assert_eq!(
                (e.kind, e.message.as_str()),
                (ErrorKind::Validation, message)
            );
            let e = apply(&pool, &[a.id], change, today(), now())
                .await
                .unwrap_err();
            assert_eq!(e.message, message);
        }
        assert_eq!(get(&pool, a.id).await.updated_at, created());
    }

    #[tokio::test]
    async fn dates_are_saved_as_update_todo_saves_them() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add(
            &pool,
            p,
            None,
            "A",
            "2026-11-20",
            "2026-11-16",
            "2026-11-19",
        )
        .await;
        let b = add(&pool, p, None, "B", "", "2026-11-17", "").await;
        let c = add(&pool, p, None, "C", "2026-11-30", "", "").await;

        let got = apply(
            &pool,
            &[a.id, b.id, c.id],
            BulkChange::ShiftDue { days: -3 },
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(got.changed, vec![a.id, c.id]);
        assert_eq!(
            skipped_lines(&got.skipped),
            vec![(b.id, "日限がありません。")]
        );
        assert_eq!(get(&pool, a.id).await.due_date, Some(d("2026-11-17")));
        assert_eq!(get(&pool, c.id).await.due_date, Some(d("2026-11-27")));
        assert_eq!(get(&pool, a.id).await.updated_at, now());
        assert_eq!(get(&pool, b.id).await.updated_at, created());
        assert_eq!(
            events(&pool, a.id).await,
            vec![(
                EventKind::DueDate,
                "2026-11-20".into(),
                "2026-11-17".into(),
                None
            )]
        );

        apply(
            &pool,
            &[a.id, b.id, c.id],
            BulkChange::ShiftPlanned { days: 2 },
            today(),
            now(),
        )
        .await
        .unwrap();
        let a2 = get(&pool, a.id).await;
        assert_eq!(
            (a2.planned_start, a2.planned_end),
            (Some(d("2026-11-18")), Some(d("2026-11-21")))
        );
        let b2 = get(&pool, b.id).await;
        assert_eq!(
            (b2.planned_start, b2.planned_end),
            (Some(d("2026-11-19")), None)
        );
        // One row per TODO; the 日限 stays.
        assert_eq!(
            events(&pool, b.id).await,
            vec![(
                EventKind::Planned,
                r#"{"start":"2026-11-17","end":null}"#.into(),
                r#"{"start":"2026-11-19","end":null}"#.into(),
                None
            )]
        );
        assert_eq!(a2.due_date, Some(d("2026-11-17")));

        apply(
            &pool,
            &[a.id, b.id],
            BulkChange::SetDue {
                date: Some(d("2026-12-01")),
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(get(&pool, b.id).await.due_date, Some(d("2026-12-01")));

        // The 優先度 is not in the history.
        let got = apply(
            &pool,
            &[a.id, b.id],
            BulkChange::Priority {
                priority: Priority::High,
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(got.changed.len(), 2);
        assert_eq!(get(&pool, a.id).await.priority, Priority::High);
        assert_eq!(events(&pool, a.id).await.len(), 3);
    }

    #[tokio::test]
    async fn closing_a_parent_with_its_child_selected_closes_the_child_once() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let parent = add(&pool, p, None, "親", "", "", "").await;
        let c1 = add(&pool, p, Some(parent.id), "子1", "", "", "").await;
        let c2 = add(&pool, p, Some(parent.id), "子2", "", "", "").await;
        let change = BulkChange::Status {
            status: Status::Cancelled,
            reason: Some(" 不要になったため ".into()),
        };

        let got = preview(&pool, &[c1.id, parent.id], change.clone(), today())
            .await
            .unwrap();
        assert_eq!(got.targets, vec![parent.id]);
        assert_eq!(got.closing_children, 2);
        assert!(got.skipped.is_empty());

        let got = apply(&pool, &[c1.id, parent.id], change, today(), now())
            .await
            .unwrap();
        assert_eq!(got.changed, vec![parent.id]);
        for id in [parent.id, c1.id, c2.id] {
            let t = get(&pool, id).await;
            assert_eq!(t.status, Status::Cancelled);
            assert_eq!(t.close_reason.as_deref(), Some("不要になったため"));
            assert_eq!(
                events(&pool, id).await,
                vec![(
                    EventKind::Status,
                    "todo".into(),
                    "cancelled".into(),
                    Some("不要になったため".into())
                )]
            );
        }
    }

    #[tokio::test]
    async fn closing_a_parent_alone_closes_its_children_and_makes_the_next_round() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let parent = add(&pool, p, None, "月次締め", "2026-11-19", "", "").await;
        let child = add(&pool, p, Some(parent.id), "集計", "", "", "").await;
        repeats::set(
            &pool,
            parent.id,
            Some(RepeatInput {
                rule: RepeatRule::Weekly {
                    weekdays: vec![crate::domain::repeat::DayOfWeek::Thu],
                },
                until_date: None,
            }),
            today(),
            created(),
        )
        .await
        .unwrap();
        let ended = add(&pool, p, None, "最後の回", "2026-11-19", "", "").await;
        repeats::set(
            &pool,
            ended.id,
            Some(RepeatInput {
                rule: RepeatRule::Daily,
                until_date: Some(d("2026-11-19")),
            }),
            today(),
            created(),
        )
        .await
        .unwrap();

        let ids = [parent.id, ended.id];
        let got = preview(&pool, &ids, status(Status::Done), today())
            .await
            .unwrap();
        assert_eq!((got.closing_children, got.next_rounds), (1, 1));

        let before = todo_count(&pool).await;
        apply(&pool, &ids, status(Status::Done), today(), now())
            .await
            .unwrap();
        assert_eq!(get(&pool, child.id).await.status, Status::Done);
        // The next round of 月次締め and its child; none of 最後の回.
        assert_eq!(todo_count(&pool).await, before + 2);
        let (next_due,): (String,) = sqlx::query_as(
            "SELECT due_date FROM todos WHERE title = '月次締め' AND status = 'todo'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(next_due, "2026-11-26");
    }

    #[tokio::test]
    async fn a_parent_with_a_child_that_cannot_close_is_left_out() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let parent = add(&pool, p, None, "親", "", "", "").await;
        let stuck = add(&pool, p, Some(parent.id), "子", "", "", "").await;
        let other = add(&pool, p, None, "ほか", "", "", "").await;
        // Started after today (entered by hand).
        todos::update(
            &pool,
            stuck.id,
            todos::TodoFields {
                actual_start: Some(d("2026-11-25")),
                ..fields_of(&stuck)
            },
            today(),
            created(),
        )
        .await
        .unwrap();

        let got = apply(
            &pool,
            &[parent.id, other.id],
            status(Status::Done),
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(got.changed, vec![other.id]);
        assert_eq!(
            skipped_lines(&got.skipped),
            vec![(
                parent.id,
                "子TODO「子」を完了にできません。実績開始日(11月25日)が実績終了日(11月19日)より後になってしまいます。実績期間を直してから、もう一度ステータスを変えてください。"
            )]
        );
        assert_eq!(get(&pool, parent.id).await.status, Status::Todo);
        assert_eq!(get(&pool, stuck.id).await.status, Status::Todo);
        assert_eq!(get(&pool, other.id).await.status, Status::Done);
    }

    #[tokio::test]
    async fn going_doing_lists_the_todos_waiting_for_unfinished_ones() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "基幹システム更改").await;
        let first = add(&pool, p, None, "要件の確定", "", "", "").await;
        let later = add(&pool, p, None, "API設計", "", "", "").await;
        dependencies::add(&pool, later.id, first.id, today(), created())
            .await
            .unwrap();

        let got = preview(&pool, &[later.id], status(Status::Doing), today())
            .await
            .unwrap();
        assert_eq!(got.blocked.len(), 1);
        assert_eq!(got.blocked[0].todo.title, "API設計");
        assert_eq!(
            got.blocked[0].blocked_by,
            vec![BlockingTodo {
                id: first.id,
                title: "要件の確定".into(),
                parent_title: None,
                project_name: "基幹システム更改".into(),
            }]
        );
        // Not stopped.
        apply(&pool, &[later.id], status(Status::Doing), today(), now())
            .await
            .unwrap();
        assert_eq!(get(&pool, later.id).await.status, Status::Doing);
    }

    #[tokio::test]
    async fn the_save_works_it_out_again_on_the_data_as_it_is_then() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add(&pool, p, None, "A", "", "", "").await;
        let b = add(&pool, p, None, "B", "", "", "").await;
        let got = preview(&pool, &[a.id, b.id], status(Status::Done), today())
            .await
            .unwrap();
        assert_eq!(got.targets, vec![a.id, b.id]);
        // Closed elsewhere after the confirmation.
        set_status(&pool, &b, StatusChange::to(Status::Done)).await;

        let got = apply(&pool, &[a.id, b.id], status(Status::Done), today(), now())
            .await
            .unwrap();
        assert_eq!(got.changed, vec![a.id]);
        assert_eq!(
            skipped_lines(&got.skipped),
            vec![(b.id, "すでに「完了」です。")]
        );
        assert_eq!(events(&pool, b.id).await.len(), 1);
    }

    #[tokio::test]
    async fn an_unexpected_failure_changes_nothing() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add(&pool, p, None, "A", "2026-11-20", "", "").await;
        let b = add(&pool, p, None, "B", "2026-11-21", "", "").await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE TRIGGER stop_b BEFORE UPDATE ON todos WHEN NEW.id = {}
             BEGIN SELECT RAISE(ABORT, 'stopped'); END",
            b.id
        )))
        .execute(&pool)
        .await
        .unwrap();

        let e = apply(&pool, &[a.id, b.id], status(Status::Done), today(), now())
            .await
            .unwrap_err();
        assert!(
            e.message.starts_with("「B」を完了にできません。"),
            "{}",
            e.message
        );
        assert_eq!(get(&pool, a.id).await.status, Status::Todo);
        assert!(events(&pool, a.id).await.is_empty());

        let e = apply(
            &pool,
            &[a.id, b.id],
            BulkChange::ShiftDue { days: 1 },
            today(),
            now(),
        )
        .await
        .unwrap_err();
        assert!(
            e.message.starts_with("「B」の日限を変えられません。"),
            "{}",
            e.message
        );
        assert_eq!(get(&pool, a.id).await.due_date, Some(d("2026-11-20")));
        assert!(events(&pool, a.id).await.is_empty());
    }
}
