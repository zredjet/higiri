//! The resident timer (v1.1: D-6, D-7, and C-1 while the app stays in the
//! tray).
//!
//! A tokio task that sleeps until the next notification time (at most
//! `domain::notify::MAX_WAIT`, so it reads the clock again after the PC slept)
//! and, each time it wakes:
//!
//! 1. reads the clock with `commands::clock`;
//! 2. takes the day's backup with `backup::run_daily` when the date changed
//!    since the last wake (one a day, the newest 7 kept, as at startup);
//! 3. when a notification time is due ([`tick`]): counts 日限注意 with the
//!    dashboard's own count, notifies when there is at least one, and saves
//!    `last_notified_at` either way, so a time with none is not notified later.
//!
//! Saving the settings (or restoring a backup) wakes the task through
//! [`Notifier::wake`], so it computes the wait again for the new times.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use sqlx::SqlitePool;
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Notify;

use crate::backup;
use crate::commands::error::CmdResult;
use crate::commands::{clock, dashboard};
use crate::domain::notify::{self, MAX_WAIT, Message};
use crate::repo;

/// Wakes the resident timer (managed state).
#[derive(Default)]
pub struct Notifier {
    wake: Arc<Notify>,
}

impl Notifier {
    /// Makes the timer compute its wait again (after the settings changed).
    /// A wake while it is busy is kept and takes effect when it next waits.
    pub fn wake(&self) {
        self.wake.notify_one();
    }
}

/// What one wake decided: the notification to show now, and how long to
/// sleep before the next wake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tick {
    pub message: Option<Message>,
    pub wait: Duration,
}

/// Handles the notification time due at `now`, if any. `tz` is the PC's time
/// zone (the times are local). Database only; showing the message is up to
/// the caller.
pub async fn tick<Tz: TimeZone>(pool: &SqlitePool, now: DateTime<Utc>, tz: &Tz) -> CmdResult<Tick> {
    let settings = {
        let mut conn = pool.acquire().await?;
        repo::settings::load(&mut conn).await?
    };
    let now_local = now.with_timezone(tz).naive_local();
    let times = notify::times_of(&settings.notify_times);
    let wait = notify::next_wake(now_local, &times);
    if !settings.notify_enabled {
        return Ok(Tick {
            message: None,
            wait,
        });
    }
    let last_local = settings
        .last_notified_at
        .map(|t| t.with_timezone(tz).naive_local());
    if notify::pending_slot(now_local, &times, last_local).is_none() {
        return Ok(Tick {
            message: None,
            wait,
        });
    }
    let dash = dashboard::get(pool, now_local.date()).await?;
    let mut conn = pool.acquire().await?;
    repo::settings::set_last_notified_at(&mut conn, Some(now)).await?;
    Ok(Tick {
        message: notify::message(&dash.counts),
        wait,
    })
}

/// Takes the day's backup when `today` is a new day since `last_day`. Returns
/// the new file. Failures are logged; the next attempt is the next new day
/// (or the next launch), as at startup.
pub async fn backup_if_new_day(
    pool: &SqlitePool,
    data_dir: &Path,
    last_day: &mut NaiveDate,
    today: NaiveDate,
) -> Option<PathBuf> {
    if today == *last_day {
        return None;
    }
    *last_day = today;
    match backup::run_daily(pool, data_dir, today).await {
        Ok(Some(file)) => {
            log::info!("backup written while resident: {}", file.display());
            Some(file)
        }
        Ok(None) => None,
        Err(e) => {
            log::error!("daily backup while resident failed: {e}");
            None
        }
    }
}

/// Shows an OS notification. Errors are logged by the callers that need it.
pub fn show<R: Runtime>(
    app: &AppHandle<R>,
    message: &Message,
) -> Result<(), tauri_plugin_notification::Error> {
    app.notification()
        .builder()
        .title(&message.title)
        .body(&message.body)
        .show()
}

