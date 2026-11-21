//! 後続TODOのずらし (G-8, v0.4.0): preview_dependent_shift,
//! shift_with_dependents.
//!
//! A Gantt drag that moves a plan's end later asks first when TODOs waiting
//! for it (後続TODO) would then start before it ends. Moving them too moves
//! each by the days it lacks (`domain::shift`). The confirmation lists them;
//! the save works them out again on the data as it is then, and writes the
//! dragged TODO's 予定期間 and theirs in one transaction, each recorded as
//! `update_todo` records it (one `planned` row per TODO). The 日限 never
//! moves.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};
use tauri::State;

use super::clock;
use super::error::{AppError, CmdResult};
use super::todos::save_planned;
use super::validate;
use crate::domain::model::Todo;
use crate::domain::shift::{Shift, dependent_shifts, reachable};
use crate::repo;

/// A 後続TODO to move with the dragged one (the confirmation's list).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct DependentShift {
    pub id: i64,
    pub title: String,
    /// For a child: shown 「親 / 子」.
    pub parent_title: Option<String>,
    /// The project's name is shown when it is not the dragged TODO's.
    pub project_id: i64,
    pub project_name: String,
    /// The 予定期間 now (a 予定終了 may be missing: only the start moves).
    pub planned_start: NaiveDate,
    pub planned_end: Option<NaiveDate>,
    /// The 予定期間 once moved.
    pub shifted_start: NaiveDate,
    pub shifted_end: Option<NaiveDate>,
    /// Calendar days later (1 or more).
    pub days: i64,
}

/// The 後続TODO (and theirs) that would start before their 先行TODO ends if
/// `todo_id` were planned `planned_start`〜`planned_end`, each with its new
/// 予定期間, in the dependencies' order. Empty: nothing to ask (save it with
/// `update_todo`). Nothing is written.
#[tauri::command]
#[specta::specta]
pub async fn preview_dependent_shift(
    pool: State<'_, SqlitePool>,
    todo_id: i64,
    planned_start: NaiveDate,
    planned_end: NaiveDate,
) -> CmdResult<Vec<DependentShift>> {
    preview(&pool, todo_id, planned_start, planned_end).await
}

/// Plans `todo_id` `planned_start`〜`planned_end` and moves its 後続TODO by
/// the days they lack, worked out again now, in one transaction. Returns the
/// ids of the TODOs moved, the dragged one first.
#[tauri::command]
#[specta::specta]
pub async fn shift_with_dependents(
    pool: State<'_, SqlitePool>,
    todo_id: i64,
    planned_start: NaiveDate,
    planned_end: NaiveDate,
) -> CmdResult<Vec<i64>> {
    shift(
        &pool,
        todo_id,
        planned_start,
        planned_end,
        clock::today(),
        clock::now(),
    )
    .await
}

/// The dragged TODO, the moves and the TODOs they move (by id).
struct Plan {
    dragged: Todo,
    shifts: Vec<Shift>,
    later: HashMap<i64, Todo>,
}

/// What `todo_id` planned `start`〜`end` moves, on the data as it is now.
async fn plan(
    conn: &mut SqliteConnection,
    todo_id: i64,
    start: NaiveDate,
    end: NaiveDate,
) -> CmdResult<Plan> {
    validate::period(Some(start), Some(end), "予定")?;
    let dragged = repo::todos::get(conn, todo_id).await?;
    let dependencies = repo::dependencies::list(conn).await?;
    let later = repo::todos::list_by_ids(conn, &reachable(todo_id, &dependencies)).await?;
    let shifts = dependent_shifts(
        todo_id,
        Some(end),
        later.iter().chain([&dragged]),
        &dependencies,
    );
    // A moved plan keeps its length: it stays valid (the table refuses a
    // plan ending before it starts).
    Ok(Plan {
        dragged,
        shifts,
        later: later.into_iter().map(|t| (t.id, t)).collect(),
    })
}

