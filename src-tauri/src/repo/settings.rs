//! `settings` table: one row per key, value stored as JSON.
//!
//! | key | value (JSON) |
//! | --- | --- |
//! | `reminder_frequency` | `"every_launch"` or `"once_per_day"` |
//! | `soon_days` | number |
//! | `follow_up_days` | number |
//! | `last_reminded_on` | `"YYYY-MM-DD"` or `null` |
//! | `theme` | `"light"`, `"dark"` or `"system"` |
//! | `notify_enabled` | `true` / `false` |
//! | `notify_times` | `["HH:MM", …]` (1 to 5, distinct) |
//! | `last_notified_at` | `"2026-11-19T00:00:00.000Z"` (UTC) or `null` |
//! | `close_to_tray` | `true` / `false` |
//! | `autostart` | `true` / `false` |
//! | `tray_hint_shown` | `true` / `false` |
//!
//! Missing keys, unknown keys and unreadable values fall back to the defaults
//! so that a bad row never prevents the app from starting.
//!
//! `last_reminded_on`, `last_notified_at` and `tray_hint_shown` are written by
//! the app, one key at a time ([`set_last_reminded_on`] and the like), so that
//! saving the settings screen never puts back an older value.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::SqliteConnection;

use super::Result;
use crate::domain::date::{format_timestamp, parse_timestamp};
use crate::domain::notify::normalize_times;
use crate::domain::settings::Settings;

pub const REMINDER_FREQUENCY: &str = "reminder_frequency";
pub const SOON_DAYS: &str = "soon_days";
pub const FOLLOW_UP_DAYS: &str = "follow_up_days";
pub const LAST_REMINDED_ON: &str = "last_reminded_on";
pub const THEME: &str = "theme";
pub const NOTIFY_ENABLED: &str = "notify_enabled";
pub const NOTIFY_TIMES: &str = "notify_times";
pub const LAST_NOTIFIED_AT: &str = "last_notified_at";
pub const CLOSE_TO_TRAY: &str = "close_to_tray";
pub const AUTOSTART: &str = "autostart";
pub const TRAY_HINT_SHOWN: &str = "tray_hint_shown";

/// Keys a restore (and the JSON import) leaves as they are on this PC:
/// `autostart` belongs to the PC, the others are written by the app.
pub const KEPT_ON_REPLACE: [&str; 4] = [
    AUTOSTART,
    LAST_NOTIFIED_AT,
    TRAY_HINT_SHOWN,
    LAST_REMINDED_ON,
];

fn read<T: DeserializeOwned>(value: &str, fallback: T) -> T {
    serde_json::from_str(value).unwrap_or(fallback)
}

/// A stored list of times, or the fallback unless it is a valid list.
fn read_times(value: &str, fallback: Vec<String>) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value)
        .ok()
        .and_then(|times| normalize_times(&times).ok())
        .unwrap_or(fallback)
}

/// A stored UTC timestamp (or null), or the fallback when unreadable.
fn read_timestamp(value: &str, fallback: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    match serde_json::from_str::<Option<String>>(value) {
        Ok(None) => None,
        Ok(Some(s)) => parse_timestamp(&s).or(fallback),
        Err(_) => fallback,
    }
}

/// Reads all settings, applying defaults for anything missing or unreadable.
pub async fn load(conn: &mut SqliteConnection) -> Result<Settings> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(&mut *conn)
        .await?;
    let mut s = Settings::default();
    for (key, value) in rows {
        match key.as_str() {
            REMINDER_FREQUENCY => s.reminder_frequency = read(&value, s.reminder_frequency),
            SOON_DAYS => s.soon_days = read(&value, s.soon_days),
            FOLLOW_UP_DAYS => s.follow_up_days = read(&value, s.follow_up_days),
            LAST_REMINDED_ON => s.last_reminded_on = read(&value, s.last_reminded_on),
            THEME => s.theme = read(&value, s.theme),
            NOTIFY_ENABLED => s.notify_enabled = read(&value, s.notify_enabled),
            NOTIFY_TIMES => s.notify_times = read_times(&value, s.notify_times),
            LAST_NOTIFIED_AT => s.last_notified_at = read_timestamp(&value, s.last_notified_at),
            CLOSE_TO_TRAY => s.close_to_tray = read(&value, s.close_to_tray),
            AUTOSTART => s.autostart = read(&value, s.autostart),
            TRAY_HINT_SHOWN => s.tray_hint_shown = read(&value, s.tray_hint_shown),
            _ => {}
        }
    }
    Ok(s)
}

