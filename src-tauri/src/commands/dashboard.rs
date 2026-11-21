//! ダッシュボード: get_dashboard, get_startup_reminder, ack_reminder.

use std::path::Path;

use chrono::NaiveDate;
use sqlx::{SqliteConnection, SqlitePool};
use tauri::State;

use super::backup::DataDir;
use super::clock;
use super::error::CmdResult;
use super::todos::all_links;
use crate::backup;
use crate::domain::backup_mirror::{self, BackupMirrorAlert};
use crate::domain::dashboard::{self, Dashboard, StartupReminder};
use crate::domain::model::{Project, Todo};
use crate::domain::settings::Settings;
use crate::domain::urgency::Rules;
use crate::domain::views::Links;
use crate::repo;
use crate::repo::todos::TodoFilter;

#[tauri::command]
#[specta::specta]
pub async fn get_dashboard(
    pool: State<'_, SqlitePool>,
    data_dir: State<'_, DataDir>,
) -> CmdResult<Dashboard> {
    with_backup_alert(&pool, &data_dir.0, clock::today()).await
}

/// The reminder to show at startup (or when the date changed while the app
/// was open), or null when there is nothing to show.
#[tauri::command]
#[specta::specta]
pub async fn get_startup_reminder(
    pool: State<'_, SqlitePool>,
) -> CmdResult<Option<StartupReminder>> {
    reminder(&pool, clock::today()).await
}

/// Records that today's reminder was seen (closing the dialog).
#[tauri::command]
#[specta::specta]
pub async fn ack_reminder(pool: State<'_, SqlitePool>) -> CmdResult<()> {
    ack(&pool, clock::today()).await
}

/// The active projects and their TODOs, without the projects `hidden`
/// (higiri-cli leaves out the projects it does not show, v0.5.0).
async fn active(
    conn: &mut SqliteConnection,
    hidden: &[i64],
) -> CmdResult<(Vec<Project>, Vec<Todo>, Links)> {
    let mut projects = repo::projects::list(conn, false).await?;
    let mut todos = repo::todos::list(conn, &TodoFilter::default()).await?;
    if !hidden.is_empty() {
        projects.retain(|p| !hidden.contains(&p.id));
        todos.retain(|t| !hidden.contains(&t.project_id));
    }
    let links = all_links(conn).await?;
    Ok((projects, todos, links))
}

/// The dashboard without the backup mirror's notice (the resident timer's
/// counts).
pub async fn get(pool: &SqlitePool, today: NaiveDate) -> CmdResult<Dashboard> {
    Ok(load(pool, today, &[]).await?.0)
}

/// The dashboard as the screen shows it, with the backup mirror's notice
/// (from the daily backups in `data_dir`).
pub async fn with_backup_alert(
    pool: &SqlitePool,
    data_dir: &Path,
    today: NaiveDate,
) -> CmdResult<Dashboard> {
    without_projects(pool, data_dir, today, &[]).await
}

/// [`with_backup_alert`] as if the projects `hidden` were not there: their
/// TODOs are left out before anything is counted (higiri-cli's `today`).
pub async fn without_projects(
    pool: &SqlitePool,
    data_dir: &Path,
    today: NaiveDate,
    hidden: &[i64],
) -> CmdResult<Dashboard> {
    let (mut dash, settings) = load(pool, today, hidden).await?;
    dash.backup_mirror_alert = mirror_alert(data_dir, &settings);
    Ok(dash)
}

async fn load(
    pool: &SqlitePool,
    today: NaiveDate,
    hidden: &[i64],
) -> CmdResult<(Dashboard, Settings)> {
    let mut conn = pool.acquire().await?;
    let settings = repo::settings::load(&mut conn).await?;
    let (projects, todos, links) = active(&mut conn, hidden).await?;
    let dash = dashboard::build(
        projects,
        &todos,
        &links,
        today,
        &Rules::from_settings(&settings),
    );
    Ok((dash, settings))
}

/// 3 or more daily backups after the last one that reached the mirror. A
/// backups folder that cannot be read gives no notice (and a log line).
fn mirror_alert(data_dir: &Path, settings: &Settings) -> Option<BackupMirrorAlert> {
    settings.backup_mirror_dir.as_ref()?;
    let daily = backup::daily_dates(data_dir)
        .inspect_err(|e| log::error!("backups could not be listed: {}", e.kind()))
        .ok()?;
    backup_mirror::alert(true, settings.backup_mirror_last_ok, &daily)
}

pub async fn reminder(pool: &SqlitePool, today: NaiveDate) -> CmdResult<Option<StartupReminder>> {
    let mut conn = pool.acquire().await?;
    let settings = repo::settings::load(&mut conn).await?;
    if !dashboard::reminder_allowed(&settings, today) {
        return Ok(None);
    }
    let (projects, todos, links) = active(&mut conn, &[]).await?;
    let dash = dashboard::build(
        projects,
        &todos,
        &links,
        today,
        &Rules::from_settings(&settings),
    );
    Ok(dashboard::startup_reminder(
        &settings,
        dash.attention,
        today,
    ))
}

