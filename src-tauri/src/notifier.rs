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
//! 3. copies the daily backups to the backup mirror (v0.4.0, C-3) when the
//!    newest one has not reached it ([`mirror_if_behind`]): right after the
//!    startup's backup (the first wake), after a new day's backup, and again
//!    at every wake while the mirror cannot be written (a drive plugged back
//!    in catches up the same day);
//! 4. when a notification time is due ([`tick`]): counts 日限注意 with the
//!    dashboard's own count, notifies when there is at least one, and saves
//!    `last_notified_at` either way, so a time with none is not notified later.
//!
//! The settings it needs ([`NotifySettings`], [`MirrorSettings`]) are kept in
//! memory by [`Notifier`], so a wake that only reads the clock never touches
//! the database: only a due time (the count and `last_notified_at`), a new
//! day (the backup) and a copy to the mirror (`backup_mirror_last_ok`) do.
//! Saving the settings, changing the mirror or restoring a backup hands the
//! new values over with [`Notifier::update`], which also wakes the task so it
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
use crate::domain::backup_mirror;
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

/// What the timer needs to keep the backup mirror in step (v0.4.0), and what
/// it logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorSettings {
    /// The mirror folder (none: no mirror).
    pub dir: Option<PathBuf>,
    /// The newest daily backup that reached it.
    pub last_ok: Option<NaiveDate>,
    /// The day a failure was last logged (one line a day).
    failure_logged_on: Option<NaiveDate>,
    /// The last copy failed (the next one that works is logged).
    failing: bool,
}

impl From<&Settings> for MirrorSettings {
    fn from(s: &Settings) -> MirrorSettings {
        MirrorSettings {
            dir: s.backup_mirror_dir.as_ref().map(PathBuf::from),
            last_ok: s.backup_mirror_last_ok,
            failure_logged_on: None,
            failing: false,
        }
    }
}

impl MirrorSettings {
    /// Takes saved settings. The same folder keeps what was logged, and an
    /// older day (read before the timer saved its own) does not replace a
    /// newer one; another folder (or none) starts afresh.
    fn take(&mut self, settings: &Settings) {
        let next = MirrorSettings::from(settings);
        if next.dir == self.dir {
            self.last_ok = self.last_ok.max(next.last_ok);
        } else {
            *self = next;
        }
    }
}

/// The resident timer's settings and its wake-up (managed state).
pub struct Notifier {
    wake: Arc<Notify>,
    settings: Arc<Mutex<NotifySettings>>,
    mirror: Arc<Mutex<MirrorSettings>>,
}

