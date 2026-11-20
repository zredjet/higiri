//! 検索: search_todos (v1.1, T-7).

use std::collections::HashMap;

use chrono::NaiveDate;
use sqlx::SqlitePool;
use tauri::State;

use super::clock;
use super::error::CmdResult;
use crate::domain::search::{MAX_HITS, SearchResult, logs_by_todo, rank, search_hit, split_terms};
use crate::domain::subtodo::Family;
use crate::domain::urgency::Rules;
use crate::domain::views::todo_item;
use crate::repo;

/// TODOs that contain every word of `text` (split on spaces, 全角 included),
/// with where each word was found. At most 100, with the total count.
#[tauri::command]
#[specta::specta]
pub async fn search_todos(pool: State<'_, SqlitePool>, text: String) -> CmdResult<SearchResult> {
    search(&pool, &text, clock::today()).await
}

pub async fn search(pool: &SqlitePool, text: &str, today: NaiveDate) -> CmdResult<SearchResult> {
    let terms = split_terms(text);
    if terms.is_empty() {
        return Ok(SearchResult::empty());
    }
    let mut conn = pool.acquire().await?;
    let candidates = repo::search::candidates(&mut conn, &terms).await?;
    let total = candidates.len() as i64;
    let ids = rank(candidates, &terms, MAX_HITS);

    let todos = repo::todos::list_by_ids(&mut conn, &ids).await?;
    // Parents (for 「親 / 子」) and children (for the counts) of the hits.
    let parent_ids: Vec<i64> = todos.iter().filter_map(|t| t.parent_id).collect();
    let parents = repo::todos::list_by_ids(&mut conn, &parent_ids).await?;
    let children = repo::todos::list_children(&mut conn, &ids).await?;
    let family = Family::new(todos.iter().chain(&parents).chain(&children));
    let mut logs = logs_by_todo(repo::work_logs::list_by_todos(&mut conn, &ids).await?);
    let names: HashMap<i64, String> = repo::projects::list(&mut conn, true)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let rules = Rules::from_settings(&repo::settings::load(&mut conn).await?);

    let hits = todos
        .into_iter()
        .map(|t| {
            let own = logs.remove(&t.id).unwrap_or_default();
            let name = names.get(&t.project_id).cloned().unwrap_or_default();
            search_hit(todo_item(t, name, &family, today, &rules), &own, &terms)
        })
        .collect();
    Ok(SearchResult { hits, total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::projects::{self, ProjectInput};
    use crate::commands::todos::{self, NewTodoInput, TodoFields};
    use crate::commands::work_logs::{self, WorkLogInput};
    use crate::domain::model::{Priority, Status};
    use crate::domain::search::{MatchField, Segment};
    use crate::domain::status::StatusChange;
    use crate::domain::testing::{d, ts};
    use crate::domain::urgency::Urgency;
    use crate::repo::db;

    fn today() -> NaiveDate {
        d("2026-11-19")
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

    async fn add(pool: &SqlitePool, project_id: i64, title: &str, day: u32) -> i64 {
        todos::create(
            pool,
            NewTodoInput {
                project_id,
                parent_id: None,
                template_id: None,
                title: title.into(),
                priority: Some(Priority::Medium),
                due_date: None,
                planned_start: None,
                planned_end: None,
                memo_md: None,
            },
            today(),
            ts(2026, 11, day, 0, 0),
        )
        .await
        .unwrap()
        .todo
        .id
    }

    fn text(segments: &[Segment]) -> String {
        segments
            .iter()
            .map(|s| {
                if s.hit {
                    format!("[{}]", s.text)
                } else {
                    s.text.clone()
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn empty_words_find_nothing() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "基幹システム更改").await;
        add(&pool, p, "認証方式の検討", 10).await;
        for words in ["", "  ", "\u{3000}"] {
            assert_eq!(
                search(&pool, words, today()).await.unwrap(),
                SearchResult::empty()
            );
        }
    }

    #[tokio::test]
    async fn returns_items_with_their_matches_in_order() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "基幹システム更改").await;
        let memo_only = add(&pool, p, "API設計書のレビュー", 18).await;
        let title_old = add(&pool, p, "認証方式の検討", 10).await;
        let title_new = add(&pool, p, "認証APIの確認", 12).await;
        add(&pool, p, "結合試験", 19).await;

        // The memo of the first one mentions the word; its update is the newest.
        let item = todos::get(&pool, memo_only, today()).await.unwrap();
        todos::update(
            &pool,
            memo_only,
            TodoFields {
                title: item.todo.title.clone(),
                priority: item.todo.priority,
                due_date: Some(d("2026-11-18")),
                planned_start: None,
                planned_end: None,
                actual_start: None,
                actual_end: None,
                waiting_on: None,
                follow_up_date: None,
                close_reason: None,
                memo_md: "# 概要\n認証の方式を待っている".into(),
            },
            today(),
            ts(2026, 11, 19, 0, 0),
        )
        .await
        .unwrap();
        work_logs::save(
            &pool,
            WorkLogInput {
                id: None,
                todo_id: title_new,
                log_date: d("2026-11-19"),
                body_md: "- 認証担当に確認\n- 回答待ち".into(),
                minutes: None,
            },
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();

        let result = search(&pool, "認証", today()).await.unwrap();
        assert_eq!(result.total, 3);
        let ids: Vec<i64> = result.hits.iter().map(|h| h.item.todo.id).collect();
        // Title hits first (newest update first), then the memo hit.
        assert_eq!(ids, vec![title_new, title_old, memo_only]);

        let first = &result.hits[0];
        assert_eq!(text(&first.title), "[認証]APIの確認");
        assert_eq!(first.item.project_name, "基幹システム更改");
        assert_eq!(first.matches.len(), 1);
        assert_eq!(first.matches[0].field, MatchField::WorkLog);
        assert_eq!(first.matches[0].log_date, Some(d("2026-11-19")));
        assert_eq!(
            text(&first.matches[0].segments),
            "- [認証]担当に確認 - 回答待ち"
        );

        // The item carries today's judgement.
        let last = &result.hits[2];
        assert_eq!(text(&last.title), "API設計書のレビュー");
        assert_eq!(last.item.assessment.urgency, Some(Urgency::Overdue));
        assert_eq!(last.matches[0].field, MatchField::Memo);
        assert_eq!(
            text(&last.matches[0].segments),
            "# 概要 [認証]の方式を待っている"
        );
    }

    #[tokio::test]
    async fn waiting_on_and_cancel_reason_and_archived_projects() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "基幹システム更改").await;
        let old = project(&pool, "旧システム保守").await;
        let waiting = add(&pool, p, "回答の確認", 10).await;
        let cancelled = add(&pool, old, "旧画面の改修", 10).await;
        todos::change_status_impl(
            &pool,
            waiting,
            &StatusChange {
                to: Status::Waiting,
                waiting_on: Some("認証担当".into()),
                follow_up_date: Some(d("2026-11-20")),
                reason: None,
            },
            today(),
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();
        todos::change_status_impl(
            &pool,
            cancelled,
            &StatusChange {
                to: Status::Cancelled,
                waiting_on: None,
                follow_up_date: None,
                reason: Some("認証基盤に統合したため".into()),
            },
            today(),
            ts(2026, 11, 19, 2, 0),
        )
        .await
        .unwrap();
        projects::archive(&pool, old, true, ts(2026, 11, 19, 3, 0))
            .await
            .unwrap();

        let result = search(&pool, "認証", today()).await.unwrap();
        let found: Vec<(i64, MatchField, String)> = result
            .hits
            .iter()
            .flat_map(|h| {
                h.matches
                    .iter()
                    .map(|m| (h.item.todo.id, m.field, text(&m.segments)))
            })
            .collect();
        assert_eq!(
            found,
            vec![
                (
                    cancelled,
                    MatchField::CloseReason,
                    "[認証]基盤に統合したため".to_owned()
                ),
                (waiting, MatchField::WaitingOn, "[認証]担当".to_owned()),
            ]
        );
        assert_eq!(result.hits[0].item.project_name, "旧システム保守");
    }

    #[tokio::test]
    async fn at_most_100_with_the_total() {
        let pool = db::open_in_memory().await.unwrap();
        let p = project(&pool, "基幹システム更改").await;
        let mut tx = pool.begin().await.unwrap();
        for n in 0..105 {
            sqlx::query(
                "INSERT INTO todos (project_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            )
            .bind(p)
            .bind(format!("認証の確認 {n:03}"))
            .bind(format!("2026-11-19T00:{:02}:{:02}.000Z", n / 60, n % 60))
            .execute(&mut *tx)
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();

        let result = search(&pool, "認証", today()).await.unwrap();
        assert_eq!(result.total, 105);
        assert_eq!(result.hits.len(), 100);
        // The newest update first.
        assert_eq!(text(&result.hits[0].title), "[認証]の確認 104");
        assert_eq!(text(&result.hits[99].title), "[認証]の確認 005");

        let result = search(&pool, "認証 100", today()).await.unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(text(&result.hits[0].title), "[認証]の確認 [100]");
    }
}
