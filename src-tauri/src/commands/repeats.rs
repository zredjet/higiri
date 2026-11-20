//! 繰り返しのTODO (T-12, v0.3.0): set_todo_repeat, and the next round that
//! `change_status` makes when a repeating TODO is closed.
//!
//! Only the current round has the repetition. Closing it (完了 or 中止)
//! makes the next round with its children and moves the repetition there,
//! in the same transaction as the status change. Reopening the previous
//! round leaves the next one as it is, and closing it again makes nothing
//! (it has no repetition any more).

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::{SqliteConnection, SqlitePool};
use tauri::State;

use super::clock;
use super::error::{AppError, CmdResult};
use super::todos::item;
use crate::domain::date::days_between;
use crate::domain::dependency::positions_among;
use crate::domain::events;
use crate::domain::model::Todo;
use crate::domain::repeat::{RepeatRule, RoundDates, check_target, copied_children};
use crate::domain::urgency::Rules;
use crate::domain::views::TodoItem;
use crate::repo;
use crate::repo::todos::NewTodo;

/// The repetition set on a TODO.
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct RepeatInput {
    pub rule: RepeatRule,
    /// The last day a round may fall on (not before the current 日限).
    /// `null` = no end.
    pub until_date: Option<NaiveDate>,
}

/// Gives a TODO a repetition, changes it, or (`null`) takes it off. Only a
/// parentless, open TODO with a 日限 gets one. Not recorded in the history.
#[tauri::command]
#[specta::specta]
pub async fn set_todo_repeat(
    pool: State<'_, SqlitePool>,
    id: i64,
    repeat: Option<RepeatInput>,
) -> CmdResult<TodoItem> {
    set(&pool, id, repeat, clock::today(), clock::now()).await
}

