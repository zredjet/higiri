//! 全文検索 (T-7): the TODOs that contain every word.
//!
//! `LIKE … ESCAPE '\'` over the title, memo, 待っている相手, 中止の理由 and
//! the work logs' text, archived projects included. `%`, `_` and `\` in a
//! word are matched as characters. SQLite's `LIKE` ignores the case of ASCII
//! letters only, as the design asks (全角・半角 are not folded).

use sqlx::{QueryBuilder, Sqlite, SqliteConnection};

use super::{Result, read_ts};
use crate::domain::search::Candidate;

/// `%word%` with the `LIKE` wildcards (and the escape character) escaped.
fn like_pattern(word: &str) -> String {
    let mut out = String::with_capacity(word.len() + 2);
    out.push('%');
    for c in word.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// Every TODO that contains all of `words` (each word in any of the
/// searched fields), unordered. No words: none.
pub async fn candidates(conn: &mut SqliteConnection, words: &[String]) -> Result<Vec<Candidate>> {
    if words.is_empty() {
        return Ok(Vec::new());
    }
    // Every word is bound as a parameter; only fixed SQL text is pushed.
    let mut query =
        QueryBuilder::<Sqlite>::new("SELECT t.id, t.title, t.updated_at FROM todos t WHERE ");
    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            query.push(" AND ");
        }
        let pattern = like_pattern(word);
        query.push("(t.title LIKE ");
        query.push_bind(pattern.clone());
        query.push(" ESCAPE '\\' OR t.memo_md LIKE ");
        query.push_bind(pattern.clone());
        query.push(" ESCAPE '\\' OR t.waiting_on LIKE ");
        query.push_bind(pattern.clone());
        query.push(" ESCAPE '\\' OR t.close_reason LIKE ");
        query.push_bind(pattern.clone());
        query.push(
            " ESCAPE '\\' OR EXISTS (SELECT 1 FROM work_logs w WHERE w.todo_id = t.id AND w.body_md LIKE ",
        );
        query.push_bind(pattern);
        query.push(" ESCAPE '\\'))");
    }
    query
        .build_query_as::<(i64, String, String)>()
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|(id, title, updated_at)| {
            Ok(Candidate {
                id,
                title,
                updated_at: read_ts("todos.updated_at", updated_at)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Priority;
    use crate::domain::search::split_terms;
    use crate::domain::testing::{d, ts};
    use crate::repo::projects;
    use crate::repo::testing::{conn, project};
    use crate::repo::todos::{self, NewTodo};
    use crate::repo::work_logs::{self, NewWorkLog};

    async fn add(c: &mut SqliteConnection, project_id: i64, title: &str, memo: &str) -> i64 {
        todos::insert(
            c,
            &NewTodo {
                project_id,
                title: title.into(),
                priority: Priority::Medium,
                due_date: None,
                planned_start: None,
                planned_end: None,
                memo_md: memo.into(),
            },
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap()
        .id
    }

    async fn ids(c: &mut SqliteConnection, text: &str) -> Vec<i64> {
        let mut found: Vec<i64> = candidates(c, &split_terms(text))
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        found.sort();
        found
    }

    #[test]
    fn escapes_the_like_wildcards() {
        assert_eq!(like_pattern("認証"), "%認証%");
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        assert_eq!(like_pattern("C:\\tmp"), "%C:\\\\tmp%");
    }

    #[tokio::test]
    async fn percent_underscore_and_backslash_are_characters() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "基幹システム更改").await;
        let pct = add(&mut c, p.id, "進捗100%の確認", "").await;
        let plain = add(&mut c, p.id, "進捗1000件", "").await;
        let under = add(&mut c, p.id, "user_id の整理", "").await;
        let other = add(&mut c, p.id, "userXid の整理", "").await;
        let slash = add(&mut c, p.id, "C:\\tmp を消す", "").await;

        assert_eq!(ids(&mut c, "100%").await, vec![pct]);
        assert_eq!(ids(&mut c, "100").await, vec![pct, plain]);
        assert_eq!(ids(&mut c, "user_id").await, vec![under]);
        assert_eq!(ids(&mut c, "user").await, vec![under, other]);
        assert_eq!(ids(&mut c, "%").await, vec![pct]);
        assert_eq!(ids(&mut c, "_").await, vec![under]);
        assert_eq!(ids(&mut c, "\\").await, vec![slash]);
        assert_eq!(ids(&mut c, "C:\\tmp").await, vec![slash]);
    }

    #[tokio::test]
    async fn ascii_ignores_case_and_width_is_not_folded() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "基幹システム更改").await;
        let api = add(&mut c, p.id, "API設計書のレビュー", "").await;
        let wide = add(&mut c, p.id, "ＡＰＩの一覧", "").await;
        let kana = add(&mut c, p.id, "レビュー会", "").await;

        assert_eq!(ids(&mut c, "api").await, vec![api]);
        assert_eq!(ids(&mut c, "Api").await, vec![api]);
        assert_eq!(ids(&mut c, "ＡＰＩ").await, vec![wide]);
        // 全角 letters do not fold case either.
        assert_eq!(ids(&mut c, "ａｐｉ").await, Vec::<i64>::new());
        // Half-width katakana is a different text.
        assert_eq!(ids(&mut c, "ﾚﾋﾞｭｰ").await, Vec::<i64>::new());
        assert_eq!(ids(&mut c, "レビュー").await, vec![api, kana]);
    }

    #[tokio::test]
    async fn two_character_japanese_words_in_every_field() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "基幹システム更改").await;
        let title = add(&mut c, p.id, "認証方式の検討", "").await;
        let memo = add(&mut c, p.id, "設計", "# 概要\n- 認証APIの確認").await;
        let waiting = add(&mut c, p.id, "回答待ち", "").await;
        let reason = add(&mut c, p.id, "移行", "").await;
        let logged = add(&mut c, p.id, "結合試験", "").await;
        let none = add(&mut c, p.id, "資料の準備", "認可の話").await;
        sqlx::query("UPDATE todos SET status = 'waiting', waiting_on = '認証担当' WHERE id = ?1")
            .bind(waiting)
            .execute(&mut *c)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE todos SET status = 'cancelled', close_reason = '認証基盤に統合' WHERE id = ?1",
        )
        .bind(reason)
        .execute(&mut *c)
        .await
        .unwrap();
        for (todo_id, body) in [(logged, "- 認証の手順を確認"), (none, "- 権限の確認")]
        {
            work_logs::insert(
                &mut c,
                &NewWorkLog {
                    todo_id,
                    log_date: d("2026-11-19"),
                    body_md: body.into(),
                    minutes: None,
                },
                ts(2026, 11, 19, 2, 0),
            )
            .await
            .unwrap();
        }

        assert_eq!(
            ids(&mut c, "認証").await,
            vec![title, memo, waiting, reason, logged]
        );
    }

    #[tokio::test]
    async fn every_word_must_appear_somewhere_in_the_todo() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "基幹システム更改").await;
        let both = add(&mut c, p.id, "認証のレビュー", "").await;
        let split = add(&mut c, p.id, "認証方式の検討", "").await;
        let one = add(&mut c, p.id, "認証APIの確認", "").await;
        work_logs::insert(
            &mut c,
            &NewWorkLog {
                todo_id: split,
                log_date: d("2026-11-18"),
                body_md: "レビューを依頼".into(),
                minutes: None,
            },
            ts(2026, 11, 18, 2, 0),
        )
        .await
        .unwrap();

        assert_eq!(ids(&mut c, "認証 レビュー").await, vec![both, split]);
        assert_eq!(ids(&mut c, "認証\u{3000}レビュー").await, vec![both, split]);
        assert_eq!(ids(&mut c, "認証").await, vec![both, split, one]);
        assert_eq!(ids(&mut c, "認証 存在しない").await, Vec::<i64>::new());
        assert_eq!(ids(&mut c, "").await, Vec::<i64>::new());
    }

    #[tokio::test]
    async fn archived_projects_are_searched() {
        let (_pool, mut c) = conn().await;
        let active = project(&mut c, "基幹システム更改").await;
        let old = project(&mut c, "旧システム保守").await;
        let a = add(&mut c, active.id, "認証方式の検討", "").await;
        let b = add(&mut c, old.id, "認証の廃止", "").await;
        projects::set_archived(
            &mut c,
            old.id,
            Some(ts(2026, 11, 19, 3, 0)),
            ts(2026, 11, 19, 3, 0),
        )
        .await
        .unwrap();

        assert_eq!(ids(&mut c, "認証").await, vec![a, b]);
        let found = candidates(&mut c, &split_terms("廃止")).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "認証の廃止");
        assert_eq!(found[0].updated_at, ts(2026, 11, 19, 1, 0));
    }
}