async fn put<T: Serialize>(conn: &mut SqliteConnection, key: &str, value: &T) -> Result<()> {
    let json = serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned());
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(json)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Writes every setting.
pub async fn save(conn: &mut SqliteConnection, s: &Settings) -> Result<()> {
    save_user_settings(conn, s).await?;
    set_last_reminded_on(conn, s.last_reminded_on).await?;
    set_last_notified_at(conn, s.last_notified_at).await?;
    set_tray_hint_shown(conn, s.tray_hint_shown).await?;
    Ok(())
}

/// Writes the settings the user edits (not the values the app writes).
pub async fn save_user_settings(conn: &mut SqliteConnection, s: &Settings) -> Result<()> {
    put(conn, REMINDER_FREQUENCY, &s.reminder_frequency).await?;
    put(conn, SOON_DAYS, &s.soon_days).await?;
    put(conn, FOLLOW_UP_DAYS, &s.follow_up_days).await?;
    put(conn, THEME, &s.theme).await?;
    put(conn, NOTIFY_ENABLED, &s.notify_enabled).await?;
    put(conn, NOTIFY_TIMES, &s.notify_times).await?;
    put(conn, CLOSE_TO_TRAY, &s.close_to_tray).await?;
    put(conn, AUTOSTART, &s.autostart).await?;
    Ok(())
}

/// The day the startup reminder was last acknowledged.
pub async fn set_last_reminded_on(
    conn: &mut SqliteConnection,
    day: Option<NaiveDate>,
) -> Result<()> {
    put(conn, LAST_REMINDED_ON, &day).await
}

/// When the timer last handled a notification time.
pub async fn set_last_notified_at(
    conn: &mut SqliteConnection,
    at: Option<DateTime<Utc>>,
) -> Result<()> {
    put(conn, LAST_NOTIFIED_AT, &at.map(format_timestamp)).await
}