pub async fn set(
    pool: &SqlitePool,
    id: i64,
    repeat: Option<RepeatInput>,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<TodoItem> {
    let mut tx = pool.begin().await?;
    let todo = repo::todos::get(&mut tx, id).await?;
    let current = repo::repeats::get(&mut tx, id).await?;
    let saved = match (repeat, current) {
        // Nothing to take off.
        (None, None) => todo,
        (None, Some(_)) => {
            repo::repeats::delete(&mut tx, id).await?;
            repo::todos::touch(&mut tx, id, now).await?
        }
        (Some(input), current) => {
            check_target(&todo, input.until_date)
                .map_err(|e| AppError::validation(e.to_string()))?;
            input
                .rule
                .check()
                .map_err(|e| AppError::validation(e.to_string()))?;
            if current.is_some() {
                repo::repeats::update(&mut tx, id, &input.rule, input.until_date, now).await?;
            } else {
                repo::repeats::insert(&mut tx, id, &input.rule, input.until_date, now).await?;
            }
            repo::todos::touch(&mut tx, id, now).await?
        }
    };
    let out = item(&mut tx, saved, today).await?;
    tx.commit().await?;
    Ok(out)
}

/// After `prev` was closed: when it repeats, makes the next round (設計書
/// T-12 の手順 1〜3, 5, 6; copying the dependencies inside the set comes with
/// T-11) and moves the repetition to it. `children` are `prev`'s children as
/// they were before it was closed. When the next 日限 falls after the end
/// (or there is none to count from), the repetition is removed instead.
pub(super) async fn next_round(
    conn: &mut SqliteConnection,
    prev: &Todo,
    children: &[Todo],
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<()> {
    let Some(repeat) = repo::repeats::get(conn, prev.id).await? else {
        return Ok(());
    };
    let rules = Rules::from_settings(&repo::settings::load(conn).await?);
    let (Some(prev_due), Some(due)) = (
        prev.due_date,
        repeat.next_round_due(prev.due_date, today, &rules),
    ) else {
        repo::repeats::delete(conn, prev.id).await?;
        return Ok(());
    };
    let days = days_between(prev_due, due);
    let round = copy(
        conn,
        prev,
        prev.parent_id,
        prev.project_id,
        RoundDates::of(prev).shifted(days),
        today,
        now,
    )
    .await?;
    let copied: Vec<&Todo> = copied_children(children).collect();
    let mut made_ids = Vec::with_capacity(copied.len());
    for child in &copied {
        let dates = RoundDates::of(child).shifted(days);
        let made = copy(
            conn,
            child,
            Some(round.id),
            round.project_id,
            dates,
            today,
            now,
        )
        .await?;
        made_ids.push(made.id);
    }
    // The dependencies between the copied children (T-11); none with TODOs
    // outside the round.
    let old_ids: Vec<i64> = copied.iter().map(|c| c.id).collect();
    let inside = repo::dependencies::among(conn, &old_ids).await?;
    for (later, first) in positions_among(&old_ids, &inside) {
        repo::dependencies::insert(conn, made_ids[later], made_ids[first], now).await?;
    }
    repo::repeats::move_to(conn, prev.id, round.id, now).await?;
    Ok(())
}

/// Adds `from` again, 未着手 with these dates, and records its 「作成」 with
/// `repeated_from`. Title, priority and memo are copied; the work logs, the
/// actual period and the 待ち・中止 fields are not.
async fn copy(
    conn: &mut SqliteConnection,
    from: &Todo,
    parent_id: Option<i64>,
    project_id: i64,
    dates: RoundDates,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<Todo> {
    let made = repo::todos::insert(
        conn,
        &NewTodo {
            project_id,
            parent_id,
            title: from.title.clone(),
            priority: from.priority,
            due_date: dates.due_date,
            planned_start: dates.planned_start,
            planned_end: dates.planned_end,
            memo_md: from.memo_md.clone(),
        },
        now,
    )
    .await?;
    repo::events::insert(
        conn,
        made.id,
        &events::repeated(from.id, made.status, today, now),
    )
    .await?;
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::commands::projects::{self, ProjectInput};
    use crate::commands::settings::{self, SettingsInput};
    use crate::commands::todos::{self, NewTodoInput, TodoFields};
    use crate::commands::work_logs::{self, WorkLogInput};
    use crate::domain::calendar::DayCount;
    use crate::domain::events::{EventKind, TodoEvent};
    use crate::domain::model::{Priority, Status};
    use crate::domain::repeat::{DayOfWeek, RepeatInfo};
    use crate::domain::status::StatusChange;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    /// Thursday.
    fn today() -> NaiveDate {
        d("2026-11-19")
    }

    fn now() -> DateTime<Utc> {
        ts(2026, 11, 19, 1, 0)
    }

    fn monday() -> RepeatRule {
        RepeatRule::Weekly {
            weekdays: vec![DayOfWeek::Mon],
        }
    }

    fn every(rule: RepeatRule) -> Option<RepeatInput> {
        Some(RepeatInput {
            rule,
            until_date: None,
        })
    }

    async fn setup() -> (SqlitePool, i64) {
        let pool = db::open_in_memory().await.unwrap();
        let p = projects::create(
            &pool,
            ProjectInput {
                name: "基幹システム更改".into(),
                color: None,
                description: String::new(),
            },
            now(),
        )
        .await
        .unwrap();
        (pool, p.id)
    }

    fn input(project_id: i64, title: &str, due: Option<&str>) -> NewTodoInput {
        NewTodoInput {
            project_id,
            parent_id: None,
            template_id: None,
            title: title.into(),
            priority: None,
            due_date: due.map(d),
            planned_start: None,
            planned_end: None,
            memo_md: None,
        }
    }

    async fn add(pool: &SqlitePool, input: NewTodoInput) -> TodoItem {
        todos::create(pool, input, today(), now()).await.unwrap()
    }

    async fn close(pool: &SqlitePool, id: i64, change: StatusChange) -> TodoItem {
        todos::change_status_impl(pool, id, &change, today(), now())
            .await
            .unwrap()
    }

    fn done() -> StatusChange {
        StatusChange::to(Status::Done)
    }

    async fn all(pool: &SqlitePool) -> Vec<Todo> {
        let mut c = pool.acquire().await.unwrap();
        repo::todos::list(&mut c, &repo::todos::TodoFilter::default())
            .await
            .unwrap()
    }

    /// The TODOs added after `id`, in order.
    async fn made_after(pool: &SqlitePool, id: i64) -> Vec<Todo> {
        all(pool).await.into_iter().filter(|t| t.id > id).collect()
    }

    async fn events_of(pool: &SqlitePool, id: i64) -> Vec<TodoEvent> {
        let mut c = pool.acquire().await.unwrap();
        repo::events::list_by_todo(&mut c, id).await.unwrap()
    }

    async fn repeat_of(pool: &SqlitePool, id: i64) -> Option<RepeatRule> {
        let mut c = pool.acquire().await.unwrap();
        repo::repeats::get(&mut c, id)
            .await
            .unwrap()
            .map(|r| r.rule)
    }

    fn message(r: CmdResult<TodoItem>) -> String {
        r.unwrap_err().message
    }

    fn fields_of(t: &Todo) -> TodoFields {
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

    // ---- set_todo_repeat ---------------------------------------------------

    #[tokio::test]
    async fn set_gives_changes_and_takes_off_the_repetition() {
        let (pool, pid) = setup().await;
        let t = add(&pool, input(pid, "定例の資料", Some("2026-11-23"))).await;
        assert_eq!(t.repeat, None);
        let id = t.todo.id;

        let later = ts(2026, 11, 19, 3, 0);
        let set1 = set(
            &pool,
            id,
            Some(RepeatInput {
                rule: monday(),
                until_date: Some(d("2027-03-31")),
            }),
            today(),
            later,
        )
        .await
        .unwrap();
        assert_eq!(
            set1.repeat,
            Some(RepeatInfo {
                rule: monday(),
                until_date: Some(d("2027-03-31")),
                next_due: Some(d("2026-11-30")),
            })
        );
        assert_eq!(set1.todo.updated_at, later);
        assert_eq!(todos::get(&pool, id, today()).await.unwrap(), set1);

        let last = RepeatRule::MonthlyLast;
        let set2 = set(&pool, id, every(last.clone()), today(), now())
            .await
            .unwrap();
        assert_eq!(set2.repeat.as_ref().map(|r| &r.rule), Some(&last));
        assert_eq!(set2.repeat.as_ref().and_then(|r| r.until_date), None);
        assert_eq!(set2.repeat.and_then(|r| r.next_due), Some(d("2026-11-30")));

        let off = set(&pool, id, None, today(), now()).await.unwrap();
        assert_eq!(off.repeat, None);
        assert_eq!(repeat_of(&pool, id).await, None);
        // Taking it off again does nothing.
        assert_eq!(
            set(&pool, id, None, today(), now()).await.unwrap().repeat,
            None
        );
        // Not recorded in the history.
        assert_eq!(
            events_of(&pool, id)
                .await
                .into_iter()
                .map(|e| e.kind)
                .collect::<Vec<_>>(),
            vec![EventKind::Created]
        );
    }

    #[tokio::test]
    async fn set_refuses_children_closed_todos_and_todos_without_a_due_date() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, input(pid, "親", Some("2026-11-30")))
            .await
            .todo
            .id;
        let child = add(
            &pool,
            NewTodoInput {
                parent_id: Some(parent),
                ..input(pid, "子", Some("2026-11-27"))
            },
        )
        .await
        .todo
        .id;
        let no_due = add(&pool, input(pid, "日限なし", None)).await.todo.id;
        let finished = add(&pool, input(pid, "終わった", Some("2026-11-20")))
            .await
            .todo
            .id;
        close(&pool, finished, done()).await;
        let try_set = |id: i64, input: Option<RepeatInput>| set(&pool, id, input, today(), now());

        assert_eq!(
            message(try_set(child, every(RepeatRule::Daily)).await),
            "子TODOには繰り返しを付けられません(繰り返しを付けられるのは親のないTODOだけです)。親から外してから付けてください。"
        );
        assert_eq!(
            message(try_set(finished, every(RepeatRule::Daily)).await),
            "完了にしたTODOには繰り返しを付けられません。ステータスを戻してから付けてください。"
        );
        assert_eq!(
            message(try_set(no_due, every(RepeatRule::Daily)).await),
            "繰り返しを付けるには日限が要ります。先に日限を入力してください。"
        );
        let ends_early = Some(RepeatInput {
            rule: RepeatRule::Daily,
            until_date: Some(d("2026-11-29")),
        });
        assert_eq!(
            message(try_set(parent, ends_early).await),
            "終わりの日が今の日限(11月30日)より前です。日限と同じ日か、それより後の日を選んでください。"
        );
        assert_eq!(
            message(try_set(parent, every(RepeatRule::EveryDays { days: 1 })).await),
            "日数が2〜365日の範囲にありません。2〜365日で入力してください。"
        );
        assert_eq!(
            message(try_set(parent, every(RepeatRule::Weekly { weekdays: vec![] })).await),
            "曜日が選ばれていません。曜日を1つ以上選んでください。"
        );
        assert_eq!(
            try_set(999, every(RepeatRule::Daily))
                .await
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );
        let mut c = pool.acquire().await.unwrap();
        assert!(repo::repeats::list(&mut c).await.unwrap().is_empty());
        // The end on the 日限 itself is fine.
        drop(c);
        let on_due = Some(RepeatInput {
            rule: RepeatRule::Daily,
            until_date: Some(d("2026-11-30")),
        });
        assert!(try_set(parent, on_due).await.is_ok());
    }

    #[tokio::test]
    async fn a_repeating_todo_keeps_its_due_date_and_its_place() {
        let (pool, pid) = setup().await;
        let t = add(&pool, input(pid, "定例の資料", Some("2026-11-23"))).await;
        let other = add(&pool, input(pid, "親になるTODO", None)).await.todo.id;
        set(&pool, t.todo.id, every(monday()), today(), now())
            .await
            .unwrap();

        let mut f = fields_of(&t.todo);
        f.due_date = None;
        assert_eq!(
            message(todos::update(&pool, t.todo.id, f.clone(), today(), now()).await),
            "繰り返しのあるTODOは、日限を空にできません。繰り返しを外してから日限を消してください。"
        );
        // Moving it is fine.
        f.due_date = Some(d("2026-11-30"));
        let moved = todos::update(&pool, t.todo.id, f, today(), now())
            .await
            .unwrap();
        assert_eq!(moved.repeat.and_then(|r| r.next_due), Some(d("2026-12-07")));

        assert_eq!(
            message(todos::set_parent(&pool, t.todo.id, Some(other), today(), now()).await),
            "繰り返しのあるTODOは、ほかのTODOの子にできません。先に繰り返しを外してください。"
        );
        // Without the repetition, both are fine again.
        set(&pool, t.todo.id, None, today(), now()).await.unwrap();
        assert!(
            todos::set_parent(&pool, t.todo.id, Some(other), today(), now())
                .await
                .is_ok()
        );
    }

    // ---- 次の回 --------------------------------------------------------------

    #[tokio::test]
    async fn completing_a_weekly_todo_makes_next_mondays_round() {
        let (pool, pid) = setup().await;
        let prev = add(
            &pool,
            NewTodoInput {
                priority: Some(Priority::High),
                memo_md: Some("- 前回の議事録を見る".into()),
                ..input(pid, "定例の資料", Some("2026-11-19"))
            },
        )
        .await
        .todo;
        set(&pool, prev.id, every(monday()), today(), now())
            .await
            .unwrap();
        work_logs::save(
            &pool,
            WorkLogInput {
                id: None,
                todo_id: prev.id,
                log_date: today(),
                body_md: "作った".into(),
                minutes: Some(30),
            },
            now(),
        )
        .await
        .unwrap();
        close(&pool, prev.id, StatusChange::to(Status::Doing)).await;

        let closed = close(&pool, prev.id, done()).await;
        assert_eq!(closed.todo.status, Status::Done);
        assert_eq!(closed.repeat, None);

        let made = made_after(&pool, prev.id).await;
        assert_eq!(made.len(), 1);
        let next = &made[0];
        assert_eq!(next.title, "定例の資料");
        assert_eq!(next.priority, Priority::High);
        assert_eq!(next.memo_md, "- 前回の議事録を見る");
        assert_eq!(next.project_id, pid);
        assert_eq!(next.parent_id, None);
        assert_eq!(next.status, Status::Todo);
        assert_eq!(next.due_date, Some(d("2026-11-23")));
        assert_eq!(
            (next.actual_start, next.actual_end, next.closed_at),
            (None, None, None)
        );
        // The repetition moved to it, with the next round after.
        let item = todos::get(&pool, next.id, today()).await.unwrap();
        assert_eq!(
            item.repeat,
            Some(RepeatInfo {
                rule: monday(),
                until_date: None,
                next_due: Some(d("2026-11-30")),
            })
        );
        assert_eq!(repeat_of(&pool, prev.id).await, None);
        // Its history: 「作成」 with where it came from. No work logs.
        let evs = events_of(&pool, next.id).await;
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].kind, EventKind::Created);
        assert_eq!(evs[0].note.repeated_from, Some(prev.id));
        assert_eq!(
            evs[0].note.to_json(),
            format!(r#"{{"repeated_from":{}}}"#, prev.id)
        );
        let timeline = work_logs::timeline(&pool, next.id).await.unwrap();
        assert_eq!(timeline.len(), 1);
        assert!(format!("{timeline:?}").contains("繰り返しで作成"));
    }

    #[tokio::test]
    async fn cancelling_makes_the_next_round_too() {
        let (pool, pid) = setup().await;
        let prev = add(&pool, input(pid, "月末の締め", Some("2026-11-30")))
            .await
            .todo;
        set(
            &pool,
            prev.id,
            every(RepeatRule::MonthlyLast),
            today(),
            now(),
        )
        .await
        .unwrap();
        close(&pool, prev.id, StatusChange::cancel("今月は不要")).await;
        let made = made_after(&pool, prev.id).await;
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].due_date, Some(d("2026-12-31")));
        assert_eq!(made[0].status, Status::Todo);
        assert_eq!(made[0].close_reason, None);
        assert_eq!(
            repeat_of(&pool, made[0].id).await,
            Some(RepeatRule::MonthlyLast)
        );
    }

    #[tokio::test]
    async fn past_the_last_day_no_round_is_made_and_the_repetition_ends() {
        let (pool, pid) = setup().await;
        let prev = add(&pool, input(pid, "定例の資料", Some("2026-11-23")))
            .await
            .todo;
        let item = set(
            &pool,
            prev.id,
            Some(RepeatInput {
                rule: monday(),
                until_date: Some(d("2026-11-29")),
            }),
            today(),
            now(),
        )
        .await
        .unwrap();
        // The screen can tell: no next round.
        assert_eq!(item.repeat.and_then(|r| r.next_due), None);
        close(&pool, prev.id, done()).await;
        assert!(made_after(&pool, prev.id).await.is_empty());
        let mut c = pool.acquire().await.unwrap();
        assert!(repo::repeats::list(&mut c).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_planned_period_moves_with_the_due_date() {
        let (pool, pid) = setup().await;
        let both = add(
            &pool,
            NewTodoInput {
                planned_start: Some(d("2026-11-16")),
                planned_end: Some(d("2026-11-18")),
                ..input(pid, "両方", Some("2026-11-19"))
            },
        )
        .await
        .todo;
        let end_only = add(
            &pool,
            NewTodoInput {
                planned_end: Some(d("2026-11-18")),
                ..input(pid, "終了日だけ", Some("2026-11-19"))
            },
        )
        .await
        .todo;
        for t in [&both, &end_only] {
            set(&pool, t.id, every(monday()), today(), now())
                .await
                .unwrap();
        }
        close(&pool, both.id, done()).await;
        close(&pool, end_only.id, done()).await;
        let made = made_after(&pool, end_only.id).await;
        // Thu 11/19 → Mon 11/23: four days.
        let dates: Vec<_> = made
            .iter()
            .map(|t| (t.title.as_str(), t.due_date, t.planned_start, t.planned_end))
            .collect();
        assert_eq!(
            dates,
            vec![
                (
                    "両方",
                    Some(d("2026-11-23")),
                    Some(d("2026-11-20")),
                    Some(d("2026-11-22"))
                ),
                (
                    "終了日だけ",
                    Some(d("2026-11-23")),
                    None,
                    Some(d("2026-11-22"))
                ),
            ]
        );
    }

    #[tokio::test]
    async fn the_children_are_copied_except_those_cancelled_before() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, input(pid, "月次締め", Some("2026-11-30")))
            .await
            .todo
            .id;
        let child = |title: &str, due: Option<&str>| NewTodoInput {
            parent_id: Some(parent),
            planned_start: due.map(|_| d("2026-11-24")),
            planned_end: due.map(d),
            ..input(pid, title, due)
        };
        let open = add(&pool, child("請求書の確認", Some("2026-11-27")))
            .await
            .todo
            .id;
        let finished = add(&pool, child("経費の集計", Some("2026-11-26")))
            .await
            .todo
            .id;
        let dropped = add(&pool, child("やめた作業", None)).await.todo.id;
        let waiting = add(&pool, child("承認待ち", None)).await.todo.id;
        close(&pool, finished, done()).await;
        close(&pool, dropped, StatusChange::cancel("不要になった")).await;
        close(
            &pool,
            waiting,
            StatusChange::waiting("部長の承認", d("2026-11-25")),
        )
        .await;
        set(
            &pool,
            parent,
            every(RepeatRule::MonthlyDay { day: 31 }),
            today(),
            now(),
        )
        .await
        .unwrap();

        // 中止 closes the open children with the same reason; they are
        // copied, the one cancelled before is not.
        close(&pool, parent, StatusChange::cancel("計画の見直し")).await;
        assert_eq!(
            todos::get(&pool, open, today()).await.unwrap().todo.status,
            Status::Cancelled
        );
        let made = made_after(&pool, waiting).await;
        let round = &made[0];
        assert_eq!(round.title, "月次締め");
        assert_eq!(round.due_date, Some(d("2026-12-31")));
        let shape: Vec<_> = made[1..]
            .iter()
            .map(|t| {
                (
                    t.title.as_str(),
                    t.parent_id,
                    t.status,
                    t.due_date,
                    t.planned_start,
                    t.planned_end,
                )
            })
            .collect();
        // 11/30 → 12/31: 31 days.
        assert_eq!(
            shape,
            vec![
                (
                    "請求書の確認",
                    Some(round.id),
                    Status::Todo,
                    Some(d("2026-12-28")),
                    Some(d("2026-12-25")),
                    Some(d("2026-12-28"))
                ),
                (
                    "経費の集計",
                    Some(round.id),
                    Status::Todo,
                    Some(d("2026-12-27")),
                    Some(d("2026-12-25")),
                    Some(d("2026-12-27"))
                ),
                ("承認待ち", Some(round.id), Status::Todo, None, None, None),
            ]
        );
        for (copy, from) in made[1..].iter().zip([open, finished, waiting]) {
            let evs = events_of(&pool, copy.id).await;
            assert_eq!(evs.len(), 1);
            assert_eq!(evs[0].note.repeated_from, Some(from));
            assert_eq!(copy.waiting_on, None);
        }
        let item = todos::get(&pool, round.id, today()).await.unwrap();
        assert_eq!(item.children.map(|c| (c.total, c.open)), Some((3, 3)));
    }

    #[tokio::test]
    async fn the_dependencies_inside_the_round_are_copied_and_those_outside_are_not() {
        use crate::commands::dependencies;
        let (pool, pid) = setup().await;
        let parent = add(&pool, input(pid, "月次締め", Some("2026-11-30")))
            .await
            .todo
            .id;
        let child = |title: &str| NewTodoInput {
            parent_id: Some(parent),
            ..input(pid, title, None)
        };
        let sales = add(&pool, child("売上の集計")).await.todo.id;
        let check = add(&pool, child("請求書の確認")).await.todo.id;
        let report = add(&pool, child("報告書の作成")).await.todo.id;
        let dropped = add(&pool, child("やめた作業")).await.todo.id;
        let outside = add(&pool, input(pid, "前月の確定", None)).await.todo.id;
        let after = add(&pool, input(pid, "経営会議の資料", None)).await.todo.id;
        let join = |later: i64, first: i64| {
            let pool = pool.clone();
            async move {
                dependencies::add(&pool, later, first, today(), now())
                    .await
                    .unwrap();
            }
        };
        // Inside: 報告書 → 売上, 報告書 → 確認, 確認 → 売上, 報告書 → やめた作業.
        // Outside: 売上 → 前月の確定, 経営会議の資料 → 報告書, 経営会議の資料 → 親.
        join(report, sales).await;
        join(report, check).await;
        join(check, sales).await;
        join(report, dropped).await;
        join(sales, outside).await;
        join(after, report).await;
        join(after, parent).await;
        close(&pool, dropped, StatusChange::cancel("不要")).await;
        set(
            &pool,
            parent,
            every(RepeatRule::MonthlyLast),
            today(),
            now(),
        )
        .await
        .unwrap();
        close(&pool, parent, done()).await;

        let made = made_after(&pool, after).await;
        let ids: Vec<i64> = made.iter().map(|t| t.id).collect();
        let titles: Vec<&str> = made.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            titles,
            ["月次締め", "売上の集計", "請求書の確認", "報告書の作成"]
        );
        let (round, new_sales, new_check, new_report) = (ids[0], ids[1], ids[2], ids[3]);
        let mut c = pool.acquire().await.unwrap();
        let mut copied: Vec<(i64, i64)> = repo::dependencies::list(&mut c)
            .await
            .unwrap()
            .into_iter()
            .map(|d| (d.todo_id, d.depends_on_id))
            .filter(|(later, first)| ids.contains(later) || ids.contains(first))
            .collect();
        copied.sort_unstable();
        let mut expected = vec![
            (new_check, new_sales),
            (new_report, new_sales),
            (new_report, new_check),
        ];
        expected.sort_unstable();
        assert_eq!(copied, expected);
        // The old round keeps its own.
        assert!(
            repo::dependencies::get(&mut c, after, parent)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            repo::dependencies::get(&mut c, report, sales)
                .await
                .unwrap()
                .is_some()
        );
        drop(c);
        assert!(made[1..].iter().all(|t| t.parent_id == Some(round)));
    }

    #[tokio::test]
    async fn reopening_and_closing_again_makes_no_second_round() {
        let (pool, pid) = setup().await;
        let prev = add(&pool, input(pid, "定例の資料", Some("2026-11-19")))
            .await
            .todo;
        set(&pool, prev.id, every(monday()), today(), now())
            .await
            .unwrap();
        close(&pool, prev.id, done()).await;
        let reopened = close(&pool, prev.id, StatusChange::to(Status::Doing)).await;
        assert_eq!(reopened.repeat, None);
        close(&pool, prev.id, done()).await;
        let made = made_after(&pool, prev.id).await;
        assert_eq!(made.len(), 1);
        assert_eq!(repeat_of(&pool, made[0].id).await, Some(monday()));
    }

    #[tokio::test]
    async fn a_failed_close_makes_no_round() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, input(pid, "親", Some("2026-11-19")))
            .await
            .todo
            .id;
        let child = add(
            &pool,
            NewTodoInput {
                parent_id: Some(parent),
                ..input(pid, "先の子", None)
            },
        )
        .await
        .todo;
        let mut f = fields_of(&child);
        f.actual_start = Some(d("2026-11-25"));
        todos::update(&pool, child.id, f, today(), now())
            .await
            .unwrap();
        set(&pool, parent, every(monday()), today(), now())
            .await
            .unwrap();
        assert!(
            todos::change_status_impl(&pool, parent, &done(), today(), now())
                .await
                .is_err()
        );
        assert!(made_after(&pool, child.id).await.is_empty());
        assert_eq!(repeat_of(&pool, parent).await, Some(monday()));
    }

    #[tokio::test]
    async fn in_business_days_the_next_round_skips_the_weekend() {
        let (pool, pid) = setup().await;
        let s = settings::get(&pool).await.unwrap();
        settings::update(
            &pool,
            SettingsInput {
                day_count: DayCount::Business,
                ..SettingsInput::from(&s)
            },
        )
        .await
        .unwrap();
        // Fri 11/27, daily: not Sat or Sun (judged on Friday) but Mon.
        let prev = add(&pool, input(pid, "日次の確認", Some("2026-11-27")))
            .await
            .todo;
        set(&pool, prev.id, every(RepeatRule::Daily), today(), now())
            .await
            .unwrap();
        close(&pool, prev.id, done()).await;
        let made = made_after(&pool, prev.id).await;
        assert_eq!(made[0].due_date, Some(d("2026-11-30")));
    }

    #[tokio::test]
    async fn the_list_carries_each_repetition() {
        let (pool, pid) = setup().await;
        let a = add(&pool, input(pid, "a", Some("2026-11-23")))
            .await
            .todo
            .id;
        add(&pool, input(pid, "b", Some("2026-11-23"))).await;
        set(&pool, a, every(monday()), today(), now())
            .await
            .unwrap();
        let list = todos::list(&pool, &todos::TodoQuery::default(), today())
            .await
            .unwrap();
        let repeats: Vec<_> = list
            .rows
            .iter()
            .map(|r| {
                (
                    r.item.todo.title.as_str(),
                    r.item.repeat.as_ref().map(|x| x.next_due),
                )
            })
            .collect();
        assert_eq!(
            repeats,
            vec![("a", Some(Some(d("2026-11-30")))), ("b", None)]
        );
    }

    #[tokio::test]
    async fn deleting_the_todo_stops_the_repetition() {
        let (pool, pid) = setup().await;
        let a = add(&pool, input(pid, "a", Some("2026-11-23")))
            .await
            .todo
            .id;
        set(&pool, a, every(monday()), today(), now())
            .await
            .unwrap();
        todos::delete(&pool, a).await.unwrap();
        let mut c = pool.acquire().await.unwrap();
        assert!(repo::repeats::list(&mut c).await.unwrap().is_empty());
    }
}