/// Starts the resident timer. `backup_day` is the day whose backup the
/// startup already took.
pub fn spawn<R: Runtime>(
    app: AppHandle<R>,
    notifier: &Notifier,
    pool: SqlitePool,
    data_dir: PathBuf,
    backup_day: NaiveDate,
) {
    let wake = Arc::clone(&notifier.wake);
    tauri::async_runtime::spawn(async move {
        let mut backup_day = backup_day;
        loop {
            let now = clock::now();
            let zone = clock::zone();
            let today = now.with_timezone(&zone).date_naive();
            backup_if_new_day(&pool, &data_dir, &mut backup_day, today).await;
            let wait = match tick(&pool, now, &zone).await {
                Ok(Tick { message, wait }) => {
                    if let Some(m) = message {
                        match show(&app, &m) {
                            Ok(()) => log::info!("notification shown"),
                            Err(e) => log::error!("notification failed: {e}"),
                        }
                    }
                    wait
                }
                Err(e) => {
                    log::error!("notification check failed: {}", e.message);
                    MAX_WAIT
                }
            };
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = wake.notified() => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;
    use crate::commands::settings::{self, SettingsInput};
    use crate::domain::settings::Settings;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    /// Japan Standard Time: 09:00 JST is 00:00 UTC.
    fn jst() -> FixedOffset {
        FixedOffset::east_opt(9 * 3600).unwrap()
    }

    async fn sample() -> SqlitePool {
        let pool = db::open_in_memory().await.unwrap();
        crate::sample::seed_if_empty(&pool, d("2026-11-19"))
            .await
            .unwrap();
        pool
    }

    async fn last_notified(pool: &SqlitePool) -> Option<DateTime<Utc>> {
        settings::get(pool).await.unwrap().last_notified_at
    }

    #[tokio::test]
    async fn notifies_the_due_time_once_with_the_dashboard_counts() {
        let pool = sample().await;
        // 8:59 JST: nothing yet, wake at 9:00.
        let t = tick(&pool, ts(2026, 11, 18, 23, 59), &jst()).await.unwrap();
        assert_eq!(t.message, None);
        assert_eq!(t.wait, Duration::from_secs(60));
        assert_eq!(last_notified(&pool).await, None);

        // Started at 9:30 JST: the 9:00 time, with the mock's 5 日限注意.
        let at = ts(2026, 11, 19, 0, 30);
        let t = tick(&pool, at, &jst()).await.unwrap();
        let message = t.message.unwrap();
        assert_eq!(message.title, "日限注意が5件あります");
        assert_eq!(
            message.body,
            notify::message(&dashboard::get(&pool, d("2026-11-19")).await.unwrap().counts)
                .unwrap()
                .body
        );
        assert_eq!(last_notified(&pool).await, Some(at));

        // Woken again before 12:55: nothing more.
        let t = tick(&pool, ts(2026, 11, 19, 0, 40), &jst()).await.unwrap();
        assert_eq!(t.message, None);
        assert_eq!(t.wait, MAX_WAIT);
        // 12:55 JST.
        let t = tick(&pool, ts(2026, 11, 19, 3, 55), &jst()).await.unwrap();
        assert!(t.message.is_some());
    }

    #[tokio::test]
    async fn a_time_with_nothing_to_report_counts_as_notified() {
        let pool = db::open_in_memory().await.unwrap();
        let at = ts(2026, 11, 19, 0, 30);
        let t = tick(&pool, at, &jst()).await.unwrap();
        assert_eq!(t.message, None);
        assert_eq!(last_notified(&pool).await, Some(at));

        // TODOs added afterwards wait for the next time.
        crate::sample::seed_if_empty(&pool, d("2026-11-19"))
            .await
            .unwrap();
        let t = tick(&pool, ts(2026, 11, 19, 1, 0), &jst()).await.unwrap();
        assert_eq!(t.message, None);
        let t = tick(&pool, ts(2026, 11, 19, 3, 55), &jst()).await.unwrap();
        assert!(t.message.is_some());
    }

    #[tokio::test]
    async fn turned_off_notifies_nothing_and_keeps_the_record() {
        let pool = sample().await;
        settings::update(
            &pool,
            SettingsInput {
                notify_enabled: false,
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        let t = tick(&pool, ts(2026, 11, 19, 0, 30), &jst()).await.unwrap();
        assert_eq!(t.message, None);
        assert_eq!(last_notified(&pool).await, None);
    }

    #[tokio::test]
    async fn new_times_take_effect_on_the_next_tick() {
        let pool = sample().await;
        settings::update(
            &pool,
            SettingsInput {
                notify_times: vec!["17:30".into()],
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        // 9:30 JST: 9:00 is no longer a notification time.
        let t = tick(&pool, ts(2026, 11, 19, 0, 30), &jst()).await.unwrap();
        assert_eq!(t.message, None);
        // 17:25 JST -> wake at 17:30.
        let t = tick(&pool, ts(2026, 11, 19, 8, 25), &jst()).await.unwrap();
        assert_eq!(t.wait, Duration::from_secs(5 * 60));
        let t = tick(&pool, ts(2026, 11, 19, 8, 30), &jst()).await.unwrap();
        assert!(t.message.is_some());
    }

    #[tokio::test]
    async fn a_new_day_while_resident_takes_one_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = db::open(&tmp.path().join(db::DB_FILE_NAME)).await.unwrap();
        // The startup took 11/19's backup.
        backup::run_daily(&pool, tmp.path(), d("2026-11-19"))
            .await
            .unwrap();
        let mut day = d("2026-11-19");

        // Waking again the same day takes none.
        assert_eq!(
            backup_if_new_day(&pool, tmp.path(), &mut day, d("2026-11-19")).await,
            None
        );
        // After midnight: 11/20's backup.
        let file = backup_if_new_day(&pool, tmp.path(), &mut day, d("2026-11-20"))
            .await
            .unwrap();
        assert!(file.ends_with("todo-2026-11-20.db"));
        assert_eq!(day, d("2026-11-20"));
        // Later the same day: no second one.
        assert_eq!(
            backup_if_new_day(&pool, tmp.path(), &mut day, d("2026-11-20")).await,
            None
        );
        let names: Vec<String> = backup::list(tmp.path())
            .unwrap()
            .into_iter()
            .map(|b| b.file_name)
            .collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"todo-2026-11-20.db".to_owned()));
        assert!(names.contains(&"todo-2026-11-19.db".to_owned()));

        // A launch on 11/20 (backup already there) then a wake: still two.
        let mut day = d("2026-11-19");
        assert_eq!(
            backup_if_new_day(&pool, tmp.path(), &mut day, d("2026-11-20")).await,
            None
        );
        assert_eq!(backup::list(tmp.path()).unwrap().len(), 2);
    }
}
