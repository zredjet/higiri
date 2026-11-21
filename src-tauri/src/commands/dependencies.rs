//! TODO間の依存関係 (T-11, v0.3.0): add_dependency, remove_dependency,
//! list_dependencies.
//!
//! 「`todo_id` のTODOは、`depends_on_id` のTODOが終わるまで取りかかれない」.
//! Nothing is stopped: `change_status` lets a TODO go 作業中 whatever it
//! waits for (the screen asks first). Joining and removing are not recorded
//! in the history; they change the later TODO's `updated_at`, like moving it
//! under another parent.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

use super::clock;
use super::error::{AppError, CmdResult};
use super::todos::{item, items};
use crate::domain::dependency::check_new;
use crate::domain::views::TodoItem;
use crate::repo;
use crate::repo::db;

/// The TODOs around one in the detail panel, each in the order the
/// dependencies were added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct DependencyList {
    /// 先行TODO: the TODOs it waits for.
    pub depends_on: Vec<TodoItem>,
    /// 後続TODO: the TODOs waiting for it.
    pub dependents: Vec<TodoItem>,
}

/// Makes `todo_id` wait for `depends_on_id` (先行TODOを足す).
/// Refused: itself, a pair twice, a circle, a parent and its own child.
/// Projects may differ and either TODO may be closed. Returns the later TODO.
#[tauri::command]
#[specta::specta]
pub async fn add_dependency(
    pool: State<'_, SqlitePool>,
    todo_id: i64,
    depends_on_id: i64,
) -> CmdResult<TodoItem> {
    add(&pool, todo_id, depends_on_id, clock::today(), clock::now()).await
}

/// Stops `todo_id` waiting for `depends_on_id`. Returns the later TODO.
#[tauri::command]
#[specta::specta]
pub async fn remove_dependency(
    pool: State<'_, SqlitePool>,
    todo_id: i64,
    depends_on_id: i64,
) -> CmdResult<TodoItem> {
    remove(&pool, todo_id, depends_on_id, clock::today(), clock::now()).await
}

/// The TODOs `todo_id` waits for and those waiting for it.
#[tauri::command]
#[specta::specta]
pub async fn list_dependencies(
    pool: State<'_, SqlitePool>,
    todo_id: i64,
) -> CmdResult<DependencyList> {
    list(&pool, todo_id, clock::today()).await
}

