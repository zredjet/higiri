//! ガント: get_gantt.

use chrono::NaiveDate;
use serde::Deserialize;
use sqlx::SqlitePool;
use tauri::State;

use super::clock;
use super::error::CmdResult;
use crate::domain::gantt::{self, Gantt, GanttFilter};
use crate::domain::urgency::Rules;
use crate::repo;
use crate::repo::todos::TodoFilter;

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct GanttQuery {
    /// Only this project; `null` = all active projects.
    pub project_id: Option<i64>,
    /// 完了したTODOも表示 (default on in the screen).
    pub include_done: bool,
    /// 中止したTODOも表示 (default off in the screen).
    pub include_cancelled: bool,
}

/// Rows of the Gantt chart, grouped by project: planned and actual spans,
/// 待ち spans and the TODO with its 日限注意 category. All rows are returned;
/// the screen draws the days it shows.
#[tauri::command]
#[specta::specta]
pub async fn get_gantt(pool: State<'_, SqlitePool>, query: GanttQuery) -> CmdResult<Gantt> {
    get(&pool, &query, clock::today()).await
}

pub async fn get(pool: &SqlitePool, query: &GanttQuery, today: NaiveDate) -> CmdResult<Gantt> {
    let mut conn = pool.acquire().await?;
    let settings = repo::settings::load(&mut conn).await?;
    let projects = match query.project_id {
        Some(id) => vec![repo::projects::get(&mut conn, id).await?],
        None => repo::projects::list(&mut conn, false).await?,
    };
    let todos = repo::todos::list(
        &mut conn,
        &TodoFilter {
            project_id: query.project_id,
            include_archived_projects: query.project_id.is_some(),
            ..TodoFilter::default()
        },
    )
    .await?;
    let events = repo::events::list_status_events(&mut conn).await?;
    Ok(gantt::build(
        projects,
        &todos,
        &events,
        GanttFilter {
            include_done: query.include_done,
            include_cancelled: query.include_cancelled,
        },
        today,
        &Rules::from_settings(&settings),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::domain::gantt::DaySpan;
    use crate::domain::testing::d;
    use crate::repo::db;

    async fn seeded() -> SqlitePool {
        let pool = db::open_in_memory().await.unwrap();
        crate::sample::seed_if_empty(&pool, d("2026-11-19"))
            .await
            .unwrap();
        pool
    }

    fn q(include_done: bool, include_cancelled: bool) -> GanttQuery {
        GanttQuery {
            project_id: None,
            include_done,
            include_cancelled,
        }
    }

    #[tokio::test]
    async fn sample_rows_match_the_mock() {
        let pool = seeded().await;
        let g = get(&pool, &q(true, false), d("2026-11-19")).await.unwrap();
        let shape: Vec<(String, usize)> = g
            .projects
            .iter()
            .map(|p| (p.project.name.clone(), p.rows.len()))
            .collect();
        // Done shown, cancelled hidden: 5 / 3 / 2 rows as in the mock.
        assert_eq!(
            shape,
            vec![
                ("基幹システム更改".to_owned(), 5),
                ("社内ツール整備".to_owned(), 3),
                ("採用サイト改修".to_owned(), 2)
            ]
        );

        let find = |title: &str| {
            g.projects
                .iter()
                .flat_map(|p| p.rows.iter())
                .find(|r| r.item.todo.title == title)
                .unwrap()
                .clone()
        };
        let span = |a: &str, b: &str| DaySpan {
            start: d(a),
            end: d(b),
        };

        let draft = find("移行手順書のドラフト");
        assert_eq!(draft.planned, Some(span("2026-11-11", "2026-11-18")));
        assert_eq!(draft.actual, Some(span("2026-11-12", "2026-11-19")));
        assert!(draft.waiting.is_empty());

        let api = find("API設計書のレビュー対応");
        assert_eq!(api.actual, Some(span("2026-11-17", "2026-11-19")));
        assert_eq!(api.waiting, vec![span("2026-11-19", "2026-11-19")]);

        let recruit = find("募集要項ページの原稿確認");
        assert_eq!(recruit.actual, Some(span("2026-11-16", "2026-11-19")));
        assert_eq!(recruit.waiting, vec![span("2026-11-17", "2026-11-19")]);

        let license = find("ライセンス棚卸し");
        assert_eq!(license.actual, None);
        assert_eq!(license.item.assessment.days_until_due, Some(-2));
    }

    #[tokio::test]
    async fn filters_by_project_and_closed_statuses() {
        let pool = seeded().await;
        let today = d("2026-11-19");
        let all = get(&pool, &q(true, true), today).await.unwrap();
        assert_eq!(all.projects[0].rows.len(), 6);
        let open_only = get(&pool, &q(false, false), today).await.unwrap();
        let rows: usize = open_only.projects.iter().map(|p| p.rows.len()).sum();
        assert_eq!(rows, 8);

        let pid = all.projects[2].project.id;
        let one = get(
            &pool,
            &GanttQuery {
                project_id: Some(pid),
                include_done: true,
                include_cancelled: false,
            },
            today,
        )
        .await
        .unwrap();
        assert_eq!(one.projects.len(), 1);
        assert_eq!(one.projects[0].project.name, "採用サイト改修");

        let missing = GanttQuery {
            project_id: Some(999),
            include_done: true,
            include_cancelled: false,
        };
        assert_eq!(
            get(&pool, &missing, today).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
    }
}
