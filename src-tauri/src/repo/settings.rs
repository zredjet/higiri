//! `settings` table: one row per key, value stored as JSON.
//!
//! | key | value (JSON) |
//! | --- | --- |
//! | `reminder_frequency` | `"every_launch"` or `"once_per_day"` |
//! | `soon_days` | number |
//! | `follow_up_days` | number |
//! | `last_reminded_on` | `"YYYY-MM-DD"` or `null` |
//! | `theme` | `"light"`, `"dark"` or `"system"` |
//!
//! Missing keys, unknown keys and unreadable values fall back to the defaults
//! so that a bad row never prevents the app from starting.

use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::SqliteConnection;

use super::Result;
use crate::domain::settings::Settings;

const REMINDER_FREQUENCY: &str = "reminder_frequency";
const SOON_DAYS: &str = "soon_days";
const FOLLOW_UP_DAYS: &str = "follow_up_days";
const LAST_REMINDED_ON: &str = "last_reminded_on";
const THEME: &str = "theme";

fn read<T: DeserializeOwned>(value: &str, fallback: T) -> T {
    serde_json::from_str(value).unwrap_or(fallback)
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
    put(conn, REMINDER_FREQUENCY, &s.reminder_frequency).await?;
    put(conn, SOON_DAYS, &s.soon_days).await?;
    put(conn, FOLLOW_UP_DAYS, &s.follow_up_days).await?;
    put(conn, LAST_REMINDED_ON, &s.last_reminded_on).await?;
    put(conn, THEME, &s.theme).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::settings::{ReminderFrequency, Theme};
    use crate::domain::testing::d;
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
        };
        save(&mut c, &s).await.unwrap();
        assert_eq!(load(&mut c).await.unwrap(), s);

        // Saving again overwrites instead of failing on the primary key.
        let s2 = Settings {
            last_reminded_on: None,
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
                ("follow_up_days".to_owned(), "2".to_owned()),
                ("last_reminded_on".to_owned(), "null".to_owned()),
                (
                    "reminder_frequency".to_owned(),
                    r#""once_per_day""#.to_owned()
                ),
                ("soon_days".to_owned(), "5".to_owned()),
                ("theme".to_owned(), r#""system""#.to_owned()),
            ]
        );
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
                ..Settings::default()
            }
        );
    }
}
