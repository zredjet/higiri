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
//! The settings it needs ([`NotifySettings`]) are kept in memory by
//! [`Notifier`], so a wake that only reads the clock never touches the
//! database: only a due time (the count and `last_notified_at`) and a new day
//! (the backup) do. Saving the settings or restoring a backup hands the new
//! values over with [`Notifier::update`], which also wakes the task so it
//! computes the wait again.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
use sqlx::SqlitePool;
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Notify;

use crate::backup;
use crate::commands::error::CmdResult;
use crate::commands::{clock, dashboard};
use crate::domain::notify::{self, MAX_WAIT, Message};
use crate::domain::settings::Settings;
use crate::repo;

/// What the timer needs from the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifySettings {
    pub enabled: bool,
    /// Notification times, sorted.
    pub times: Vec<NaiveTime>,
    /// When the last notification time was handled.
    pub last_notified_at: Option<DateTime<Utc>>,
}

impl From<&Settings> for NotifySettings {
    fn from(s: &Settings) -> NotifySettings {
        NotifySettings {
            enabled: s.notify_enabled,
            times: notify::times_of(&s.notify_times),
            last_notified_at: s.last_notified_at,
        }
    }
}

impl NotifySettings {
    /// Records a handled time; an earlier one never replaces a later one.
    fn notified(&mut self, at: DateTime<Utc>) {
        self.last_notified_at = self.last_notified_at.max(Some(at));
    }
}

/// The resident timer's settings and its wake-up (managed state).
pub struct Notifier {
    wake: Arc<Notify>,
    settings: Arc<Mutex<NotifySettings>>,
}

impl Notifier {
    /// Starts with the settings read at startup.
    pub fn new(settings: &Settings) -> Notifier {
        Notifier {
            wake: Arc::default(),
            settings: Arc::new(Mutex::new(NotifySettings::from(settings))),
        }
    }

    /// Takes the saved settings (after the settings screen or a restore) and
    /// makes the timer compute its wait again. A `last_notified_at` older
    /// than the one in memory (read before the timer wrote its own) is not
    /// taken, so a time is never notified twice. A wake while the timer is
    /// busy is kept and takes effect when it next waits.
    pub fn update(&self, settings: &Settings) {
        {
            let mut current = lock(&self.settings);
            let last = current.last_notified_at;
            *current = NotifySettings::from(settings);
            if let Some(at) = last {
                current.notified(at);
            }
        }
        self.wake.notify_one();
    }

    /// A copy of the settings in memory.
    pub fn settings(&self) -> NotifySettings {
        lock(&self.settings).clone()
    }
}

fn lock(m: &Mutex<NotifySettings>) -> std::sync::MutexGuard<'_, NotifySettings> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What one wake decided: the notification to show now, how long to sleep
/// before the next wake, and the time saved as `last_notified_at` (when a
/// notification time was handled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tick {
    pub message: Option<Message>,
    pub wait: Duration,
    pub notified_at: Option<DateTime<Utc>>,
}

/// Handles the notification time due at `now`, if any, with the settings in
/// memory. `tz` is the PC's time zone (the times are local). The database is
/// used only when a time is due: the dashboard's count, and saving
/// `last_notified_at`. Showing the message is up to the caller.
pub async fn tick<Tz: TimeZone>(
    pool: &SqlitePool,
    settings: &NotifySettings,
    now: DateTime<Utc>,
    tz: &Tz,
) -> CmdResult<Tick> {
    let now_local = now.with_timezone(tz).naive_local();
    let wait = notify::next_wake(now_local, &settings.times);
    let nothing = Tick {
        message: None,
        wait,
        notified_at: None,
    };
    if !settings.enabled {
        return Ok(nothing);
    }
    let last_local = settings
        .last_notified_at
        .map(|t| t.with_timezone(tz).naive_local());
    if notify::pending_slot(now_local, &settings.times, last_local).is_none() {
        return Ok(nothing);
    }
    let dash = dashboard::get(pool, now_local.date()).await?;
    let mut conn = pool.acquire().await?;
    repo::settings::set_last_notified_at(&mut conn, Some(now)).await?;
    Ok(Tick {
        message: notify::message(&dash.counts),
        wait,
        notified_at: Some(now),
    })
}