/// Whether the tray hint was shown.
pub async fn set_tray_hint_shown(conn: &mut SqliteConnection, shown: bool) -> Result<()> {
    put(conn, TRAY_HINT_SHOWN, &shown).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::settings::{ReminderFrequency, Theme};
    use crate::domain::testing::{d, ts};
    use crate::repo::testing::conn;

    #[tokio::test]
    async fn empty_table_gives_defaults() {
        let (_pool, mut c) = conn().await;
        assert_eq!(load(&mut c).await.unwrap(), Settings::default());
    }

    #[tokio::test]
    async fn save_then_load_round_trips() {
        let (_pool, mut c) = conn().await;
        let s = Settings {
            reminder_frequency: ReminderFrequency::OncePerDay,
            soon_days: 5,
            follow_up_days: 2,
            last_reminded_on: Some(d("2026-11-19")),
            theme: Theme::System,
            notify_enabled: false,
            notify_times: vec!["08:30".into()],
            last_notified_at: Some(ts(2026, 11, 19, 0, 0)),
            close_to_tray: false,
            autostart: true,
            tray_hint_shown: true,
        };
        save(&mut c, &s).await.unwrap();
        assert_eq!(load(&mut c).await.unwrap(), s);

        // Saving again overwrites instead of failing on the primary key.
        let s2 = Settings {
            last_reminded_on: None,
            last_notified_at: None,
            ..s
        };
        save(&mut c, &s2).await.unwrap();
        assert_eq!(load(&mut c).await.unwrap(), s2);

        let mut rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM settings ORDER BY key")
                .fetch_all(&mut *c)
                .await
                .unwrap();
        rows.sort();
        assert_eq!(
            rows,
            vec![
                ("autostart".to_owned(), "true".to_owned()),
                ("close_to_tray".to_owned(), "false".to_owned()),
                ("follow_up_days".to_owned(), "2".to_owned()),
                ("last_notified_at".to_owned(), "null".to_owned()),
                ("last_reminded_on".to_owned(), "null".to_owned()),
                ("notify_enabled".to_owned(), "false".to_owned()),
                ("notify_times".to_owned(), r#"["08:30"]"#.to_owned()),
                (
                    "reminder_frequency".to_owned(),
                    r#""once_per_day""#.to_owned()
                ),
                ("soon_days".to_owned(), "5".to_owned()),
                ("theme".to_owned(), r#""system""#.to_owned()),
                ("tray_hint_shown".to_owned(), "true".to_owned()),
            ]
        );
    }

    #[tokio::test]
    async fn timestamps_are_stored_with_milliseconds_and_z() {
        let (_pool, mut c) = conn().await;
        set_last_notified_at(&mut c, Some(ts(2026, 11, 19, 0, 0)))
            .await
            .unwrap();
        let (value,): (String,) =
            sqlx::query_as("SELECT value FROM settings WHERE key = 'last_notified_at'")
                .fetch_one(&mut *c)
                .await
                .unwrap();
        assert_eq!(value, r#""2026-11-19T00:00:00.000Z""#);
        assert_eq!(
            load(&mut c).await.unwrap().last_notified_at,
            Some(ts(2026, 11, 19, 0, 0))
        );
    }

    #[tokio::test]
    async fn user_settings_leave_the_app_written_values() {
        let (_pool, mut c) = conn().await;
        set_last_reminded_on(&mut c, Some(d("2026-11-19")))
            .await
            .unwrap();
        set_last_notified_at(&mut c, Some(ts(2026, 11, 19, 3, 0)))
            .await
            .unwrap();
        set_tray_hint_shown(&mut c, true).await.unwrap();
        // A stale copy of the settings (as the settings screen holds it).
        let stale = Settings {
            soon_days: 7,
            ..Settings::default()
        };
        save_user_settings(&mut c, &stale).await.unwrap();
        let s = load(&mut c).await.unwrap();
        assert_eq!(s.soon_days, 7);
        assert_eq!(s.last_reminded_on, Some(d("2026-11-19")));
        assert_eq!(s.last_notified_at, Some(ts(2026, 11, 19, 3, 0)));
        assert!(s.tray_hint_shown);
    }

    #[tokio::test]
    async fn unreadable_values_fall_back_to_defaults() {
        let (_pool, mut c) = conn().await;
        for (k, v) in [
            ("soon_days", "\"many\""),
            ("theme", "\"purple\""),
            ("last_reminded_on", "\"2026-02-30\""),
            ("unknown_key", "1"),
            ("follow_up_days", "7"),
            ("notify_enabled", "\"yes\""),
            ("notify_times", r#"["09:00","09:00"]"#),
            ("last_notified_at", r#""yesterday""#),
            ("close_to_tray", "false"),
            ("autostart", "1"),
            ("tray_hint_shown", "null"),
        ] {
            sqlx::query("INSERT INTO settings (key, value) VALUES (?1, ?2)")
                .bind(k)
                .bind(v)
                .execute(&mut *c)
                .await
                .unwrap();
        }
        let s = load(&mut c).await.unwrap();
        assert_eq!(
            s,
            Settings {
                follow_up_days: 7,
                close_to_tray: false,
                ..Settings::default()
            }
        );

        // Lists of times that are not 1 to 5 valid, distinct times.
        for bad in [
            "[]",
            r#"["9時"]"#,
            r#"["1:00","2:00","3:00","4:00","5:00","6:00"]"#,
            "\"09:00\"",
        ] {
            sqlx::query("UPDATE settings SET value = ?1 WHERE key = 'notify_times'")
                .bind(bad)
                .execute(&mut *c)
                .await
                .unwrap();
            assert_eq!(
                load(&mut c).await.unwrap().notify_times,
                ["09:00", "12:55"],
                "{bad}"
            );
        }
    }
}