impl Notifier {
    /// Starts with the settings read at startup.
    pub fn new(settings: &Settings) -> Notifier {
        Notifier {
            wake: Arc::default(),
            settings: Arc::new(Mutex::new(NotifySettings::from(settings))),
            mirror: Arc::new(Mutex::new(MirrorSettings::from(settings))),
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
        lock(&self.mirror).take(settings);
        self.wake.notify_one();
    }

    /// A copy of the settings in memory.
    pub fn settings(&self) -> NotifySettings {
        lock(&self.settings).clone()
    }

    /// A copy of the mirror's settings in memory.
    pub fn mirror(&self) -> MirrorSettings {
        lock(&self.mirror).clone()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
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

/// What one wake did about the backup mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirrored {
    /// No mirror, or the newest daily backup is already there.
    InStep,
    /// Copied; the newest daily backup on the mirror now.
    Copied(Option<NaiveDate>),
    /// The mirror could not be written (tried again at the next wake).
    Failed,
}

/// Copies the daily backups to the mirror when the newest one has not
/// reached it. Reads only the backups folder to decide (never the database);
/// a copy that worked saves `backup_mirror_last_ok` (unless the mirror was
/// changed meanwhile). The log says when it failed (once a day) and when it
/// works again, without the folder's path.
pub async fn mirror_if_behind(
    pool: &SqlitePool,
    data_dir: &Path,
    shared: &Mutex<MirrorSettings>,
    today: NaiveDate,
) -> Mirrored {
    let current = lock(shared).clone();
    let Some(dir) = current.dir.clone() else {
        return Mirrored::InStep;
    };
    let daily = match backup::daily_dates(data_dir) {
        Ok(daily) => daily,
        Err(e) => {
            log::error!("backups could not be listed: {}", e.kind());
            return Mirrored::InStep;
        }
    };
    if !backup_mirror::behind(current.last_ok, &daily) {
        return Mirrored::InStep;
    }
    match backup::mirror(data_dir.to_owned(), dir.clone()).await {
        Ok(last_ok) => {
            let saved = async {
                let mut conn = pool.acquire().await?;
                let dir = dir.to_string_lossy();
                repo::settings::set_backup_mirror_last_ok(&mut conn, &dir, last_ok).await
            }
            .await;
            if let Err(e) = saved {
                log::error!("could not save backup_mirror_last_ok: {e}");
            }
            let mut m = lock(shared);
            if m.dir.as_ref() == Some(&dir) {
                m.last_ok = m.last_ok.max(last_ok);
                if m.failing {
                    log::info!("backup mirror works again; daily backups copied");
                } else {
                    log::info!("daily backups copied to the backup mirror");
                }
                m.failing = false;
            }
            Mirrored::Copied(last_ok)
        }
        Err(e) => {
            let mut m = lock(shared);
            if m.dir.as_ref() == Some(&dir) {
                if m.failure_logged_on != Some(today) {
                    log::warn!("backup mirror could not be written: {}", e.kind());
                    m.failure_logged_on = Some(today);
                }
                m.failing = true;
            }
            Mirrored::Failed
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
/// startup already took. Its first wake comes at once, so the startup's
/// backup reaches the mirror right after the window opens (a slow drive does
/// not hold up the startup).
pub fn spawn<R: Runtime>(
    app: AppHandle<R>,
    notifier: &Notifier,
    pool: SqlitePool,
    data_dir: PathBuf,
    backup_day: NaiveDate,
) {
    let wake = Arc::clone(&notifier.wake);
    let shared = Arc::clone(&notifier.settings);
    let mirror = Arc::clone(&notifier.mirror);
    tauri::async_runtime::spawn(async move {
        let mut backup_day = backup_day;
        loop {
            let now = clock::now();
            let zone = clock::zone();
            let today = now.with_timezone(&zone).date_naive();
            backup_if_new_day(&pool, &data_dir, &mut backup_day, today).await;
            mirror_if_behind(&pool, &data_dir, &mirror, today).await;
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

    // ---- バックアップの複製先 (v0.4.0) ----

    /// A data folder with daily backup files (contents do not matter here),
    /// an in-memory database whose mirror is `mirror` (mirrored up to
    /// `last_ok`), and the timer's state read from it.
    async fn mirrored(
        data: &Path,
        days: &[&str],
        mirror: &Path,
        last_ok: Option<NaiveDate>,
    ) -> (SqlitePool, Notifier) {
        let backups = backup::dir(data);
        std::fs::create_dir_all(&backups).unwrap();
        for day in days {
            std::fs::write(backups.join(format!("todo-{day}.db")), day).unwrap();
        }
        let pool = db::open_in_memory().await.unwrap();
        {
            let mut c = pool.acquire().await.unwrap();
            repo::settings::set_backup_mirror(&mut c, Some(&mirror.display().to_string()), last_ok)
                .await
                .unwrap();
        }
        let n = Notifier::new(&settings::get(&pool).await.unwrap());
        (pool, n)
    }

    #[tokio::test]
    async fn the_newest_daily_backup_reaches_the_mirror_once() {
        let data = tempfile::tempdir().unwrap();
        let usb = tempfile::tempdir().unwrap();
        let (pool, n) = mirrored(
            data.path(),
            &["2026-11-18", "2026-11-19"],
            usb.path(),
            Some(d("2026-11-18")),
        )
        .await;
        let today = d("2026-11-19");
        assert_eq!(
            mirror_if_behind(&pool, data.path(), &n.mirror, today).await,
            Mirrored::Copied(Some(today))
        );
        assert!(usb.path().join("todo-2026-11-19.db").is_file());
        assert!(usb.path().join("todo-2026-11-18.db").is_file());
        // Saved and kept in memory.
        assert_eq!(
            settings::get(&pool).await.unwrap().backup_mirror_last_ok,
            Some(today)
        );
        assert_eq!(n.mirror().last_ok, Some(today));

        // In step: the next wakes neither copy nor read the database.
        std::fs::remove_file(usb.path().join("todo-2026-11-18.db")).unwrap();
        pool.close().await;
        assert_eq!(
            mirror_if_behind(&pool, data.path(), &n.mirror, today).await,
            Mirrored::InStep
        );
        assert!(!usb.path().join("todo-2026-11-18.db").exists());
    }

    #[tokio::test]
    async fn a_mirror_that_is_not_there_is_tried_again_at_every_wake() {
        let data = tempfile::tempdir().unwrap();
        let usb = tempfile::tempdir().unwrap();
        let drive = usb.path().join("USB");
        let (pool, n) = mirrored(
            data.path(),
            &["2026-11-18", "2026-11-19"],
            &drive,
            Some(d("2026-11-18")),
        )
        .await;
        let today = d("2026-11-19");
        for _ in 0..3 {
            assert_eq!(
                mirror_if_behind(&pool, data.path(), &n.mirror, today).await,
                Mirrored::Failed
            );
        }
        // Logged once today; nothing saved, the backups folder as it was.
        let m = n.mirror();
        assert_eq!(m.failure_logged_on, Some(today));
        assert!(m.failing);
        assert_eq!(m.last_ok, Some(d("2026-11-18")));
        assert_eq!(
            settings::get(&pool).await.unwrap().backup_mirror_last_ok,
            Some(d("2026-11-18"))
        );
        assert_eq!(backup::daily_dates(data.path()).unwrap().len(), 2);
        assert!(!drive.exists());

        // The drive is plugged back in: it catches up.
        std::fs::create_dir(&drive).unwrap();
        assert_eq!(
            mirror_if_behind(&pool, data.path(), &n.mirror, today).await,
            Mirrored::Copied(Some(today))
        );
        assert!(!n.mirror().failing);
        assert_eq!(
            settings::get(&pool).await.unwrap().backup_mirror_last_ok,
            Some(today)
        );
    }

    #[tokio::test]
    async fn no_mirror_nothing_to_do() {
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(backup::dir(data.path())).unwrap();
        std::fs::write(backup::dir(data.path()).join("todo-2026-11-19.db"), "").unwrap();
        let pool = db::open_in_memory().await.unwrap();
        let n = Notifier::new(&Settings::default());
        assert_eq!(
            mirror_if_behind(&pool, data.path(), &n.mirror, d("2026-11-19")).await,
            Mirrored::InStep
        );
    }

    #[tokio::test]
    async fn a_mirror_changed_while_copying_keeps_the_new_one() {
        let data = tempfile::tempdir().unwrap();
        let usb = tempfile::tempdir().unwrap();
        let (pool, n) = mirrored(data.path(), &["2026-11-19"], usb.path(), None).await;
        // Chosen elsewhere meanwhile (as choose_backup_mirror saves it).
        let other = tempfile::tempdir().unwrap();
        {
            let mut c = pool.acquire().await.unwrap();
            repo::settings::set_backup_mirror(
                &mut c,
                Some(&other.path().display().to_string()),
                Some(d("2026-11-17")),
            )
            .await
            .unwrap();
        }
        mirror_if_behind(&pool, data.path(), &n.mirror, d("2026-11-19")).await;
        let s = settings::get(&pool).await.unwrap();
        assert_eq!(
            s.backup_mirror_dir,
            Some(other.path().display().to_string())
        );
        assert_eq!(s.backup_mirror_last_ok, Some(d("2026-11-17")));
    }

    #[test]
    fn saved_mirror_settings_take_effect() {
        let n = Notifier::new(&Settings::default());
        assert_eq!(n.mirror().dir, None);
        let usb = Settings {
            backup_mirror_dir: Some("/Volumes/USB".into()),
            backup_mirror_last_ok: Some(d("2026-11-18")),
            ..Settings::default()
        };
        n.update(&usb);
        assert_eq!(n.mirror().dir, Some(PathBuf::from("/Volumes/USB")));
        assert_eq!(n.mirror().last_ok, Some(d("2026-11-18")));
        // The timer copied 11/19 meanwhile; settings read before that keep it.
        lock(&n.mirror).last_ok = Some(d("2026-11-19"));
        n.update(&usb);
        assert_eq!(n.mirror().last_ok, Some(d("2026-11-19")));
        // Another folder starts with its own day; none clears it.
        n.update(&Settings {
            backup_mirror_dir: Some("/Volumes/Other".into()),
            backup_mirror_last_ok: Some(d("2026-11-17")),
            ..Settings::default()
        });
        assert_eq!(n.mirror().last_ok, Some(d("2026-11-17")));
        n.update(&Settings::default());
        assert_eq!(n.mirror().dir, None);
        assert_eq!(n.mirror().last_ok, None);
    }
}