pub async fn add(
    pool: &SqlitePool,
    todo_id: i64,
    depends_on_id: i64,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<TodoItem> {
    let mut tx = db::begin_write(pool).await?;
    let todo = repo::todos::get(&mut tx, todo_id).await?;
    let first = repo::todos::get(&mut tx, depends_on_id).await?;
    let existing = repo::dependencies::list(&mut tx).await?;
    check_new(&todo, &first, &existing).map_err(|e| AppError::validation(e.to_string()))?;
    repo::dependencies::insert(&mut tx, todo_id, depends_on_id, now).await?;
    let saved = repo::todos::touch(&mut tx, todo_id, now).await?;
    let out = item(&mut tx, saved, today).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn remove(
    pool: &SqlitePool,
    todo_id: i64,
    depends_on_id: i64,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<TodoItem> {
    let mut tx = db::begin_write(pool).await?;
    repo::todos::get(&mut tx, todo_id).await?;
    repo::dependencies::delete(&mut tx, todo_id, depends_on_id).await?;
    let saved = repo::todos::touch(&mut tx, todo_id, now).await?;
    let out = item(&mut tx, saved, today).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn list(pool: &SqlitePool, todo_id: i64, today: NaiveDate) -> CmdResult<DependencyList> {
    let mut conn = pool.acquire().await?;
    repo::todos::get(&mut conn, todo_id).await?;
    let (depends_on, dependents) = repo::dependencies::neighbours(&mut conn, todo_id).await?;
    let firsts = repo::todos::list_by_ids(&mut conn, &depends_on).await?;
    let laters = repo::todos::list_by_ids(&mut conn, &dependents).await?;
    Ok(DependencyList {
        depends_on: items(&mut conn, firsts, today).await?,
        dependents: items(&mut conn, laters, today).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::commands::projects::{self, ProjectInput};
    use crate::commands::todos::{self, NewTodoInput, TodoFields};
    use crate::domain::model::{Status, Todo};
    use crate::domain::status::StatusChange;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    fn today() -> NaiveDate {
        d("2026-11-19")
    }

    fn now() -> DateTime<Utc> {
        ts(2026, 11, 19, 1, 0)
    }

    async fn project(pool: &SqlitePool, name: &str) -> i64 {
        projects::create(
            pool,
            ProjectInput {
                name: name.into(),
                color: None,
                description: String::new(),
            },
            ts(2026, 11, 1, 0, 0),
        )
        .await
        .unwrap()
        .id
    }

    async fn add_todo(
        pool: &SqlitePool,
        project_id: i64,
        parent: Option<i64>,
        title: &str,
    ) -> Todo {
        todos::create(
            pool,
            NewTodoInput {
                project_id,
                parent_id: parent,
                template_id: None,
                title: title.into(),
                priority: None,
                due_date: None,
                planned_start: None,
                planned_end: None,
                memo_md: None,
            },
            today(),
            ts(2026, 11, 1, 0, 0),
        )
        .await
        .unwrap()
        .todo
    }

    async fn join(pool: &SqlitePool, later: &Todo, first: &Todo) -> CmdResult<TodoItem> {
        add(pool, later.id, first.id, today(), now()).await
    }

    fn message(r: CmdResult<TodoItem>) -> String {
        let e = r.unwrap_err();
        assert_eq!(e.kind, ErrorKind::Validation, "{e:?}");
        e.message
    }

    async fn status(pool: &SqlitePool, t: &Todo, to: Status, reason: Option<&str>) {
        todos::change_status_impl(
            pool,
            t.id,
            &StatusChange {
                to,
                waiting_on: None,
                follow_up_date: None,
                reason: reason.map(str::to_owned),
            },
            today(),
            now(),
        )
        .await
        .unwrap();
    }

    async fn pairs(pool: &SqlitePool) -> Vec<(i64, i64)> {
        let mut c = pool.acquire().await.unwrap();
        repo::dependencies::list(&mut c)
            .await
            .unwrap()
            .into_iter()
            .map(|d| (d.todo_id, d.depends_on_id))
            .collect()
    }

    #[tokio::test]
    async fn add_returns_the_later_todo_blocked_by_the_first() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "基幹システム更改").await;
        let a = add_todo(&pool, p, None, "要件の確定").await;
        let b = add_todo(&pool, p, None, "設計").await;
        let item = join(&pool, &b, &a).await.unwrap();
        assert_eq!(item.todo.id, b.id);
        assert_eq!(item.todo.updated_at, now());
        assert_eq!(
            item.blocked_by
                .iter()
                .map(|x| (x.id, x.title.as_str(), x.project_name.as_str()))
                .collect::<Vec<_>>(),
            [(a.id, "要件の確定", "基幹システム更改")]
        );
        assert_eq!(pairs(&pool).await, [(b.id, a.id)]);
        // The first TODO itself waits for nothing.
        let first = todos::get(&pool, a.id, today()).await.unwrap();
        assert!(first.blocked_by.is_empty());
        // Nothing is recorded in the history.
        let mut c = pool.acquire().await.unwrap();
        assert_eq!(
            repo::events::list_by_todo(&mut c, b.id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn itself_twice_and_circles_are_refused() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add_todo(&pool, p, None, "A").await;
        let b = add_todo(&pool, p, None, "B").await;
        let c = add_todo(&pool, p, None, "C").await;
        assert_eq!(
            message(join(&pool, &a, &a).await),
            "自分自身は、先行TODOにできません。ほかのTODOを選んでください。"
        );
        join(&pool, &b, &a).await.unwrap();
        assert_eq!(
            message(join(&pool, &b, &a).await),
            "「A」は、すでに先行TODOに入っています。"
        );
        // A→B→A.
        assert_eq!(
            message(join(&pool, &a, &b).await),
            "「B」は、このTODOの後続にあたるため、先行TODOにすると循環になります。ほかのTODOを選んでください。"
        );
        // A→B→C→A.
        join(&pool, &c, &b).await.unwrap();
        assert!(
            message(join(&pool, &a, &c).await).starts_with("「C」は、このTODOの後続にあたるため")
        );
        // Refused ones wrote nothing.
        assert_eq!(pairs(&pool).await, [(b.id, a.id), (c.id, b.id)]);
        // Missing TODOs.
        let missing = add(&pool, a.id, 999, today(), now()).await.unwrap_err();
        assert_eq!(missing.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn a_parent_and_its_own_child_cannot_be_joined_but_siblings_can() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let parent = add_todo(&pool, p, None, "親").await;
        let one = add_todo(&pool, p, Some(parent.id), "子1").await;
        let two = add_todo(&pool, p, Some(parent.id), "子2").await;
        let other = add_todo(&pool, p, None, "ほか").await;
        let refused =
            "親のTODOと、その子TODOの間には依存を結べません。ほかのTODOを選んでください。";
        assert_eq!(message(join(&pool, &one, &parent).await), refused);
        assert_eq!(message(join(&pool, &parent, &one).await), refused);
        join(&pool, &two, &one).await.unwrap();
        join(&pool, &other, &one).await.unwrap();
        join(&pool, &parent, &other).await.unwrap();
    }

    #[tokio::test]
    async fn projects_may_differ_and_closed_todos_may_be_joined() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let q = project(&pool, "Q").await;
        let a = add_todo(&pool, p, None, "A").await;
        let x = add_todo(&pool, q, None, "X").await;
        let done = add_todo(&pool, q, None, "済").await;
        status(&pool, &done, Status::Done, None).await;
        let item = join(&pool, &a, &x).await.unwrap();
        assert_eq!(item.blocked_by[0].project_name, "Q");
        join(&pool, &a, &done).await.unwrap();
        join(&pool, &done, &x).await.unwrap();
        // A closed TODO is blocked by nothing.
        let closed = todos::get(&pool, done.id, today()).await.unwrap();
        assert!(closed.blocked_by.is_empty());
    }

    #[tokio::test]
    async fn a_finished_first_todo_no_longer_blocks_and_cancelled_counts_as_finished() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add_todo(&pool, p, None, "A").await;
        let b = add_todo(&pool, p, None, "B").await;
        let later = add_todo(&pool, p, None, "後").await;
        join(&pool, &later, &a).await.unwrap();
        join(&pool, &later, &b).await.unwrap();
        let blocked = |pool: SqlitePool| async move {
            todos::get(&pool, later.id, today())
                .await
                .unwrap()
                .blocked_by
                .into_iter()
                .map(|x| x.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(blocked(pool.clone()).await, [a.id, b.id]);
        status(&pool, &a, Status::Done, None).await;
        assert_eq!(blocked(pool.clone()).await, [b.id]);
        status(&pool, &b, Status::Cancelled, Some("不要")).await;
        assert!(blocked(pool.clone()).await.is_empty());
        // Nothing stops the later TODO going 作業中, before or after.
        status(&pool, &later, Status::Doing, None).await;
    }

    #[tokio::test]
    async fn the_later_todo_may_go_doing_while_its_first_is_open() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add_todo(&pool, p, None, "A").await;
        let b = add_todo(&pool, p, None, "B").await;
        join(&pool, &b, &a).await.unwrap();
        status(&pool, &b, Status::Doing, None).await;
        let item = todos::get(&pool, b.id, today()).await.unwrap();
        assert_eq!(item.todo.status, Status::Doing);
        assert_eq!(item.blocked_by.len(), 1);
    }

    #[tokio::test]
    async fn starts_before_dependency_compares_the_plans() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add_todo(&pool, p, None, "A").await;
        let b = add_todo(&pool, p, None, "B").await;
        join(&pool, &b, &a).await.unwrap();
        let plan = |t: &Todo, start: &str, end: &str| TodoFields {
            title: t.title.clone(),
            priority: t.priority,
            due_date: None,
            planned_start: Some(d(start)),
            planned_end: Some(d(end)),
            actual_start: None,
            actual_end: None,
            waiting_on: None,
            follow_up_date: None,
            close_reason: None,
            memo_md: String::new(),
        };
        todos::update(
            &pool,
            a.id,
            plan(&a, "2026-11-16", "2026-11-20"),
            today(),
            now(),
        )
        .await
        .unwrap();
        let item = todos::update(
            &pool,
            b.id,
            plan(&b, "2026-11-19", "2026-11-25"),
            today(),
            now(),
        )
        .await
        .unwrap();
        assert!(item.starts_before_dependency);
        // The same day is fine.
        let item = todos::update(
            &pool,
            b.id,
            plan(&b, "2026-11-20", "2026-11-25"),
            today(),
            now(),
        )
        .await
        .unwrap();
        assert!(!item.starts_before_dependency);
    }

    #[tokio::test]
    async fn the_warning_goes_once_the_first_todo_is_finished() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add_todo(&pool, p, None, "A").await;
        let c = add_todo(&pool, p, None, "C").await;
        let b = add_todo(&pool, p, None, "B").await;
        let plan = |t: &Todo, start: &str, end: &str| TodoFields {
            title: t.title.clone(),
            priority: t.priority,
            due_date: None,
            planned_start: Some(d(start)),
            planned_end: Some(d(end)),
            actual_start: None,
            actual_end: None,
            waiting_on: None,
            follow_up_date: None,
            close_reason: None,
            memo_md: String::new(),
        };
        for (t, start, end) in [
            (&a, "2026-11-16", "2026-11-20"),
            (&c, "2026-11-16", "2026-11-22"),
            (&b, "2026-11-19", "2026-11-25"),
        ] {
            todos::update(&pool, t.id, plan(t, start, end), today(), now())
                .await
                .unwrap();
        }
        // b starts 11/19, before a (11/20) and c (11/22) are planned to end.
        join(&pool, &b, &a).await.unwrap();
        join(&pool, &b, &c).await.unwrap();
        let warned =
            |pool: SqlitePool| async move { todos::get(&pool, b.id, today()).await.unwrap() };
        assert!(warned(pool.clone()).await.starts_before_dependency);
        // a done: c still holds b up.
        status(&pool, &a, Status::Done, None).await;
        assert!(warned(pool.clone()).await.starts_before_dependency);
        // c done too: the warning goes, the plans stay as they were.
        status(&pool, &c, Status::Done, None).await;
        let item = warned(pool.clone()).await;
        assert!(!item.starts_before_dependency);
        assert!(item.blocked_by.is_empty());
        assert_eq!(item.todo.planned_start, Some(d("2026-11-19")));
        // c reopened (完了 → 作業中): unfinished again, so is the warning.
        status(&pool, &c, Status::Doing, None).await;
        assert!(warned(pool.clone()).await.starts_before_dependency);
    }

    #[tokio::test]
    async fn remove_and_list() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let q = project(&pool, "Q").await;
        let a = add_todo(&pool, p, None, "A").await;
        let b = add_todo(&pool, p, None, "B").await;
        let x = add_todo(&pool, q, None, "X").await;
        let c = add_todo(&pool, p, None, "C").await;
        // b waits for x, then a; c waits for b.
        join(&pool, &b, &x).await.unwrap();
        add(&pool, b.id, a.id, today(), ts(2026, 11, 19, 2, 0))
            .await
            .unwrap();
        add(&pool, c.id, b.id, today(), ts(2026, 11, 19, 3, 0))
            .await
            .unwrap();

        let around = list(&pool, b.id, today()).await.unwrap();
        let ids = |v: &[TodoItem]| v.iter().map(|i| i.todo.id).collect::<Vec<_>>();
        assert_eq!(ids(&around.depends_on), [x.id, a.id]);
        assert_eq!(ids(&around.dependents), [c.id]);
        assert_eq!(around.depends_on[0].project_name, "Q");
        // The items carry their own dependencies.
        assert_eq!(around.dependents[0].blocked_by[0].id, b.id);

        let item = remove(&pool, b.id, x.id, today(), ts(2026, 11, 19, 4, 0))
            .await
            .unwrap();
        assert_eq!(item.todo.updated_at, ts(2026, 11, 19, 4, 0));
        assert_eq!(
            ids(&list(&pool, b.id, today()).await.unwrap().depends_on),
            [a.id]
        );
        let gone = remove(&pool, b.id, x.id, today(), now()).await.unwrap_err();
        assert_eq!(gone.kind, ErrorKind::NotFound);
        assert_eq!(
            gone.message,
            "依存が見つかりません。外された可能性があります。画面を開き直してください。"
        );
        assert_eq!(
            list(&pool, 999, today()).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn todos_joined_by_a_dependency_cannot_become_parent_and_child() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "P").await;
        let a = add_todo(&pool, p, None, "A").await;
        let b = add_todo(&pool, p, None, "B").await;
        let c = add_todo(&pool, p, None, "C").await;
        join(&pool, &b, &a).await.unwrap();
        let refused = "依存で結ばれたTODOどうしは、親子にできません。先に詳細パネルの「先行TODO」か「後続TODO」から外してください。";
        for (child, parent) in [(&b, &a), (&a, &b)] {
            let e = todos::set_parent(&pool, child.id, Some(parent.id), today(), now())
                .await
                .unwrap_err();
            assert_eq!(e.message, refused);
        }
        // b and c both wait for a, not for each other: they may be parent
        // and child, or siblings.
        join(&pool, &c, &a).await.unwrap();
        todos::set_parent(&pool, b.id, Some(c.id), today(), now())
            .await
            .unwrap();
        let d_ = add_todo(&pool, p, None, "D").await;
        todos::set_parent(&pool, b.id, Some(d_.id), today(), now())
            .await
            .unwrap();
        todos::set_parent(&pool, c.id, Some(d_.id), today(), now())
            .await
            .unwrap();
    }
}