pub async fn preview(
    pool: &SqlitePool,
    todo_id: i64,
    start: NaiveDate,
    end: NaiveDate,
) -> CmdResult<Vec<DependentShift>> {
    let mut conn = pool.acquire().await?;
    let plan = plan(&mut conn, todo_id, start, end).await?;
    if plan.shifts.is_empty() {
        return Ok(Vec::new());
    }
    let projects: HashMap<i64, String> = repo::projects::list(&mut conn, true)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let parent_ids: Vec<i64> = plan
        .shifts
        .iter()
        .filter_map(|s| plan.later.get(&s.id)?.parent_id)
        .collect();
    let parents: HashMap<i64, String> = repo::todos::list_by_ids(&mut conn, &parent_ids)
        .await?
        .into_iter()
        .map(|t| (t.id, t.title))
        .collect();
    plan.shifts
        .iter()
        .map(|s| {
            let t = plan.later.get(&s.id).ok_or_else(AppError::internal)?;
            Ok(DependentShift {
                id: t.id,
                title: t.title.clone(),
                parent_title: t.parent_id.and_then(|p| parents.get(&p).cloned()),
                project_id: t.project_id,
                project_name: projects.get(&t.project_id).cloned().unwrap_or_default(),
                planned_start: s.start,
                planned_end: s.end,
                shifted_start: s.new_start,
                shifted_end: s.new_end,
                days: s.days,
            })
        })
        .collect()
}

