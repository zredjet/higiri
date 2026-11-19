//! プロジェクト: list_projects, create_project, update_project,
//! archive_project, delete_project.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use sqlx::SqlitePool;
use tauri::State;

use super::clock;
use super::error::CmdResult;
use super::validate;
use crate::domain::model::Project;
use crate::domain::views::{ProjectSummary, project_summaries};
use crate::repo;
use crate::repo::projects::{NewProject, ProjectEdit};
use crate::repo::todos::TodoFilter;

#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ProjectInput {
    pub name: String,
    /// `#RRGGBB`. When creating, `null` uses the default colour; when
    /// updating, `null` keeps the current colour.
    pub color: Option<String>,
    pub description: String,
}

// ---- tauri commands ----------------------------------------------------------

/// Projects with their counts. Archived ones only when `include_archived`.
#[tauri::command]
#[specta::specta]
pub async fn list_projects(
    pool: State<'_, SqlitePool>,
    include_archived: bool,
) -> CmdResult<Vec<ProjectSummary>> {
    list(&pool, include_archived, clock::today()).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_project(
    pool: State<'_, SqlitePool>,
    input: ProjectInput,
) -> CmdResult<Project> {
    create(&pool, input, clock::now()).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_project(
    pool: State<'_, SqlitePool>,
    id: i64,
    input: ProjectInput,
) -> CmdResult<Project> {
    update(&pool, id, input, clock::now()).await
}

/// Archives (`archived = true`) or restores (`false`) a project.
#[tauri::command]
#[specta::specta]
pub async fn archive_project(
    pool: State<'_, SqlitePool>,
    id: i64,
    archived: bool,
) -> CmdResult<Project> {
    archive(&pool, id, archived, clock::now()).await
}

/// Deletes a project with all its TODOs and records. The screen confirms first.
#[tauri::command]
#[specta::specta]
pub async fn delete_project(pool: State<'_, SqlitePool>, id: i64) -> CmdResult<()> {
    delete(&pool, id).await
}

// ---- implementation ----------------------------------------------------------

pub async fn list(
    pool: &SqlitePool,
    include_archived: bool,
    today: NaiveDate,
) -> CmdResult<Vec<ProjectSummary>> {
    let mut conn = pool.acquire().await?;
    let settings = repo::settings::load(&mut conn).await?;
    let projects = repo::projects::list(&mut conn, include_archived).await?;
    let todos = repo::todos::list(
        &mut conn,
        &TodoFilter {
            include_archived_projects: include_archived,
            ..TodoFilter::default()
        },
    )
    .await?;
    Ok(project_summaries(
        projects,
        &todos,
        today,
        settings.soon_days,
    ))
}

pub async fn create(
    pool: &SqlitePool,
    input: ProjectInput,
    now: DateTime<Utc>,
) -> CmdResult<Project> {
    let new = NewProject {
        name: validate::required_text(&input.name, "プロジェクト名")?,
        color: input.color.as_deref().map(validate::color).transpose()?,
        description: input.description.trim().to_owned(),
    };
    let mut conn = pool.acquire().await?;
    Ok(repo::projects::insert(&mut conn, &new, now).await?)
}

pub async fn update(
    pool: &SqlitePool,
    id: i64,
    input: ProjectInput,
    now: DateTime<Utc>,
) -> CmdResult<Project> {
    let name = validate::required_text(&input.name, "プロジェクト名")?;
    let color = input.color.as_deref().map(validate::color).transpose()?;
    let mut tx = pool.begin().await?;
    let current = repo::projects::get(&mut tx, id).await?;
    let edit = ProjectEdit {
        name,
        color: color.unwrap_or(current.color),
        description: input.description.trim().to_owned(),
    };
    let saved = repo::projects::update(&mut tx, id, &edit, now).await?;
    tx.commit().await?;
    Ok(saved)
}

pub async fn archive(
    pool: &SqlitePool,
    id: i64,
    archived: bool,
    now: DateTime<Utc>,
) -> CmdResult<Project> {
    let mut conn = pool.acquire().await?;
    let at = archived.then_some(now);
    Ok(repo::projects::set_archived(&mut conn, id, at, now).await?)
}

pub async fn delete(pool: &SqlitePool, id: i64) -> CmdResult<()> {
    let mut conn = pool.acquire().await?;
    Ok(repo::projects::delete(&mut conn, id).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    fn input(name: &str, color: Option<&str>) -> ProjectInput {
        ProjectInput {
            name: name.into(),
            color: color.map(str::to_owned),
            description: "  説明  ".into(),
        }
    }

    #[tokio::test]
    async fn create_validates_and_trims() {
        let pool = db::open_in_memory().await.unwrap();
        let now = ts(2026, 11, 19, 0, 0);
        let p = create(&pool, input(" 基幹システム更改 ", None), now)
            .await
            .unwrap();
        assert_eq!(p.name, "基幹システム更改");
        assert_eq!(p.description, "説明");
        assert_eq!(p.color, "#3B6FD4");

        let e = create(&pool, input("  ", None), now).await.unwrap_err();
        assert_eq!(e.kind, ErrorKind::Validation);
        assert_eq!(e.message, "プロジェクト名を入力してください。");
        assert!(create(&pool, input("x", Some("blue")), now).await.is_err());
    }

    #[tokio::test]
    async fn update_keeps_colour_when_not_given() {
        let pool = db::open_in_memory().await.unwrap();
        let now = ts(2026, 11, 19, 0, 0);
        let p = create(&pool, input("A", Some("#2e8b57")), now)
            .await
            .unwrap();
        let u = update(&pool, p.id, input("B", None), now).await.unwrap();
        assert_eq!((u.name.as_str(), u.color.as_str()), ("B", "#2E8B57"));
        let e = update(&pool, 999, input("B", None), now).await.unwrap_err();
        assert_eq!(e.kind, ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn archive_hides_from_default_list_and_restores() {
        let pool = db::open_in_memory().await.unwrap();
        let now = ts(2026, 11, 19, 0, 0);
        let today = d("2026-11-19");
        let a = create(&pool, input("A", None), now).await.unwrap();
        create(&pool, input("B", None), now).await.unwrap();

        let archived = archive(&pool, a.id, true, now).await.unwrap();
        assert_eq!(archived.archived_at, Some(now));
        assert_eq!(list(&pool, false, today).await.unwrap().len(), 1);
        assert_eq!(list(&pool, true, today).await.unwrap().len(), 2);

        archive(&pool, a.id, false, now).await.unwrap();
        assert_eq!(list(&pool, false, today).await.unwrap().len(), 2);

        delete(&pool, a.id).await.unwrap();
        assert_eq!(list(&pool, true, today).await.unwrap().len(), 1);
        assert_eq!(
            delete(&pool, a.id).await.unwrap_err().kind,
            ErrorKind::NotFound
        );
    }
}
