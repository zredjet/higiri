//! TODO: list_todos, get_todo, create_todo, update_todo, change_status,
//! delete_todo, set_todo_parent (v0.3.0, S-1).

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::{SqliteConnection, SqlitePool};
use tauri::State;

use super::clock;
use super::error::{AppError, CmdResult};
use super::validate;
use crate::domain::events;
use crate::domain::listing::{DueFilter, TodoSort, due_matches, sort_todos, tree_rows};
use crate::domain::model::{Priority, Status, Todo};
use crate::domain::status::{StatusChange, StatusFields, transition};
use crate::domain::subtodo::{Family, check_new_child, check_parent};
use crate::domain::urgency::Rules;
use crate::domain::views::{StatusCounts, TodoItem, TodoList, TodoListRow, todo_item};
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
    /// Adds it as a child of this TODO (S-1), which must be in the same
    /// project and have no parent. `null` = a TODO of its own.
    pub parent_id: Option<i64>,
    /// Makes it from this template (T-10): it gets the template's memo and
    /// children, each child due `due_date` + its days (no due date when
    /// `due_date` is empty) and 未着手. Not with `parent_id`. The title and
    /// priority are the input's (the screen fills them in from the template).
    pub template_id: Option<i64>,
    pub title: String,
    /// `null` = 中.
    pub priority: Option<Priority>,
    pub due_date: Option<NaiveDate>,
    pub planned_start: Option<NaiveDate>,
    pub planned_end: Option<NaiveDate>,
    /// `null` = empty memo (the template's memo, with `template_id`).
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

/// Adds a TODO as 未着手 (also used by the quick add, T-5), with its
/// children when it is made from a template (T-10), in one transaction.
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

/// Deletes a TODO with its work logs and history, and its children (S-1).
/// The screen confirms first.
#[tauri::command]
#[specta::specta]
pub async fn delete_todo(pool: State<'_, SqlitePool>, id: i64) -> CmdResult<()> {
    delete(&pool, id).await
}

/// Puts a TODO under a parent, moves it to another one, or (`null`) takes it
/// out (S-1). Same project, one level. Not recorded in the history.
#[tauri::command]
#[specta::specta]
pub async fn set_todo_parent(
    pool: State<'_, SqlitePool>,
    id: i64,
    parent_id: Option<i64>,
) -> CmdResult<TodoItem> {
    set_parent(&pool, id, parent_id, clock::today(), clock::now()).await
}

// ---- implementation ----------------------------------------------------------

async fn rules(conn: &mut SqliteConnection) -> CmdResult<Rules> {
    Ok(Rules::from_settings(&repo::settings::load(conn).await?))
}

/// One TODO with its parent's title and its children counts, read from the
/// database (its family is not at hand).
async fn item(conn: &mut SqliteConnection, todo: Todo, today: NaiveDate) -> CmdResult<TodoItem> {
    let project = repo::projects::get(conn, todo.project_id).await?;
    let rules = rules(conn).await?;
    let parent = match todo.parent_id {
        Some(p) => repo::todos::list_by_ids(conn, &[p]).await?,
        None => Vec::new(),
    };
    let children = repo::todos::list_children(conn, &[todo.id]).await?;
    let family = Family::new(parent.iter().chain(&children).chain([&todo]));
    Ok(todo_item(todo, project.name, &family, today, &rules))
}