pub async fn shift(
    pool: &SqlitePool,
    todo_id: i64,
    start: NaiveDate,
    end: NaiveDate,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<Vec<i64>> {
    let mut tx = pool.begin().await?;
    let plan = plan(&mut tx, todo_id, start, end).await?;
    save_planned(&mut tx, &plan.dragged, Some(start), Some(end), today, now).await?;
    let mut moved = Vec::with_capacity(plan.shifts.len() + 1);
    moved.push(todo_id);
    for s in &plan.shifts {
        let before = plan.later.get(&s.id).ok_or_else(AppError::internal)?;
        save_planned(&mut tx, before, Some(s.new_start), s.new_end, today, now).await?;
        moved.push(s.id);
    }
    tx.commit().await?;
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::commands::projects::{self, ProjectInput};
    use crate::commands::todos::{self, NewTodoInput, TodoFields};
    use crate::domain::events::EventKind;
    use crate::domain::model::Status;
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

    /// A TODO planned `start`〜`end` (`""` = no date).
    async fn add(
        pool: &SqlitePool,
        project_id: i64,
        parent: Option<i64>,
        title: &str,
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
                due_date: Some(d("2026-12-20")),
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

    async fn join(pool: &SqlitePool, later: &Todo, first: &Todo) {
        crate::commands::dependencies::add(pool, later.id, first.id, today(), created())
            .await
            .unwrap();
    }

    async fn get(pool: &SqlitePool, id: i64) -> Todo {
        todos::get(pool, id, today()).await.unwrap().todo
    }

    /// The `planned` rows of a TODO's history: (old, new).
    async fn planned_events(pool: &SqlitePool, id: i64) -> Vec<(String, String)> {
        let mut c = pool.acquire().await.unwrap();
        repo::events::list_by_todo(&mut c, id)
            .await
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == EventKind::Planned)
            .map(|e| (e.old_value.unwrap(), e.new_value.unwrap()))
            .collect()
    }

    fn period(start: &str, end: &str) -> String {
        let value = |s: &str| {
            if s.is_empty() {
                "null".to_string()
            } else {
                format!("\"{s}\"")
            }
        };
        format!("{{\"start\":{},\"end\":{}}}", value(start), value(end))
    }

    fn lines(shifts: &[DependentShift]) -> Vec<(i64, String, String, i64)> {
        shifts
            .iter()
            .map(|s| {
                (
                    s.id,
                    s.shifted_start.to_string(),
                    s.shifted_end.map(|e| e.to_string()).unwrap_or_default(),
                    s.days,
                )
            })
            .collect()
    }

    fn line(id: i64, start: &str, end: &str, days: i64) -> (i64, String, String, i64) {
        (id, start.into(), end.into(), days)
    }

    #[tokio::test]
    async fn preview_lists_the_later_todos_with_their_names() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "基幹システム更改").await;
        let q = project(&pool, "社内ツール整備").await;
        let a = add(&pool, p, None, "API設計", "2026-11-16", "2026-11-20").await;
        let b = add(&pool, p, None, "結合テスト計画", "2026-11-22", "2026-11-24").await;
        let parent = add(&pool, q, None, "開発環境の標準化", "", "").await;
        let c = add(&pool, q, Some(parent.id), "手順書", "2026-11-25", "").await;
        join(&pool, &b, &a).await;
        join(&pool, &c, &b).await;

        let got = preview(&pool, a.id, d("2026-11-21"), d("2026-11-25"))
            .await
            .unwrap();
        assert_eq!(
            got,
            vec![
                DependentShift {
                    id: b.id,
                    title: "結合テスト計画".into(),
                    parent_title: None,
                    project_id: p,
                    project_name: "基幹システム更改".into(),
                    planned_start: d("2026-11-22"),
                    planned_end: Some(d("2026-11-24")),
                    shifted_start: d("2026-11-25"),
                    shifted_end: Some(d("2026-11-27")),
                    days: 3,
                },
                DependentShift {
                    id: c.id,
                    title: "手順書".into(),
                    parent_title: Some("開発環境の標準化".into()),
                    project_id: q,
                    project_name: "社内ツール整備".into(),
                    planned_start: d("2026-11-25"),
                    planned_end: None,
                    shifted_start: d("2026-11-27"),
                    shifted_end: None,
                    days: 2,
                },
            ]
        );
        // Nothing written.
        assert_eq!(get(&pool, a.id).await.planned_end, Some(d("2026-11-20")));
        assert_eq!(get(&pool, b.id).await.planned_start, Some(d("2026-11-22")));
        assert!(planned_events(&pool, b.id).await.is_empty());

        // Room left: nothing to ask.
        assert_eq!(
            preview(&pool, a.id, d("2026-11-18"), d("2026-11-22"))
                .await
                .unwrap(),
            vec![]
        );
    }

    #[tokio::test]
    async fn shift_writes_every_plan_and_its_history() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add(&pool, p, None, "A", "2026-11-16", "2026-11-20").await;
        let b = add(&pool, p, None, "B", "2026-11-22", "2026-11-24").await;
        let c = add(&pool, p, None, "C", "2026-11-25", "2026-11-26").await;
        // Room: not moved.
        let e = add(&pool, p, None, "E", "2026-12-10", "2026-12-11").await;
        join(&pool, &b, &a).await;
        join(&pool, &c, &b).await;
        join(&pool, &e, &c).await;

        let moved = shift(
            &pool,
            a.id,
            d("2026-11-21"),
            d("2026-11-25"),
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(moved, vec![a.id, b.id, c.id]);

        let plan = |t: &Todo| (t.planned_start, t.planned_end, t.updated_at);
        assert_eq!(
            plan(&get(&pool, a.id).await),
            (Some(d("2026-11-21")), Some(d("2026-11-25")), now())
        );
        assert_eq!(
            plan(&get(&pool, b.id).await),
            (Some(d("2026-11-25")), Some(d("2026-11-27")), now())
        );
        assert_eq!(
            plan(&get(&pool, c.id).await),
            (Some(d("2026-11-27")), Some(d("2026-11-28")), now())
        );
        let unmoved = get(&pool, e.id).await;
        assert_eq!(
            plan(&unmoved),
            (Some(d("2026-12-10")), Some(d("2026-12-11")), created())
        );
        // The 日限 stays.
        assert_eq!(get(&pool, b.id).await.due_date, Some(d("2026-12-20")));

        // One `planned` row each, as update_todo writes it.
        assert_eq!(
            planned_events(&pool, a.id).await,
            [(
                period("2026-11-16", "2026-11-20"),
                period("2026-11-21", "2026-11-25")
            )]
        );
        assert_eq!(
            planned_events(&pool, b.id).await,
            [(
                period("2026-11-22", "2026-11-24"),
                period("2026-11-25", "2026-11-27")
            )]
        );
        assert_eq!(
            planned_events(&pool, c.id).await,
            [(
                period("2026-11-25", "2026-11-26"),
                period("2026-11-27", "2026-11-28")
            )]
        );
        assert!(planned_events(&pool, e.id).await.is_empty());
        let mut conn = pool.acquire().await.unwrap();
        let rows = repo::events::list_by_todo(&mut conn, b.id).await.unwrap();
        let row = rows.iter().find(|e| e.kind == EventKind::Planned).unwrap();
        assert_eq!((row.event_date, row.occurred_at), (today(), now()));
    }

    #[tokio::test]
    async fn shift_works_the_moves_out_again() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add(&pool, p, None, "A", "2026-11-16", "2026-11-20").await;
        let b = add(&pool, p, None, "B", "2026-11-22", "2026-11-24").await;
        let c = add(&pool, p, None, "C", "2026-11-28", "2026-11-29").await;
        join(&pool, &b, &a).await;
        join(&pool, &c, &b).await;
        assert_eq!(
            lines(
                &preview(&pool, a.id, d("2026-11-21"), d("2026-11-25"))
                    .await
                    .unwrap()
            ),
            [line(b.id, "2026-11-25", "2026-11-27", 3)]
        );

        // After the confirmation, B is planned 11/24〜11/27 elsewhere (the
        // detail panel).
        todos::update(
            &pool,
            b.id,
            TodoFields {
                planned_start: Some(d("2026-11-24")),
                planned_end: Some(d("2026-11-27")),
                ..fields(&get(&pool, b.id).await)
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        let moved = shift(
            &pool,
            a.id,
            d("2026-11-21"),
            d("2026-11-25"),
            today(),
            now(),
        )
        .await
        .unwrap();
        // B lacks one day now; C (28〜) still has room after B's new end.
        assert_eq!(moved, vec![a.id, b.id]);
        let b_now = get(&pool, b.id).await;
        assert_eq!(
            (b_now.planned_start, b_now.planned_end),
            (Some(d("2026-11-25")), Some(d("2026-11-28")))
        );
        let c_now = get(&pool, c.id).await;
        assert_eq!(c_now.planned_start, Some(d("2026-11-28")));

        // A closed later TODO is not moved, nor what waits for it.
        todos::change_status_impl(
            &pool,
            b.id,
            &StatusChange::to(Status::Doing),
            today(),
            now(),
        )
        .await
        .unwrap();
        todos::change_status_impl(&pool, b.id, &StatusChange::to(Status::Done), today(), now())
            .await
            .unwrap();
        let moved = shift(
            &pool,
            a.id,
            d("2026-11-26"),
            d("2026-11-30"),
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(moved, vec![a.id]);
        assert_eq!(get(&pool, c.id).await.planned_start, Some(d("2026-11-28")));
    }

    fn fields(t: &Todo) -> TodoFields {
        TodoFields {
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

    #[tokio::test]
    async fn nothing_changes_when_a_plan_is_refused() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add(&pool, p, None, "A", "2026-11-16", "2026-11-20").await;
        let b = add(&pool, p, None, "B", "2026-11-22", "2026-11-24").await;
        join(&pool, &b, &a).await;
        let before = get(&pool, a.id).await;

        // The dragged plan ends before it starts.
        for r in [
            shift(
                &pool,
                a.id,
                d("2026-11-26"),
                d("2026-11-25"),
                today(),
                now(),
            )
            .await,
            preview(&pool, a.id, d("2026-11-26"), d("2026-11-25"))
                .await
                .map(|_| Vec::new()),
        ] {
            let e = r.unwrap_err();
            assert_eq!(e.kind, ErrorKind::Validation);
            assert_eq!(
                e.message,
                "予定開始日が予定終了日より後になっています。日付を直してください。"
            );
        }

        assert_eq!(get(&pool, a.id).await, before);
        assert_eq!(get(&pool, b.id).await.planned_start, Some(d("2026-11-22")));
        assert!(planned_events(&pool, a.id).await.is_empty());
        assert!(planned_events(&pool, b.id).await.is_empty());

        // A TODO that is gone.
        let e = shift(&pool, 999, d("2026-11-21"), d("2026-11-25"), today(), now())
            .await
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn without_later_todos_only_the_dragged_one_moves() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add(&pool, p, None, "A", "2026-11-16", "2026-11-20").await;
        assert_eq!(
            preview(&pool, a.id, d("2026-11-18"), d("2026-11-22"))
                .await
                .unwrap(),
            vec![]
        );
        let moved = shift(
            &pool,
            a.id,
            d("2026-11-18"),
            d("2026-11-22"),
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(moved, vec![a.id]);
        assert_eq!(
            planned_events(&pool, a.id).await,
            [(
                period("2026-11-16", "2026-11-20"),
                period("2026-11-18", "2026-11-22")
            )]
        );
    }
}
