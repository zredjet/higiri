//! TODOテンプレート (T-10, v0.3.0): list_todo_templates, save_todo_template,
//! rename_todo_template, delete_todo_template. A TODO is made from a template
//! by create_todo (`template_id`, in `commands/todos.rs`).

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tauri::State;

use super::clock;
use super::error::{AppError, CmdResult};
use crate::domain::template::{TemplateBody, TemplateSummary, checked_name};
use crate::repo::db;
use crate::repo::templates::TemplateFields;
use crate::repo::{self, RepoError};

// ---- tauri commands ----------------------------------------------------------

/// Every template, by name.
#[tauri::command]
#[specta::specta]
pub async fn list_todo_templates(pool: State<'_, SqlitePool>) -> CmdResult<Vec<TemplateSummary>> {
    list(&pool).await
}

/// Saves a TODO without a parent and its children (中止 ones left out) as a
/// template named `name`. When a template has that name already, it is
/// replaced only with `replace` (the screen asks first).
#[tauri::command]
#[specta::specta]
pub async fn save_todo_template(
    pool: State<'_, SqlitePool>,
    todo_id: i64,
    name: String,
    replace: bool,
) -> CmdResult<TemplateSummary> {
    save(&pool, todo_id, &name, replace, clock::now()).await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_todo_template(
    pool: State<'_, SqlitePool>,
    id: i64,
    name: String,
) -> CmdResult<TemplateSummary> {
    rename(&pool, id, &name, clock::now()).await
}

/// Deletes a template. The TODOs made from it stay. The screen confirms first.
#[tauri::command]
#[specta::specta]
pub async fn delete_todo_template(pool: State<'_, SqlitePool>, id: i64) -> CmdResult<()> {
    delete(&pool, id).await
}

// ---- implementation ----------------------------------------------------------

fn same_name() -> AppError {
    AppError::validation("同じ名前のテンプレートがあります。別の名前を入力してください。")
}

/// A name taken in the meantime (the UNIQUE constraint) reads as
/// [`same_name`].
fn taken(e: RepoError) -> AppError {
    if e.is_constraint_violation() {
        same_name()
    } else {
        e.into()
    }
}

fn name(value: &str) -> CmdResult<String> {
    checked_name(value).map_err(|e| AppError::validation(e.to_string()))
}

pub async fn list(pool: &SqlitePool) -> CmdResult<Vec<TemplateSummary>> {
    let mut conn = pool.acquire().await?;
    Ok(repo::templates::list(&mut conn)
        .await?
        .iter()
        .map(|t| t.summary())
        .collect())
}

pub async fn save(
    pool: &SqlitePool,
    todo_id: i64,
    name_input: &str,
    replace: bool,
    now: DateTime<Utc>,
) -> CmdResult<TemplateSummary> {
    let name = name(name_input)?;
    let mut tx = db::begin_write(pool).await?;
    let todo = repo::todos::get(&mut tx, todo_id).await?;
    if todo.parent_id.is_some() {
        return Err(AppError::validation(
            "子TODOからはテンプレートを作れません。親のTODOから保存してください。",
        ));
    }
    let children = repo::todos::list_children(&mut tx, &[todo.id]).await?;
    // The dependencies between the children (T-11).
    let child_ids: Vec<i64> = children.iter().map(|c| c.id).collect();
    let dependencies = repo::dependencies::among(&mut tx, &child_ids).await?;
    // Titles are required when a TODO is saved; only data from elsewhere (an
    // imported file) can have an empty one.
    let body = TemplateBody::from_todos(&todo, &children, &dependencies)
        .checked()
        .map_err(|_| {
            AppError::validation(
                "タイトルが空のTODOがあるため、テンプレートにできません。タイトルを入力してから保存してください。",
            )
        })?;
    let fields = TemplateFields { name, body };
    let saved = match repo::templates::id_by_name(&mut tx, &fields.name).await? {
        Some(_) if !replace => return Err(same_name()),
        Some(id) => repo::templates::update(&mut tx, id, &fields, now).await?,
        None => repo::templates::insert(&mut tx, &fields, now)
            .await
            .map_err(taken)?,
    };
    tx.commit().await?;
    Ok(saved.summary())
}

pub async fn rename(
    pool: &SqlitePool,
    id: i64,
    name_input: &str,
    now: DateTime<Utc>,
) -> CmdResult<TemplateSummary> {
    let name = name(name_input)?;
    let mut tx = db::begin_write(pool).await?;
    let template = repo::templates::get(&mut tx, id).await?;
    if template.name == name {
        return Ok(template.summary());
    }
    if repo::templates::id_by_name(&mut tx, &name).await?.is_some() {
        return Err(same_name());
    }
    let fields = TemplateFields {
        name,
        body: template.body,
    };
    let saved = repo::templates::update(&mut tx, id, &fields, now)
        .await
        .map_err(taken)?;
    tx.commit().await?;
    Ok(saved.summary())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> CmdResult<()> {
    let mut conn = pool.acquire().await?;
    Ok(repo::templates::delete(&mut conn, id).await?)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::commands::projects::{self, ProjectInput};
    use crate::commands::todos::{self, NewTodoInput, TodoFields};
    use crate::domain::model::{Priority, Status, Todo};
    use crate::domain::status::StatusChange;
    use crate::domain::testing::{d, ts};
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
                name: "経理".into(),
                color: None,
                description: String::new(),
            },
            now(),
        )
        .await
        .unwrap();
        (pool, p.id)
    }

    fn input(
        project_id: i64,
        parent_id: Option<i64>,
        title: &str,
        due: Option<&str>,
    ) -> NewTodoInput {
        NewTodoInput {
            project_id,
            parent_id,
            template_id: None,
            title: title.into(),
            priority: None,
            due_date: due.map(d),
            planned_start: None,
            planned_end: None,
            memo_md: None,
        }
    }

    async fn add(pool: &SqlitePool, input: NewTodoInput) -> Todo {
        todos::create(pool, input, today(), now())
            .await
            .unwrap()
            .todo
    }

    /// 月次締め (due 11/30) with three children: 売上の集計 (11/27),
    /// 中止した子 (cancelled), 報告書の作成 (no due date).
    async fn family(pool: &SqlitePool, pid: i64) -> Todo {
        let parent = add(
            pool,
            NewTodoInput {
                priority: Some(Priority::High),
                memo_md: Some("## 手順\n- 前月分を集計する".into()),
                ..input(pid, None, "月次締め", Some("2026-11-30"))
            },
        )
        .await;
        add(
            pool,
            NewTodoInput {
                priority: Some(Priority::Low),
                ..input(pid, Some(parent.id), "売上の集計", Some("2026-11-27"))
            },
        )
        .await;
        let cancelled = add(
            pool,
            input(pid, Some(parent.id), "中止した子", Some("2026-11-28")),
        )
        .await;
        todos::change_status_impl(
            pool,
            cancelled.id,
            &StatusChange::cancel("不要"),
            today(),
            now(),
        )
        .await
        .unwrap();
        add(pool, input(pid, Some(parent.id), "報告書の作成", None)).await;
        parent
    }

    #[tokio::test]
    async fn save_makes_a_template_of_the_parent_and_its_children() {
        let (pool, pid) = setup().await;
        let parent = family(&pool, pid).await;
        let saved = save(&pool, parent.id, "  月次締め ", false, now())
            .await
            .unwrap();
        assert_eq!(saved.name, "月次締め");
        assert_eq!(saved.title, "月次締め");
        assert_eq!(saved.priority, Priority::High);
        assert_eq!(saved.child_titles, ["売上の集計", "報告書の作成"]);
        assert_eq!(saved.updated_at, now());

        let mut c = pool.acquire().await.unwrap();
        let t = repo::templates::get(&mut c, saved.id).await.unwrap();
        assert_eq!(t.body.memo_md, "## 手順\n- 前月分を集計する");
        assert_eq!(t.body.children[0].priority, Priority::Low);
        assert_eq!(t.body.children[0].due_offset_days, Some(-3));
        assert_eq!(t.body.children[1].due_offset_days, None);
        drop(c);
        assert_eq!(list(&pool).await.unwrap(), vec![saved]);
    }

    #[tokio::test]
    async fn save_checks_the_name_and_where_it_is_saved_from() {
        let (pool, pid) = setup().await;
        let parent = family(&pool, pid).await;
        let message = |r: CmdResult<TemplateSummary>| r.map(|t| t.name).map_err(|e| e.message);

        assert_eq!(
            message(save(&pool, parent.id, " \u{3000}", false, now()).await),
            Err("テンプレートの名前を入力してください。".into())
        );
        assert_eq!(
            message(save(&pool, parent.id, &"締".repeat(41), false, now()).await),
            Err("テンプレートの名前は40字までにしてください。".into())
        );
        assert_eq!(
            message(save(&pool, parent.id, &"締".repeat(40), false, now()).await),
            Ok("締".repeat(40))
        );

        let mut c = pool.acquire().await.unwrap();
        let child = repo::todos::list_children(&mut c, &[parent.id])
            .await
            .unwrap()
            .remove(0);
        drop(c);
        assert_eq!(
            message(save(&pool, child.id, "子から", false, now()).await),
            Err("子TODOからはテンプレートを作れません。親のTODOから保存してください。".into())
        );
        assert_eq!(
            save(&pool, 999, "ない", false, now())
                .await
                .unwrap_err()
                .kind,
            ErrorKind::NotFound
        );
        // Nothing but the 40-character one was saved.
        assert_eq!(list(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn the_same_name_is_replaced_only_when_asked() {
        let (pool, pid) = setup().await;
        let parent = family(&pool, pid).await;
        let first = save(&pool, parent.id, "月次締め", false, now())
            .await
            .unwrap();
        let other = add(&pool, input(pid, None, "週次の確認", Some("2026-11-27"))).await;

        let e = save(&pool, other.id, "月次締め", false, now())
            .await
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Validation);
        assert_eq!(
            e.message,
            "同じ名前のテンプレートがあります。別の名前を入力してください。"
        );
        assert_eq!(list(&pool).await.unwrap(), vec![first.clone()]);

        let later = ts(2026, 11, 20, 0, 0);
        let replaced = save(&pool, other.id, "月次締め", true, later)
            .await
            .unwrap();
        assert_eq!(replaced.id, first.id);
        assert_eq!(replaced.title, "週次の確認");
        assert!(replaced.child_titles.is_empty());
        assert_eq!(replaced.updated_at, later);
        assert_eq!(list(&pool).await.unwrap(), vec![replaced]);

        // `replace` with a new name just adds one.
        save(&pool, parent.id, "月次締め(新)", true, now())
            .await
            .unwrap();
        assert_eq!(list(&pool).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn rename_checks_the_name() {
        let (pool, pid) = setup().await;
        let parent = family(&pool, pid).await;
        let a = save(&pool, parent.id, "月次締め", false, now())
            .await
            .unwrap();
        let b = save(&pool, parent.id, "週次", false, now()).await.unwrap();

        let later = ts(2026, 11, 20, 0, 0);
        let renamed = rename(&pool, a.id, " 月末の締め ", later).await.unwrap();
        assert_eq!(renamed.name, "月末の締め");
        assert_eq!(renamed.child_titles, a.child_titles);
        assert_eq!(renamed.updated_at, later);

        assert_eq!(
            rename(&pool, a.id, "週次", later)
                .await
                .unwrap_err()
                .message,
            "同じ名前のテンプレートがあります。別の名前を入力してください。"
        );
        assert_eq!(
            rename(&pool, a.id, "", later).await.unwrap_err().message,
            "テンプレートの名前を入力してください。"
        );
        // Its own name again changes nothing.
        assert_eq!(
            rename(&pool, b.id, "週次 ", ts(2026, 11, 21, 0, 0))
                .await
                .unwrap(),
            b
        );
        assert_eq!(
            rename(&pool, 999, "x", later).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
        let names: Vec<String> = list(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, ["月末の締め", "週次"]);
    }

    #[tokio::test]
    async fn delete_removes_only_the_template() {
        let (pool, pid) = setup().await;
        let parent = family(&pool, pid).await;
        let t = save(&pool, parent.id, "月次締め", false, now())
            .await
            .unwrap();
        delete(&pool, t.id).await.unwrap();
        assert!(list(&pool).await.unwrap().is_empty());
        let e = delete(&pool, t.id).await.unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound);
        assert_eq!(
            e.message,
            "テンプレートが見つかりません。削除された可能性があります。画面を開き直してください。"
        );
        // The TODOs it was saved from are as they were.
        assert_eq!(
            todos::get(&pool, parent.id, today())
                .await
                .unwrap()
                .children
                .map(|c| c.total),
            Some(2)
        );
    }

    // ---- create_todo with a template ------------------------------------------

    async fn statuses_and_events(pool: &SqlitePool, ids: &[i64]) -> Vec<(Status, usize)> {
        let mut c = pool.acquire().await.unwrap();
        let mut out = Vec::new();
        for &id in ids {
            let t = repo::todos::get(&mut c, id).await.unwrap();
            let events = repo::events::list_by_todo(&mut c, id).await.unwrap();
            assert!(
                events
                    .iter()
                    .all(|e| e.kind == crate::domain::events::EventKind::Created)
            );
            out.push((t.status, events.len()));
        }
        out
    }

    #[tokio::test]
    async fn create_from_a_template_makes_the_parent_and_its_children() {
        let (pool, pid) = setup().await;
        let source = family(&pool, pid).await;
        // The source is closed and its children too: the new ones start fresh.
        todos::change_status_impl(
            &pool,
            source.id,
            &StatusChange::to(Status::Done),
            today(),
            now(),
        )
        .await
        .unwrap();
        let t = save(&pool, source.id, "月次締め", false, now())
            .await
            .unwrap();
        let other = projects::create(
            &pool,
            ProjectInput {
                name: "別のプロジェクト".into(),
                color: None,
                description: String::new(),
            },
            now(),
        )
        .await
        .unwrap();

        let made = todos::create(
            &pool,
            NewTodoInput {
                template_id: Some(t.id),
                priority: Some(Priority::Medium),
                planned_start: Some(d("2026-12-21")),
                planned_end: Some(d("2026-12-30")),
                ..input(other.id, None, "12月の締め", Some("2026-12-31"))
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        // The parent: title, priority, project, due date and planned period
        // from the input, the memo from the template.
        assert_eq!(made.todo.title, "12月の締め");
        assert_eq!(made.todo.priority, Priority::Medium);
        assert_eq!(made.todo.project_id, other.id);
        assert_eq!(made.todo.due_date, Some(d("2026-12-31")));
        assert_eq!(made.todo.planned_start, Some(d("2026-12-21")));
        assert_eq!(made.todo.memo_md, "## 手順\n- 前月分を集計する");
        assert_eq!(made.todo.status, Status::Todo);
        assert_eq!(made.children.map(|c| (c.total, c.open)), Some((2, 2)));

        let mut c = pool.acquire().await.unwrap();
        let kids = repo::todos::list_children(&mut c, &[made.todo.id])
            .await
            .unwrap();
        drop(c);
        let shape: Vec<_> = kids
            .iter()
            .map(|k| {
                (
                    k.title.as_str(),
                    k.priority,
                    k.due_date,
                    k.project_id,
                    k.planned_start,
                )
            })
            .collect();
        assert_eq!(
            shape,
            [
                (
                    "売上の集計",
                    Priority::Low,
                    Some(d("2026-12-28")),
                    other.id,
                    None
                ),
                ("報告書の作成", Priority::Medium, None, other.id, None),
            ]
        );
        // Everything 未着手 with one 「作成」 each.
        let mut ids = vec![made.todo.id];
        ids.extend(kids.iter().map(|k| k.id));
        assert_eq!(
            statuses_and_events(&pool, &ids).await,
            vec![(Status::Todo, 1); 3]
        );
    }

    #[tokio::test]
    async fn the_dependencies_between_the_children_are_kept_and_joined_again() {
        use crate::commands::dependencies;
        let (pool, pid) = setup().await;
        let parent = family(&pool, pid).await;
        let mut c = pool.acquire().await.unwrap();
        let kids = repo::todos::list_children(&mut c, &[parent.id])
            .await
            .unwrap();
        drop(c);
        let (sales, cancelled, report) = (kids[0].id, kids[1].id, kids[2].id);
        let outside = add(&pool, input(pid, None, "前月の確定", None)).await;
        // 報告書 waits for 売上 and the cancelled child; 売上 waits for a TODO
        // outside the family. Only 報告書 → 売上 is kept.
        for (later, first) in [(report, sales), (report, cancelled), (sales, outside.id)] {
            dependencies::add(&pool, later, first, today(), now())
                .await
                .unwrap();
        }
        let t = save(&pool, parent.id, "月次締め", false, now())
            .await
            .unwrap();
        let mut c = pool.acquire().await.unwrap();
        let body = repo::templates::get(&mut c, t.id).await.unwrap().body;
        drop(c);
        assert_eq!(
            body.children
                .iter()
                .map(|k| (k.title.as_str(), k.depends_on.clone()))
                .collect::<Vec<_>>(),
            [("売上の集計", vec![]), ("報告書の作成", vec![0])]
        );

        let made = todos::create(
            &pool,
            NewTodoInput {
                template_id: Some(t.id),
                ..input(pid, None, "12月の締め", Some("2026-12-31"))
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        let mut c = pool.acquire().await.unwrap();
        let new_kids = repo::todos::list_children(&mut c, &[made.todo.id])
            .await
            .unwrap();
        let new_ids: Vec<i64> = new_kids.iter().map(|k| k.id).collect();
        let joined: Vec<(i64, i64)> = repo::dependencies::list(&mut c)
            .await
            .unwrap()
            .into_iter()
            .map(|d| (d.todo_id, d.depends_on_id))
            .filter(|(later, _)| new_ids.contains(later))
            .collect();
        drop(c);
        // The new 報告書の作成 waits for the new 売上の集計, nothing else.
        assert_eq!(joined, [(new_ids[1], new_ids[0])]);
        let report_item = todos::get(&pool, new_ids[1], today()).await.unwrap();
        assert_eq!(
            report_item
                .blocked_by
                .iter()
                .map(|b| b.title.as_str())
                .collect::<Vec<_>>(),
            ["売上の集計"]
        );
    }

    #[tokio::test]
    async fn without_a_due_date_no_child_gets_one() {
        let (pool, pid) = setup().await;
        let source = family(&pool, pid).await;
        let t = save(&pool, source.id, "月次締め", false, now())
            .await
            .unwrap();
        let made = todos::create(
            &pool,
            NewTodoInput {
                template_id: Some(t.id),
                memo_md: Some("今月は手で書いたメモ".into()),
                ..input(pid, None, "月次締め", None)
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        // A memo in the input wins over the template's.
        assert_eq!(made.todo.memo_md, "今月は手で書いたメモ");
        let mut c = pool.acquire().await.unwrap();
        let kids = repo::todos::list_children(&mut c, &[made.todo.id])
            .await
            .unwrap();
        assert_eq!(kids.len(), 2);
        assert!(kids.iter().all(|k| k.due_date.is_none()));
    }

    #[tokio::test]
    async fn a_template_cannot_make_a_child_or_come_from_nowhere() {
        let (pool, pid) = setup().await;
        let source = family(&pool, pid).await;
        let t = save(&pool, source.id, "月次締め", false, now())
            .await
            .unwrap();
        let e = todos::create(
            &pool,
            NewTodoInput {
                template_id: Some(t.id),
                ..input(pid, Some(source.id), "子にする", None)
            },
            today(),
            now(),
        )
        .await
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::Validation);
        assert_eq!(
            e.message,
            "テンプレートから作るTODOは、子TODOにできません(親子は1段だけです)。親のTODOを選ばずに追加してください。"
        );
        let e = todos::create(
            &pool,
            NewTodoInput {
                template_id: Some(999),
                ..input(pid, None, "ない", None)
            },
            today(),
            now(),
        )
        .await
        .unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound);
        // A refused create leaves nothing behind (4 = the family).
        let mut c = pool.acquire().await.unwrap();
        assert_eq!(
            repo::todos::list(&mut c, &Default::default())
                .await
                .unwrap()
                .len(),
            4
        );
    }

    #[tokio::test]
    async fn a_template_outlives_the_todos_it_was_saved_from() {
        let (pool, pid) = setup().await;
        let source = family(&pool, pid).await;
        let t = save(&pool, source.id, "月次締め", false, now())
            .await
            .unwrap();
        todos::delete(&pool, source.id).await.unwrap();
        // The source project goes too; another one uses the template.
        projects::delete(&pool, pid).await.unwrap();
        assert_eq!(list(&pool).await.unwrap(), vec![t.clone()]);
        let qid = projects::create(
            &pool,
            ProjectInput {
                name: "新しいプロジェクト".into(),
                color: None,
                description: String::new(),
            },
            now(),
        )
        .await
        .unwrap()
        .id;
        let made = todos::create(
            &pool,
            NewTodoInput {
                template_id: Some(t.id),
                ..input(qid, None, "月次締め", Some("2026-11-30"))
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(made.children.map(|c| c.total), Some(2));
    }

    #[tokio::test]
    async fn a_saved_template_does_not_follow_later_edits() {
        let (pool, pid) = setup().await;
        let source = family(&pool, pid).await;
        let t = save(&pool, source.id, "月次締め", false, now())
            .await
            .unwrap();
        let current = todos::get(&pool, source.id, today()).await.unwrap().todo;
        todos::update(
            &pool,
            source.id,
            TodoFields {
                title: "名前を変えた".into(),
                priority: current.priority,
                due_date: current.due_date,
                planned_start: None,
                planned_end: None,
                actual_start: None,
                actual_end: None,
                waiting_on: None,
                follow_up_date: None,
                close_reason: None,
                memo_md: String::new(),
            },
            today(),
            now(),
        )
        .await
        .unwrap();
        assert_eq!(list(&pool).await.unwrap(), vec![t]);
    }
}