pub async fn list(pool: &SqlitePool, query: &TodoQuery, today: NaiveDate) -> CmdResult<TodoList> {
    let mut conn = pool.acquire().await?;
    let rules = rules(&mut conn).await?;
    let names: HashMap<i64, String> = repo::projects::list(&mut conn, true)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let due = query.due.unwrap_or_default();
    let mut todos: Vec<Todo> = repo::todos::list(
        &mut conn,
        &TodoFilter {
            project_id: query.project_id,
            statuses: None,
            include_archived_projects: query.project_id.is_some(),
        },
    )
    .await?;
    // Every TODO of the list's projects: the counts of children hidden by
    // the filters are still counted.
    let family = Family::new(&todos);

    // Each TODO is judged by itself (S-1). Tab counts: every filter except
    // the status one.
    let matches_filters = |t: &Todo| {
        query.priority.is_none_or(|p| t.priority == p) && due_matches(t, due, today, &rules)
    };
    let matches_status = |t: &Todo| {
        query
            .statuses
            .as_ref()
            .is_none_or(|s| s.contains(&t.status))
    };
    let mut counts = StatusCounts::default();
    for t in todos.iter().filter(|t| matches_filters(t)) {
        counts.add(t.status);
    }

    sort_todos(&mut todos, query.sort.unwrap_or_default(), |t| t);
    let tree = tree_rows(&todos, |t| t, |t| matches_filters(t) && matches_status(t));
    let mut slots: Vec<Option<Todo>> = todos.into_iter().map(Some).collect();
    let rows = tree
        .into_iter()
        .filter_map(|r| {
            let t = slots[r.index].take()?;
            let name = names.get(&t.project_id).cloned().unwrap_or_default();
            Some(TodoListRow {
                item: todo_item(t, name, &family, today, &rules),
                child: r.child,
                bridge: r.bridge,
            })
        })
        .collect();
    Ok(TodoList {
        rows,
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
    if input.parent_id.is_some() && input.template_id.is_some() {
        // A template has children of its own: one level only.
        return Err(AppError::validation(
            "テンプレートから作るTODOは、子TODOにできません(親子は1段だけです)。親のTODOを選ばずに追加してください。",
        ));
    }

    let mut tx = pool.begin().await?;
    let project = repo::projects::get(&mut tx, input.project_id).await?;
    if project.archived_at.is_some() {
        return Err(AppError::validation(
            "アーカイブしたプロジェクトにはTODOを追加できません。プロジェクトを元に戻してから追加してください。",
        ));
    }
    if let Some(parent_id) = input.parent_id {
        let parent = repo::todos::get(&mut tx, parent_id).await?;
        check_new_child(project.id, &parent).map_err(|e| AppError::validation(e.to_string()))?;
    }
    let template = match input.template_id {
        Some(id) => Some(repo::templates::get(&mut tx, id).await?.body),
        None => None,
    };
    let memo_md = input
        .memo_md
        .or_else(|| template.as_ref().map(|b| b.memo_md.clone()))
        .unwrap_or_default();
    let todo = repo::todos::insert(
        &mut tx,
        &NewTodo {
            project_id: project.id,
            parent_id: input.parent_id,
            title,
            priority: input.priority.unwrap_or_default(),
            due_date: input.due_date,
            planned_start: input.planned_start,
            planned_end: input.planned_end,
            memo_md,
        },
        now,
    )
    .await?;
    repo::events::insert(&mut tx, todo.id, &events::created(todo.status, today, now)).await?;
    // The template's children, in its order, each 未着手 with its own 「作成」.
    for child in template.iter().flat_map(|b| b.new_children(input.due_date)) {
        let made = repo::todos::insert(
            &mut tx,
            &NewTodo {
                project_id: project.id,
                parent_id: Some(todo.id),
                title: child.title,
                priority: child.priority,
                due_date: child.due_date,
                planned_start: None,
                planned_end: None,
                memo_md: child.memo_md,
            },
            now,
        )
        .await?;
        repo::events::insert(&mut tx, made.id, &events::created(made.status, today, now)).await?;
    }
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
    if saved.status.is_closed() {
        close_children(&mut tx, id, change, today, now).await?;
    }
    let out = item(&mut tx, saved, today).await?;
    tx.commit().await?;
    Ok(out)
}

/// Closing a parent closes its open children the same way (S-1): each one
/// goes through the transition table and records its own change. A 中止
/// carries the parent's reason. Closed children stay as they are.
async fn close_children(
    conn: &mut SqliteConnection,
    parent_id: i64,
    change: &StatusChange,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<()> {
    let same = StatusChange {
        to: change.to,
        waiting_on: None,
        follow_up_date: None,
        reason: change.reason.clone(),
    };
    for child in repo::todos::list_children(conn, &[parent_id]).await? {
        if child.status.is_closed() {
            continue;
        }
        let t = transition(&StatusFields::of(&child), &same, today, now).map_err(|e| {
            AppError::validation(format!(
                "子TODO「{}」を{}にできません。{e}",
                child.title,
                change.to.label()
            ))
        })?;
        repo::todos::write_status(conn, child.id, &t.fields, now).await?;
        repo::events::insert(conn, child.id, &t.event).await?;
    }
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> CmdResult<()> {
    let mut conn = pool.acquire().await?;
    Ok(repo::todos::delete(&mut conn, id).await?)
}

pub async fn set_parent(
    pool: &SqlitePool,
    id: i64,
    parent_id: Option<i64>,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> CmdResult<TodoItem> {
    let mut tx = pool.begin().await?;
    let todo = repo::todos::get(&mut tx, id).await?;
    if let Some(parent_id) = parent_id {
        let parent = repo::todos::get(&mut tx, parent_id).await?;
        let has_children = !repo::todos::list_children(&mut tx, &[id]).await?.is_empty();
        check_parent(&todo, has_children, &parent)
            .map_err(|e| AppError::validation(e.to_string()))?;
    }
    let saved = repo::todos::set_parent(&mut tx, id, parent_id, now).await?;
    let out = item(&mut tx, saved, today).await?;
    tx.commit().await?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::commands::projects::{self, ProjectInput};
    use crate::domain::events::EventKind;
    use crate::domain::subtodo::ChildCounts;
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
            parent_id: None,
            template_id: None,
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
        assert_eq!(all.rows.len(), 2);
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
        assert_eq!(doing.rows.len(), 1);
        assert_eq!(doing.rows[0].item.todo.title, "a");
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
                .rows
                .is_empty()
        );
        let in_project = TodoQuery {
            project_id: Some(pid),
            ..TodoQuery::default()
        };
        assert_eq!(
            list(&pool, &in_project, today()).await.unwrap().rows.len(),
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
            l.rows
                .into_iter()
                .map(|r| r.item.todo.title)
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

    // ---- サブTODO (S-1) ----------------------------------------------------

    async fn add(pool: &SqlitePool, pid: i64, parent: Option<i64>, title: &str) -> TodoItem {
        create(
            pool,
            NewTodoInput {
                parent_id: parent,
                ..new_input(pid, title)
            },
            today(),
            now(),
        )
        .await
        .unwrap()
    }

    async fn other_project(pool: &SqlitePool) -> i64 {
        projects::create(
            pool,
            ProjectInput {
                name: "社内ツール整備".into(),
                color: None,
                description: String::new(),
            },
            now(),
        )
        .await
        .unwrap()
        .id
    }

    #[tokio::test]
    async fn create_adds_a_child_in_the_parents_project_only() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, pid, None, "親").await;
        let child = add(&pool, pid, Some(parent.todo.id), "子").await;
        assert_eq!(child.todo.parent_id, Some(parent.todo.id));
        assert_eq!(child.parent_title.as_deref(), Some("親"));
        assert_eq!(
            get(&pool, parent.todo.id, today()).await.unwrap().children,
            Some(ChildCounts {
                total: 1,
                done: 0,
                open: 1
            })
        );

        let qid = other_project(&pool).await;
        let try_add = |project_id: i64, parent: i64| {
            create(
                &pool,
                NewTodoInput {
                    parent_id: Some(parent),
                    ..new_input(project_id, "x")
                },
                today(),
                now(),
            )
        };
        let e = try_add(qid, parent.todo.id).await.unwrap_err();
        assert_eq!(
            e.message,
            "別のプロジェクトのTODOは親にできません。同じプロジェクトのTODOを選んでください。"
        );
        // A grandchild.
        let e = try_add(pid, child.todo.id).await.unwrap_err();
        assert_eq!(
            e.message,
            "子TODOは親にできません(親子は1段だけです)。親のないTODOを選んでください。"
        );
        assert_eq!(
            try_add(pid, 999).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn set_parent_checks_the_family_rules() {
        let (pool, pid) = setup().await;
        let a = add(&pool, pid, None, "A").await.todo.id;
        let b = add(&pool, pid, None, "B").await.todo.id;
        let c = add(&pool, pid, Some(a), "C").await.todo.id;
        let qid = other_project(&pool).await;
        let x = add(&pool, qid, None, "X").await.todo.id;
        let message = |r: CmdResult<TodoItem>| r.map(|i| i.todo.id).map_err(|e| e.message);

        assert_eq!(
            message(set_parent(&pool, b, Some(b), today(), now()).await),
            Err("自分自身を親にはできません。ほかのTODOを親に選んでください。".into())
        );
        assert_eq!(
            message(set_parent(&pool, b, Some(x), today(), now()).await),
            Err(
                "別のプロジェクトのTODOは親にできません。同じプロジェクトのTODOを選んでください。"
                    .into()
            )
        );
        // C is a child: it cannot be a parent.
        assert_eq!(
            message(set_parent(&pool, b, Some(c), today(), now()).await),
            Err("子TODOは親にできません(親子は1段だけです)。親のないTODOを選んでください。".into())
        );
        // A has a child: it cannot become a child.
        assert_eq!(
            message(set_parent(&pool, a, Some(b), today(), now()).await),
            Err("子TODOのあるTODOは、ほかのTODOの子にできません(親子は1段だけです)。先に子TODOを親から外してください。".into())
        );
        assert_eq!(
            set_parent(&pool, b, Some(999), today(), now())
                .await
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );

        // Move C from A to B, then take it out. No history is recorded.
        let moved = set_parent(&pool, c, Some(b), today(), now()).await.unwrap();
        assert_eq!(moved.todo.parent_id, Some(b));
        assert_eq!(moved.parent_title.as_deref(), Some("B"));
        assert_eq!(get(&pool, a, today()).await.unwrap().children, None);
        let out = set_parent(&pool, c, None, today(), now()).await.unwrap();
        assert_eq!(out.todo.parent_id, None);
        assert_eq!(out.parent_title, None);
        assert_eq!(event_kinds(&pool, c).await, vec![EventKind::Created]);
        // A TODO with no children (A, now) may become a child.
        assert_eq!(
            set_parent(&pool, a, Some(b), today(), now())
                .await
                .unwrap()
                .todo
                .parent_id,
            Some(b)
        );
    }

    /// The status changes of a TODO: (old, new, reason).
    async fn status_events(
        pool: &SqlitePool,
        id: i64,
    ) -> Vec<(Option<String>, Option<String>, Option<String>)> {
        let mut c = pool.acquire().await.unwrap();
        repo::events::list_by_todo(&mut c, id)
            .await
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == EventKind::Status)
            .map(|e| (e.old_value, e.new_value, e.note.reason))
            .collect()
    }

    fn s(v: &str) -> Option<String> {
        Some(v.to_owned())
    }

    #[tokio::test]
    async fn closing_a_parent_closes_its_open_children() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, pid, None, "親").await.todo.id;
        let open = add(&pool, pid, Some(parent), "開いている子").await.todo.id;
        let waiting = add(&pool, pid, Some(parent), "待ちの子").await.todo.id;
        let done = add(&pool, pid, Some(parent), "完了した子").await.todo.id;
        let other = add(&pool, pid, None, "ほかのTODO").await.todo.id;
        change_status_impl(
            &pool,
            waiting,
            &StatusChange::waiting("相手", d("2026-11-20")),
            today(),
            now(),
        )
        .await
        .unwrap();
        change_status_impl(&pool, done, &StatusChange::to(Status::Done), today(), now())
            .await
            .unwrap();
        let before_done = get(&pool, done, today()).await.unwrap();

        let out = change_status_impl(
            &pool,
            parent,
            &StatusChange::to(Status::Done),
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(out.todo.status, Status::Done);
        assert_eq!(
            out.children,
            Some(ChildCounts {
                total: 3,
                done: 3,
                open: 0
            })
        );
        for id in [open, waiting] {
            let c = get(&pool, id, today()).await.unwrap();
            assert_eq!(c.todo.status, Status::Done);
            assert_eq!(c.todo.closed_at, Some(now()));
        }
        // One status row per child, from where each one was.
        assert_eq!(
            status_events(&pool, open).await,
            vec![(s("todo"), s("done"), None)]
        );
        assert_eq!(
            status_events(&pool, waiting).await.last().cloned(),
            Some((s("waiting"), s("done"), None))
        );
        assert_eq!(
            get(&pool, waiting, today()).await.unwrap().todo.waiting_on,
            None
        );
        // A closed child stays as it was; other TODOs are not touched.
        assert_eq!(get(&pool, done, today()).await.unwrap(), before_done);
        assert_eq!(
            get(&pool, other, today()).await.unwrap().todo.status,
            Status::Todo
        );

        // Reopening the parent leaves the children closed.
        change_status_impl(
            &pool,
            parent,
            &StatusChange::to(Status::Doing),
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(
            get(&pool, open, today()).await.unwrap().todo.status,
            Status::Done
        );
    }

    #[tokio::test]
    async fn cancelling_a_parent_gives_its_children_the_same_reason() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, pid, None, "親").await.todo.id;
        let child = add(&pool, pid, Some(parent), "子").await.todo.id;
        change_status_impl(
            &pool,
            child,
            &StatusChange::to(Status::Doing),
            today(),
            now(),
        )
        .await
        .unwrap();
        change_status_impl(
            &pool,
            parent,
            &StatusChange::cancel(" 計画の見直し "),
            today(),
            now(),
        )
        .await
        .unwrap();
        let c = get(&pool, child, today()).await.unwrap();
        assert_eq!(c.todo.status, Status::Cancelled);
        assert_eq!(c.todo.close_reason.as_deref(), Some("計画の見直し"));
        assert_eq!(c.todo.actual_end, Some(today()));
        assert_eq!(
            status_events(&pool, child).await.last().cloned(),
            Some((s("doing"), s("cancelled"), s("計画の見直し")))
        );
    }

    #[tokio::test]
    async fn a_child_that_cannot_close_stops_the_whole_change() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, pid, None, "親").await.todo.id;
        let child = add(&pool, pid, Some(parent), "先の子").await;
        // An actual start after today: 完了 would end before it starts.
        let mut f = fields_of(&child.todo);
        f.actual_start = Some(d("2026-11-25"));
        update(&pool, child.todo.id, f, today(), now())
            .await
            .unwrap();

        let e = change_status_impl(
            &pool,
            parent,
            &StatusChange::to(Status::Done),
            today(),
            now(),
        )
        .await
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Validation);
        assert!(
            e.message
                .starts_with("子TODO「先の子」を完了にできません。実績開始日(11月25日)"),
            "{}",
            e.message
        );
        // Nothing changed, nothing recorded.
        assert_eq!(
            get(&pool, parent, today()).await.unwrap().todo.status,
            Status::Todo
        );
        assert!(status_events(&pool, parent).await.is_empty());
        assert!(status_events(&pool, child.todo.id).await.is_empty());
    }

    #[tokio::test]
    async fn list_is_a_tree_with_bridge_rows_left_out_of_the_counts() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, pid, None, "親").await.todo.id;
        add(&pool, pid, None, "単独").await;
        let a = add(&pool, pid, Some(parent), "子a").await.todo.id;
        add(&pool, pid, Some(parent), "子b").await;
        change_status_impl(&pool, a, &StatusChange::to(Status::Doing), today(), now())
            .await
            .unwrap();
        let shape = |l: &TodoList| {
            l.rows
                .iter()
                .map(|r| (r.item.todo.title.clone(), r.child, r.bridge))
                .collect::<Vec<_>>()
        };
        let row = |t: &str, child: bool, bridge: bool| (t.to_owned(), child, bridge);

        let all = list(&pool, &TodoQuery::default(), today()).await.unwrap();
        assert_eq!(
            shape(&all),
            vec![
                row("親", false, false),
                row("子a", true, false),
                row("子b", true, false),
                row("単独", false, false),
            ]
        );
        assert_eq!(all.total, 4);
        assert_eq!(
            all.rows[0].item.children,
            Some(ChildCounts {
                total: 2,
                done: 0,
                open: 2
            })
        );

        // 作業中 tab: only 子a matches; its parent comes as a bridge row
        // that the tabs do not count.
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
        assert_eq!(
            shape(&doing),
            vec![row("親", false, true), row("子a", true, false)]
        );
        assert_eq!(
            (doing.total, doing.counts.doing, doing.counts.todo),
            (4, 1, 3)
        );
        // The bridge row still carries every child in its counts.
        assert_eq!(doing.rows[0].item.children.map(|c| c.total), Some(2));
    }

    #[tokio::test]
    async fn due_after_parent_is_judged_on_the_dates_as_entered() {
        let (pool, pid) = setup().await;
        let parent = create(
            &pool,
            NewTodoInput {
                due_date: Some(d("2026-11-20")),
                ..new_input(pid, "親")
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        let child = create(
            &pool,
            NewTodoInput {
                parent_id: Some(parent.todo.id),
                due_date: Some(d("2026-11-21")),
                ..new_input(pid, "子")
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        assert!(child.due_after_parent);
        let mut f = fields_of(&child.todo);
        f.due_date = Some(d("2026-11-20"));
        assert!(
            !update(&pool, child.todo.id, f, today(), now())
                .await
                .unwrap()
                .due_after_parent
        );
    }

    #[tokio::test]
    async fn deleting_a_parent_deletes_its_children() {
        let (pool, pid) = setup().await;
        let parent = add(&pool, pid, None, "親").await.todo.id;
        let child = add(&pool, pid, Some(parent), "子").await.todo.id;
        delete(&pool, parent).await.unwrap();
        assert_eq!(
            get(&pool, child, today()).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
    }
}