pub async fn ack(pool: &SqlitePool, today: NaiveDate) -> CmdResult<()> {
    let mut conn = pool.acquire().await?;
    repo::settings::set_last_reminded_on(&mut conn, Some(today)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::{self, SettingsInput};
    use crate::domain::settings::ReminderFrequency;
    use crate::domain::testing::d;
    use crate::repo::db;

    async fn seeded() -> SqlitePool {
        let pool = db::open_in_memory().await.unwrap();
        crate::sample::seed_if_empty(&pool, d("2026-11-19"))
            .await
            .unwrap();
        pool
    }

    fn frequency(f: ReminderFrequency) -> SettingsInput {
        SettingsInput {
            reminder_frequency: f,
            ..SettingsInput::from(&crate::domain::settings::Settings::default())
        }
    }

    #[tokio::test]
    async fn dashboard_of_the_sample_matches_the_mock() {
        let pool = seeded().await;
        let dash = get(&pool, d("2026-11-19")).await.unwrap();
        assert_eq!(dash.attention_total, 5);
        assert_eq!(dash.today, d("2026-11-19"));
        assert_eq!(dash.today_work.len(), 2);
        assert_eq!(dash.waiting.len(), 2);
        assert_eq!(dash.projects.len(), 3);
    }

    #[tokio::test]
    async fn business_days_follow_the_setting() {
        let pool = seeded().await;
        settings::update(
            &pool,
            SettingsInput {
                day_count: crate::domain::settings::DayCount::Business,
                ..SettingsInput::from(&crate::domain::settings::Settings::default())
            },
        )
        .await
        .unwrap();
        // Thursday 11/19: the mock's five, now counted in business days.
        let dash = get(&pool, d("2026-11-19")).await.unwrap();
        assert!(dash.business_days);
        assert_eq!(dash.attention_total, 5);
        // Friday 11/20 seen on Saturday 11/21: 移行手順書 (due 11/19) is
        // 1営業日超過, API設計書 (due 11/20) too.
        let dash = get(&pool, d("2026-11-21")).await.unwrap();
        let overdue: Vec<(&str, Option<i64>)> = dash
            .attention
            .iter()
            .filter(|i| i.assessment.urgency == Some(crate::domain::urgency::Urgency::Overdue))
            .map(|i| (i.todo.title.as_str(), i.assessment.days_until_due))
            .collect();
        assert_eq!(
            overdue,
            vec![
                ("ライセンス棚卸し", Some(-3)),
                ("移行手順書のドラフト", Some(-1)),
                ("API設計書のレビュー対応", Some(-1)),
            ]
        );
    }

    #[tokio::test]
    async fn every_launch_shows_the_reminder_after_ack() {
        let pool = seeded().await;
        let today = d("2026-11-19");
        assert_eq!(
            reminder(&pool, today).await.unwrap().unwrap().items.len(),
            5
        );
        ack(&pool, today).await.unwrap();
        assert!(reminder(&pool, today).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn once_per_day_shows_it_until_acknowledged_today() {
        let pool = seeded().await;
        settings::update(&pool, frequency(ReminderFrequency::OncePerDay))
            .await
            .unwrap();
        let today = d("2026-11-19");
        assert!(reminder(&pool, today).await.unwrap().is_some());
        ack(&pool, today).await.unwrap();
        assert_eq!(reminder(&pool, today).await.unwrap(), None);
        // The next day it comes back.
        assert!(reminder(&pool, d("2026-11-20")).await.unwrap().is_some());
        assert_eq!(
            settings::get(&pool).await.unwrap().last_reminded_on,
            Some(today)
        );
    }

    #[tokio::test]
    async fn nothing_to_remind_on_an_empty_database() {
        let pool = db::open_in_memory().await.unwrap();
        assert_eq!(reminder(&pool, d("2026-11-19")).await.unwrap(), None);
        let dash = get(&pool, d("2026-11-19")).await.unwrap();
        assert_eq!(dash.attention_total, 0);
    }

    #[tokio::test]
    async fn the_mirror_notice_comes_after_three_missed_daily_backups() {
        use crate::domain::backup_mirror::BackupMirrorAlert;

        let pool = seeded().await;
        let tmp = tempfile::tempdir().unwrap();
        let backups = backup::dir(tmp.path());
        std::fs::create_dir_all(&backups).unwrap();
        let put = |day: &str| std::fs::write(backups.join(format!("todo-{day}.db")), "").unwrap();
        for day in ["2026-11-16", "2026-11-17", "2026-11-18"] {
            put(day);
        }
        let today = d("2026-11-19");
        // No mirror: no notice, and get() never has one.
        assert_eq!(
            with_backup_alert(&pool, tmp.path(), today)
                .await
                .unwrap()
                .backup_mirror_alert,
            None
        );
        {
            let mut c = pool.acquire().await.unwrap();
            repo::settings::set_backup_mirror(
                &mut c,
                Some(&tmp.path().join("usb").display().to_string()),
                Some(d("2026-11-16")),
            )
            .await
            .unwrap();
        }
        // 11/17 and 11/18 missed: not yet.
        assert_eq!(
            with_backup_alert(&pool, tmp.path(), today)
                .await
                .unwrap()
                .backup_mirror_alert,
            None
        );
        put("2026-11-19");
        let dash = with_backup_alert(&pool, tmp.path(), today).await.unwrap();
        assert_eq!(
            dash.backup_mirror_alert,
            Some(BackupMirrorAlert {
                last_ok: Some(d("2026-11-16")),
                days: 3
            })
        );
        // The rest of the dashboard is the same as without it.
        assert_eq!(dash.attention_total, 5);
        assert_eq!(get(&pool, today).await.unwrap().backup_mirror_alert, None);
    }
}
