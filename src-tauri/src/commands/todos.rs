//! TODO: list_todos, get_todo, create_todo, update_todo, change_status,
//! delete_todo.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::{SqliteConnection, SqlitePool};
use tauri::State;

use super::clock;
use super::error::{AppError, CmdResult};
use super::validate;
use crate::domain::events;
use crate::domain::listing::{DueFilter, TodoSort, due_matches, sort_todos};
use crate::domain::model::{Priority, Status, Todo};
use crate::domain::status::{StatusChange, StatusFields, transition};
use crate::domain::views::{StatusCounts, TodoItem, TodoList, todo_item};
use crate::repo;
use crate::repo::todos::{NewTodo, TodoFilter};

#[derive(Debug, Clone, Default, Deserialize, specta::Type)]
pub struct TodoQuery {
    /// Only this project (its TODOs are listed even when it is archived).
    /// `null` = all active projects.
    pub project_id: Option<i64>,
    /// Only these statuses. `null` = all.
    pub statuses: Option<Vec<Status>>,
    /// Only this priority. `null` = all.
    pub priority: Option<Priority>,
    /// 日限 filter. `null` = all.
    pub due: Option<DueFilter>,
    /// `null` = 登録順.
    pub sort: Option<TodoSort>,
}

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct NewTodoInput {
    pub project_id: i64,
    pub title: String,
    /// `null` = 中.
    pub priority: Option<Priority>,
    pub due_date: Option<NaiveDate>,
    pub planned_start: Option<NaiveDate>,
    pub planned_end: Option<NaiveDate>,
    /// `null` = empty memo.
    pub memo_md: Option<String>,
}

/// Every editable field of a TODO (the detail panel sends them all). The
/// status and the automatic fields change only through `change_status`.
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct TodoFields {
    pub title: String,
    pub priority: Priority,
    pub due_date: Option<NaiveDate>,
    pub planned_start: Option<NaiveDate>,
    pub planned_end: Option<NaiveDate>,
    pub actual_start: Option<NaiveDate>,
    pub actual_end: Option<NaiveDate>,
    /// Used only while 待ち (required then); ignored otherwise.
    pub waiting_on: Option<String>,
    /// Used only while 待ち (required then); ignored otherwise.
    pub follow_up_date: Option<NaiveDate>,
    /// Used only while 中止 (required then); ignored otherwise.
    pub close_reason: Option<String>,
    pub memo_md: String,
}

// ---- tauri commands ----------------------------------------------------------