/// One wake of the loop: [`tick`] with a copy of the settings in memory,
/// then records the handled time in memory too.
async fn check<Tz: TimeZone>(
    shared: &Mutex<NotifySettings>,
    pool: &SqlitePool,
    now: DateTime<Utc>,
    tz: &Tz,
) -> CmdResult<Tick> {
    let settings = lock(shared).clone();
    let t = tick(pool, &settings, now, tz).await?;
    if let Some(at) = t.notified_at {
        lock(shared).notified(at);
    }
    Ok(t)
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
    let shared = Arc::clone(&notifier.settings);
    tauri::async_runtime::spawn(async move {
        let mut backup_day = backup_day;
        loop {
            let now = clock::now();
            let zone = clock::zone();
            let today = now.with_timezone(&zone).date_naive();
            backup_if_new_day(&pool, &data_dir, &mut backup_day, today).await;
            let wait = match check(&shared, &pool, now, &zone).await {
                Ok(t) => {
                    if let Some(m) = t.message {
                        match show(&app, &m) {
                            Ok(()) => log::info!("notification shown"),
                            Err(e) => log::error!("notification failed: {e}"),
                        }
                    }
                    t.wait
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

    /// One wake of the loop with the timer's settings in memory.
    async fn wake(n: &Notifier, pool: &SqlitePool, now: DateTime<Utc>) -> Tick {
        check(&n.settings, pool, now, &jst()).await.unwrap()
    }

    #[tokio::test]
    async fn notifies_the_due_time_once_with_the_dashboard_counts() {
        let pool = sample().await;
        let n = Notifier::new(&Settings::default());
        // 8:59 JST: nothing yet, wake at 9:00.
        let t = wake(&n, &pool, ts(2026, 11, 18, 23, 59)).await;
        assert_eq!(t.message, None);
        assert_eq!(t.wait, Duration::from_secs(60));
        assert_eq!(t.notified_at, None);
        assert_eq!(last_notified(&pool).await, None);

        // Started at 9:30 JST: the 9:00 time, with the mock's 5 日限注意.
        let at = ts(2026, 11, 19, 0, 30);
        let t = wake(&n, &pool, at).await;
        let message = t.message.unwrap();
        assert_eq!(message.title, "日限注意が5件あります");
        assert_eq!(
            message.body,
            notify::message(&dashboard::get(&pool, d("2026-11-19")).await.unwrap().counts)
                .unwrap()
                .body
        );
        // Saved in the database and kept in memory.
        assert_eq!(t.notified_at, Some(at));
        assert_eq!(last_notified(&pool).await, Some(at));
        assert_eq!(n.settings().last_notified_at, Some(at));

        // Woken again before 12:55: nothing more.
        let t = wake(&n, &pool, ts(2026, 11, 19, 0, 40)).await;
        assert_eq!(t.message, None);
        assert_eq!(t.wait, MAX_WAIT);
        // 12:55 JST.
        let t = wake(&n, &pool, ts(2026, 11, 19, 3, 55)).await;
        assert!(t.message.is_some());
    }

    #[tokio::test]
    async fn a_time_with_nothing_to_report_counts_as_notified() {
        let pool = db::open_in_memory().await.unwrap();
        let n = Notifier::new(&Settings::default());
        let at = ts(2026, 11, 19, 0, 30);
        let t = wake(&n, &pool, at).await;
        assert_eq!(t.message, None);
        assert_eq!(t.notified_at, Some(at));
        assert_eq!(last_notified(&pool).await, Some(at));

        // TODOs added afterwards wait for the next time.
        crate::sample::seed_if_empty(&pool, d("2026-11-19"))
            .await
            .unwrap();
        let t = wake(&n, &pool, ts(2026, 11, 19, 1, 0)).await;
        assert_eq!(t.message, None);
        let t = wake(&n, &pool, ts(2026, 11, 19, 3, 55)).await;
        assert!(t.message.is_some());
    }

    #[tokio::test]
    async fn a_wake_with_nothing_due_does_not_touch_the_database() {
        let pool = sample().await;
        let n = Notifier::new(&Settings::default());
        wake(&n, &pool, ts(2026, 11, 19, 0, 30)).await;
        // From here on the database cannot be used at all.
        pool.close().await;
        for now in [ts(2026, 11, 19, 0, 40), ts(2026, 11, 19, 3, 50)] {
            let t = check(&n.settings, &pool, now, &jst()).await.unwrap();
            assert_eq!(t.message, None);
        }
        // Turned off: not even a due time reads it.
        n.update(&Settings {
            notify_enabled: false,
            ..Settings::default()
        });
        let t = check(&n.settings, &pool, ts(2026, 11, 19, 3, 55), &jst())
            .await
            .unwrap();
        assert_eq!(t.message, None);
        // A due time needs the count, so it does read it.
        n.update(&Settings::default());
        assert!(
            check(&n.settings, &pool, ts(2026, 11, 19, 3, 55), &jst())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn turned_off_notifies_nothing_and_keeps_the_record() {
        let pool = sample().await;
        let n = Notifier::new(&Settings {
            notify_enabled: false,
            ..Settings::default()
        });
        let t = wake(&n, &pool, ts(2026, 11, 19, 0, 30)).await;
        assert_eq!(t.message, None);
        assert_eq!(t.notified_at, None);
        assert_eq!(last_notified(&pool).await, None);
    }

    #[tokio::test]
    async fn saved_settings_take_effect_on_the_next_wake() {
        let pool = sample().await;
        let n = Notifier::new(&Settings::default());
        // As update_settings does: save, then hand the result over.
        let saved = settings::update(
            &pool,
            SettingsInput {
                notify_times: vec!["17:30".into()],
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        n.update(&saved);
        // 9:30 JST: 9:00 is no longer a notification time.
        let t = wake(&n, &pool, ts(2026, 11, 19, 0, 30)).await;
        assert_eq!(t.message, None);
        // 17:25 JST -> wake at 17:30.
        let t = wake(&n, &pool, ts(2026, 11, 19, 8, 25)).await;
        assert_eq!(t.wait, Duration::from_secs(5 * 60));
        let t = wake(&n, &pool, ts(2026, 11, 19, 8, 30)).await;
        assert!(t.message.is_some());
    }

    #[test]
    fn an_older_handled_time_never_replaces_a_newer_one() {
        let n = Notifier::new(&Settings::default());
        let mut s = n.settings();
        s.notified(ts(2026, 11, 19, 0, 30));
        *lock(&n.settings) = s;
        // Settings read before the timer saved its time.
        n.update(&Settings {
            last_notified_at: Some(ts(2026, 11, 18, 3, 55)),
            notify_times: vec!["08:00".into()],
            ..Settings::default()
        });
        let now = n.settings();
        assert_eq!(now.last_notified_at, Some(ts(2026, 11, 19, 0, 30)));
        assert_eq!(now.times, notify::times_of(&["08:00".to_owned()]));
        // A later one is taken.
        n.update(&Settings {
            last_notified_at: Some(ts(2026, 11, 19, 3, 55)),
            ..Settings::default()
        });
        assert_eq!(n.settings().last_notified_at, Some(ts(2026, 11, 19, 3, 55)));
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