/// TODOs with their 日限注意 category, warnings and day counts.
#[tauri::command]
#[specta::specta]
pub async fn list_todos(pool: State<'_, SqlitePool>, query: TodoQuery) -> CmdResult<TodoList> {
    list(&pool, &query, clock::today()).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_todo(pool: State<'_, SqlitePool>, id: i64) -> CmdResult<TodoItem> {
    get(&pool, id, clock::today()).await
}

/// Adds a TODO as 未着手 (also used by the quick add, T-5).
#[tauri::command]
#[specta::specta]
pub async fn create_todo(pool: State<'_, SqlitePool>, input: NewTodoInput) -> CmdResult<TodoItem> {
    create(&pool, input, clock::today(), clock::now()).await
}

/// Saves the editable fields and records changes of 日限・予定・実績・確認日.
#[tauri::command]
#[specta::specta]
pub async fn update_todo(
    pool: State<'_, SqlitePool>,
    id: i64,
    fields: TodoFields,
) -> CmdResult<TodoItem> {
    update(&pool, id, fields, clock::today(), clock::now()).await
}

/// Changes the status, fills the automatic dates and records the change, in
/// one transaction.
#[tauri::command]
#[specta::specta]
pub async fn change_status(
    pool: State<'_, SqlitePool>,
    id: i64,
    change: StatusChange,
) -> CmdResult<TodoItem> {
    change_status_impl(&pool, id, &change, clock::today(), clock::now()).await
}

/// Deletes a TODO with its work logs and history. The screen confirms first.
#[tauri::command]
#[specta::specta]
pub async fn delete_todo(pool: State<'_, SqlitePool>, id: i64) -> CmdResult<()> {
    delete(&pool, id).await
}

// ---- implementation ----------------------------------------------------------

async fn soon_days(conn: &mut SqliteConnection) -> CmdResult<u32> {
    Ok(repo::settings::load(conn).await?.soon_days)
}

async fn item(conn: &mut SqliteConnection, todo: Todo, today: NaiveDate) -> CmdResult<TodoItem> {
    let project = repo::projects::get(conn, todo.project_id).await?;
    let soon = soon_days(conn).await?;
    Ok(todo_item(todo, project.name, today, soon))
}

pub async fn list(pool: &SqlitePool, query: &TodoQuery, today: NaiveDate) -> CmdResult<TodoList> {
    let mut conn = pool.acquire().await?;
    let soon = soon_days(&mut conn).await?;
    let names: HashMap<i64, String> = repo::projects::list(&mut conn, true)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let due = query.due.unwrap_or_default();
    let todos: Vec<Todo> = repo::todos::list(
        &mut conn,
        &TodoFilter {
            project_id: query.project_id,
            statuses: None,
            include_archived_projects: query.project_id.is_some(),
        },
    )
    .await?
    .into_iter()
    .filter(|t| query.priority.is_none_or(|p| t.priority == p))
    .filter(|t| due_matches(t, due, today, soon))
    .collect();

    // Tab counts: every filter except the status one.
    let mut counts = StatusCounts::default();
    for t in &todos {
        counts.add(t.status);
    }
    let mut items: Vec<TodoItem> = todos
        .into_iter()
        .filter(|t| {
            query
                .statuses
                .as_ref()
                .is_none_or(|s| s.contains(&t.status))
        })
        .map(|t| {
            let name = names.get(&t.project_id).cloned().unwrap_or_default();
            todo_item(t, name, today, soon)
        })
        .collect();
    sort_todos(&mut items, query.sort.unwrap_or_default(), |i| &i.todo);
    Ok(TodoList {
        items,
        total: counts.total(),
        counts,
    })
}

pub async fn get(pool: &SqlitePool, id: i64, today: NaiveDate) -> CmdResult<TodoItem> {
    let mut conn = pool.acquire().await?;
    let todo = repo::todos::get(&mut conn, id).await?;
    item(&mut conn, todo, today).await
}

pub async fn create(
    pool: &SqlitePool,
    input: NewTodoInput,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<TodoItem> {
    let title = validate::required_text(&input.title, "タイトル")?;
    validate::period(input.planned_start, input.planned_end, "予定")?;

    let mut tx = pool.begin().await?;
    let project = repo::projects::get(&mut tx, input.project_id).await?;
    if project.archived_at.is_some() {
        return Err(AppError::validation(
            "アーカイブしたプロジェクトにはTODOを追加できません。プロジェクトを元に戻してから追加してください。",
        ));
    }
    let todo = repo::todos::insert(
        &mut tx,
        &NewTodo {
            project_id: project.id,
            title,
            priority: input.priority.unwrap_or_default(),
            due_date: input.due_date,
            planned_start: input.planned_start,
            planned_end: input.planned_end,
            memo_md: input.memo_md.unwrap_or_default(),
        },
        now,
    )
    .await?;
    repo::events::insert(&mut tx, todo.id, &events::created(todo.status, today, now)).await?;
    let out = item(&mut tx, todo, today).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn update(
    pool: &SqlitePool,
    id: i64,
    fields: TodoFields,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<TodoItem> {
    let title = validate::required_text(&fields.title, "タイトル")?;
    validate::period(fields.planned_start, fields.planned_end, "予定")?;
    validate::period(fields.actual_start, fields.actual_end, "実績")?;

    let mut tx = pool.begin().await?;
    let before = repo::todos::get(&mut tx, id).await?;
    let mut after = Todo {
        title,
        priority: fields.priority,
        due_date: fields.due_date,
        planned_start: fields.planned_start,
        planned_end: fields.planned_end,
        actual_start: fields.actual_start,
        actual_end: fields.actual_end,
        memo_md: fields.memo_md,
        ..before.clone()
    };
    if before.status == Status::Waiting {
        after.waiting_on = Some(validate::required_opt_text(
            fields.waiting_on.as_deref(),
            "待っている相手",
        )?);
        after.follow_up_date = Some(
            fields
                .follow_up_date
                .ok_or_else(|| AppError::validation("確認日を入力してください。"))?,
        );
    }
    if before.status == Status::Cancelled {
        after.close_reason = Some(validate::required_opt_text(
            fields.close_reason.as_deref(),
            "中止の理由",
        )?);
    }

    let saved = repo::todos::update_fields(&mut tx, &after, now).await?;
    for event in events::edit_events(&before, &saved, today, now) {
        repo::events::insert(&mut tx, id, &event).await?;
    }
    let out = item(&mut tx, saved, today).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn change_status_impl(
    pool: &SqlitePool,
    id: i64,
    change: &StatusChange,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<TodoItem> {
    let mut tx = pool.begin().await?;
    let before = repo::todos::get(&mut tx, id).await?;
    let t = transition(&StatusFields::of(&before), change, today, now)?;
    let saved = repo::todos::write_status(&mut tx, id, &t.fields, now).await?;
    repo::events::insert(&mut tx, id, &t.event).await?;
    let out = item(&mut tx, saved, today).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> CmdResult<()> {
    let mut conn = pool.acquire().await?;
    Ok(repo::todos::delete(&mut conn, id).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::commands::projects::{self, ProjectInput};
    use crate::domain::events::EventKind;
    use crate::domain::testing::{d, ts};
    use crate::domain::urgency::Urgency;
    use crate::repo::db;

    fn today() -> NaiveDate {
        d("2026-11-19")
    }

    fn now() -> DateTime<Utc> {
        ts(2026, 11, 19, 1, 0)
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

    fn new_input(project_id: i64, title: &str) -> NewTodoInput {
        NewTodoInput {
            project_id,
            title: title.into(),
            priority: None,
            due_date: None,
            planned_start: None,
            planned_end: None,
            memo_md: None,
        }
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

    async fn event_kinds(pool: &SqlitePool, id: i64) -> Vec<EventKind> {
        let mut c = pool.acquire().await.unwrap();
        repo::events::list_by_todo(&mut c, id)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.kind)
            .collect()
    }

    #[tokio::test]
    async fn create_records_the_created_event_and_classifies() {
        let (pool, pid) = setup().await;
        let item = create(
            &pool,
            NewTodoInput {
                due_date: Some(d("2026-11-19")),
                ..new_input(pid, "  移行手順書のドラフト ")
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(item.todo.title, "移行手順書のドラフト");
        assert_eq!(item.todo.status, Status::Todo);
        assert_eq!(item.todo.priority, Priority::Medium);
        assert_eq!(item.project_name, "基幹システム更改");
        assert_eq!(item.assessment.urgency, Some(Urgency::DueToday));
        assert_eq!(item.assessment.days_until_due, Some(0));
        assert_eq!(
            event_kinds(&pool, item.todo.id).await,
            vec![EventKind::Created]
        );
    }

    #[tokio::test]
    async fn create_rejects_bad_input() {
        let (pool, pid) = setup().await;
        let e = create(&pool, new_input(pid, " "), today(), now())
            .await
            .unwrap_err();
        assert_eq!(e.message, "タイトルを入力してください。");

        let reversed = NewTodoInput {
            planned_start: Some(d("2026-11-21")),
            planned_end: Some(d("2026-11-20")),
            ..new_input(pid, "逆転")
        };
        assert_eq!(
            create(&pool, reversed, today(), now())
                .await
                .unwrap_err()
                .message,
            "予定開始日が予定終了日より後になっています。日付を直してください。"
        );

        let e = create(&pool, new_input(999, "x"), today(), now())
            .await
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound);

        projects::archive(&pool, pid, true, now()).await.unwrap();
        let e = create(&pool, new_input(pid, "x"), today(), now())
            .await
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Validation);
    }

    #[tokio::test]
    async fn update_records_date_changes_only() {
        let (pool, pid) = setup().await;
        let item = create(&pool, new_input(pid, "t"), today(), now())
            .await
            .unwrap();
        let mut f = fields_of(&item.todo);
        f.title = "新しいタイトル".into();
        f.memo_md = "- メモ".into();
        let u = update(&pool, item.todo.id, f.clone(), today(), now())
            .await
            .unwrap();
        assert_eq!(u.todo.title, "新しいタイトル");
        assert_eq!(
            event_kinds(&pool, item.todo.id).await,
            vec![EventKind::Created]
        );

        f.due_date = Some(d("2026-11-20"));
        f.planned_start = Some(d("2026-11-16"));
        f.planned_end = Some(d("2026-11-21"));
        let u = update(&pool, item.todo.id, f, today(), now())
            .await
            .unwrap();
        assert!(u.assessment.warnings.plan_misses_due);
        assert_eq!(
            event_kinds(&pool, item.todo.id).await,
            vec![EventKind::Created, EventKind::DueDate, EventKind::Planned]
        );
    }

    #[tokio::test]
    async fn update_ignores_waiting_fields_unless_waiting() {
        let (pool, pid) = setup().await;
        let item = create(&pool, new_input(pid, "t"), today(), now())
            .await
            .unwrap();
        let mut f = fields_of(&item.todo);
        f.waiting_on = Some("誰か".into());
        f.follow_up_date = Some(d("2026-11-25"));
        f.close_reason = Some("理由".into());
        let u = update(&pool, item.todo.id, f, today(), now())
            .await
            .unwrap();
        assert_eq!(u.todo.waiting_on, None);
        assert_eq!(u.todo.follow_up_date, None);
        assert_eq!(u.todo.close_reason, None);
    }

    #[tokio::test]
    async fn update_while_waiting_requires_both_waiting_fields() {
        let (pool, pid) = setup().await;
        let item = create(&pool, new_input(pid, "t"), today(), now())
            .await
            .unwrap();
        let w = change_status_impl(
            &pool,
            item.todo.id,
            &StatusChange::waiting("認証担当の回答", d("2026-11-20")),
            today(),
            now(),
        )
        .await
        .unwrap();

        // Pushing the follow-up date out records a follow_up event.
        let mut f = fields_of(&w.todo);
        f.follow_up_date = Some(d("2026-11-24"));
        let u = update(&pool, item.todo.id, f.clone(), today(), now())
            .await
            .unwrap();
        assert_eq!(u.todo.follow_up_date, Some(d("2026-11-24")));
        assert_eq!(
            event_kinds(&pool, item.todo.id).await.last(),
            Some(&EventKind::FollowUp)
        );

        f.waiting_on = Some("  ".into());
        assert_eq!(
            update(&pool, item.todo.id, f.clone(), today(), now())
                .await
                .unwrap_err()
                .message,
            "待っている相手を入力してください。"
        );
        f.waiting_on = Some("認証担当".into());
        f.follow_up_date = None;
        assert_eq!(
            update(&pool, item.todo.id, f, today(), now())
                .await
                .unwrap_err()
                .message,
            "確認日を入力してください。"
        );
    }

    #[tokio::test]
    async fn update_rejects_reversed_actual_period() {
        let (pool, pid) = setup().await;
        let item = create(&pool, new_input(pid, "t"), today(), now())
            .await
            .unwrap();
        let mut f = fields_of(&item.todo);
        f.actual_start = Some(d("2026-11-20"));
        f.actual_end = Some(d("2026-11-19"));
        assert_eq!(
            update(&pool, item.todo.id, f, today(), now())
                .await
                .unwrap_err()
                .kind,
            ErrorKind::Validation
        );
    }

    #[tokio::test]
    async fn change_status_writes_fields_and_one_event() {
        let (pool, pid) = setup().await;
        let item = create(&pool, new_input(pid, "t"), today(), now())
            .await
            .unwrap();
        let id = item.todo.id;

        let doing = change_status_impl(&pool, id, &StatusChange::to(Status::Doing), today(), now())
            .await
            .unwrap();
        assert_eq!(doing.todo.actual_start, Some(today()));

        let done = change_status_impl(&pool, id, &StatusChange::to(Status::Done), today(), now())
            .await
            .unwrap();
        assert_eq!(done.todo.actual_end, Some(today()));
        assert_eq!(done.todo.closed_at, Some(now()));
        assert_eq!(done.assessment.urgency, None);

        let e = change_status_impl(&pool, id, &StatusChange::cancel("不要"), today(), now())
            .await
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Validation);
        assert!(e.message.contains("直接変えられません"), "{}", e.message);

        assert_eq!(
            event_kinds(&pool, id).await,
            vec![EventKind::Created, EventKind::Status, EventKind::Status]
        );
    }

    #[tokio::test]
    async fn failed_status_change_writes_nothing() {
        let (pool, pid) = setup().await;
        let item = create(&pool, new_input(pid, "t"), today(), now())
            .await
            .unwrap();
        let change = StatusChange {
            waiting_on: None,
            ..StatusChange::waiting("", d("2026-11-20"))
        };
        assert!(
            change_status_impl(&pool, item.todo.id, &change, today(), now())
                .await
                .is_err()
        );
        let after = get(&pool, item.todo.id, today()).await.unwrap();
        assert_eq!(after.todo, item.todo);
        assert_eq!(
            event_kinds(&pool, item.todo.id).await,
            vec![EventKind::Created]
        );
    }

    #[tokio::test]
    async fn list_filters_and_shows_archived_project_only_when_asked() {
        let (pool, pid) = setup().await;
        let a = create(&pool, new_input(pid, "a"), today(), now())
            .await
            .unwrap();
        create(&pool, new_input(pid, "b"), today(), now())
            .await
            .unwrap();
        change_status_impl(
            &pool,
            a.todo.id,
            &StatusChange::to(Status::Doing),
            today(),
            now(),
        )
        .await
        .unwrap();

        let all = list(&pool, &TodoQuery::default(), today()).await.unwrap();
        assert_eq!(all.items.len(), 2);
        assert_eq!(all.total, 2);
        let doing = list(
            &pool,
            &TodoQuery {
                statuses: Some(vec![Status::Doing]),
                ..TodoQuery::default()
            },
            today(),
        )
        .await
        .unwrap();
        assert_eq!(doing.items.len(), 1);
        assert_eq!(doing.items[0].todo.title, "a");
        // Tab counts ignore the status filter.
        assert_eq!(
            (doing.total, doing.counts.doing, doing.counts.todo),
            (2, 1, 1)
        );

        projects::archive(&pool, pid, true, now()).await.unwrap();
        assert!(
            list(&pool, &TodoQuery::default(), today())
                .await
                .unwrap()
                .items
                .is_empty()
        );
        let in_project = TodoQuery {
            project_id: Some(pid),
            ..TodoQuery::default()
        };
        assert_eq!(
            list(&pool, &in_project, today()).await.unwrap().items.len(),
            2
        );
    }

    #[tokio::test]
    async fn list_filters_by_due_and_priority_and_sorts() {
        let (pool, pid) = setup().await;
        let mk = |title: &str, due: Option<&str>, priority: Priority| NewTodoInput {
            priority: Some(priority),
            due_date: due.map(d),
            ..new_input(pid, title)
        };
        for input in [
            mk("期限なし", None, Priority::High),
            mk("来月", Some("2026-12-20"), Priority::Low),
            mk("明日", Some("2026-11-20"), Priority::Medium),
            mk("昨日", Some("2026-11-18"), Priority::Low),
        ] {
            create(&pool, input, today(), now()).await.unwrap();
        }
        let titles = |l: TodoList| {
            l.items
                .into_iter()
                .map(|i| i.todo.title)
                .collect::<Vec<_>>()
        };
        let q = |due, priority, sort| TodoQuery {
            due,
            priority,
            sort,
            ..TodoQuery::default()
        };

        let attention = list(&pool, &q(Some(DueFilter::Attention), None, None), today())
            .await
            .unwrap();
        assert_eq!(attention.total, 2);
        assert_eq!(titles(attention), vec!["明日", "昨日"]);

        let low = list(&pool, &q(None, Some(Priority::Low), None), today())
            .await
            .unwrap();
        assert_eq!(titles(low), vec!["来月", "昨日"]);

        let none = list(&pool, &q(Some(DueFilter::NoDue), None, None), today())
            .await
            .unwrap();
        assert_eq!(titles(none), vec!["期限なし"]);

        let by_due = list(&pool, &q(None, None, Some(TodoSort::DueDate)), today())
            .await
            .unwrap();
        assert_eq!(titles(by_due), vec!["昨日", "明日", "来月", "期限なし"]);

        let by_priority = list(&pool, &q(None, None, Some(TodoSort::Priority)), today())
            .await
            .unwrap();
        assert_eq!(
            titles(by_priority),
            vec!["期限なし", "明日", "昨日", "来月"]
        );
    }

    #[tokio::test]
    async fn delete_and_not_found() {
        let (pool, pid) = setup().await;
        let item = create(&pool, new_input(pid, "t"), today(), now())
            .await
            .unwrap();
        delete(&pool, item.todo.id).await.unwrap();
        assert_eq!(
            get(&pool, item.todo.id, today()).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
        assert_eq!(
            delete(&pool, item.todo.id).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
    }
}
