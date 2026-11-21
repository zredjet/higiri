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
//! | `day_count` | `"calendar"` or `"business"` |
//! | `custom_holidays` | `[{"name", "start", "end", "yearly"}, …]` |
//! | `report_template` | `{"title", "sections": [{"kind", "heading", "enabled", "options"}, …]}` |
//! | `weekly_report_template` | the same shape, with the weekly report's kinds and items |
//! | `zoom` | `70`, `80`, `90`, `100`, `110`, `125` or `150` |
//!
//! Missing keys, unknown keys and unreadable values fall back to the defaults
//! so that a bad row never prevents the app from starting (a `zoom` that is
//! not one of the levels, too).
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
use crate::domain::settings::{
    CustomHoliday, ReportTemplate, Settings, WeeklyReportTemplate, is_zoom_level,
};

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
pub const DAY_COUNT: &str = "day_count";
pub const CUSTOM_HOLIDAYS: &str = "custom_holidays";
pub const REPORT_TEMPLATE: &str = "report_template";
pub const WEEKLY_REPORT_TEMPLATE: &str = "weekly_report_template";
pub const ZOOM: &str = "zoom";

/// Keys a restore (and the JSON import) leaves as they are on this PC:
/// `autostart` and `zoom` belong to the PC (the screen differs from PC to
/// PC), the others are written by the app.
pub const KEPT_ON_REPLACE: [&str; 5] = [
    AUTOSTART,
    LAST_NOTIFIED_AT,
    TRAY_HINT_SHOWN,
    LAST_REMINDED_ON,
    ZOOM,
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

/// Stored custom holidays, or the fallback unless every entry is valid.
fn read_custom_holidays(value: &str, fallback: Vec<CustomHoliday>) -> Vec<CustomHoliday> {
    serde_json::from_str::<Vec<CustomHoliday>>(value)
        .ok()
        .and_then(|list| list.iter().map(|h| h.checked().ok()).collect())
        .unwrap_or(fallback)
}

/// A stored report template, normalized (missing sections added, repeats
/// dropped, 所感・連絡事項 last), or the fallback unless it can be read and its
/// title and headings pass the checks.
fn read_report_template(value: &str, fallback: ReportTemplate) -> ReportTemplate {
    serde_json::from_str::<ReportTemplate>(value)
        .ok()
        .and_then(|t| t.checked().ok())
        .unwrap_or(fallback)
}

/// A stored weekly report template, read by the same rules as the daily one.
fn read_weekly_report_template(
    value: &str,
    fallback: WeeklyReportTemplate,
) -> WeeklyReportTemplate {
    serde_json::from_str::<WeeklyReportTemplate>(value)
        .ok()
        .and_then(|t| t.checked().ok())
        .unwrap_or(fallback)
}

/// A stored 表示倍率, or the fallback unless it is one of the levels.
fn read_zoom(value: &str, fallback: u32) -> u32 {
    serde_json::from_str::<u32>(value)
        .ok()
        .filter(|&z| is_zoom_level(z))
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
            DAY_COUNT => s.day_count = read(&value, s.day_count),
            CUSTOM_HOLIDAYS => s.custom_holidays = read_custom_holidays(&value, s.custom_holidays),
            REPORT_TEMPLATE => s.report_template = read_report_template(&value, s.report_template),
            WEEKLY_REPORT_TEMPLATE => {
                s.weekly_report_template =
                    read_weekly_report_template(&value, s.weekly_report_template)
            }
            ZOOM => s.zoom = read_zoom(&value, s.zoom),
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
    put(conn, DAY_COUNT, &s.day_count).await?;
    put(conn, CUSTOM_HOLIDAYS, &s.custom_holidays).await?;
    put(conn, REPORT_TEMPLATE, &s.report_template).await?;
    put(conn, WEEKLY_REPORT_TEMPLATE, &s.weekly_report_template).await?;
    put(conn, ZOOM, &s.zoom).await?;
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
    use crate::domain::report_template::{
        SectionBody, SectionKind, Template, WeeklySectionBody, WeeklySectionKind,
    };
    use crate::domain::settings::{DayCount, ReminderFrequency, Theme};
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
            day_count: DayCount::Business,
            custom_holidays: vec![CustomHoliday {
                name: "年末年始".into(),
                start: d("2026-12-29"),
                end: d("2027-01-03"),
                yearly: true,
            }],
            report_template: ReportTemplate {
                title: "{日付} 業務報告".into(),
                ..ReportTemplate::default()
            },
            weekly_report_template: WeeklyReportTemplate {
                title: "{開始日}の週".into(),
                ..WeeklyReportTemplate::default()
            },
            zoom: 125,
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
        let (_, template) = rows.remove(
            rows.iter()
                .position(|(k, _)| k == "report_template")
                .unwrap(),
        );
        assert!(
            template.starts_with(r#"{"title":"{日付} 業務報告","sections":[{"heading":"本日の作業","enabled":true,"kind":"work","options":{"project_headings":true,"#),
            "{template}"
        );
        let (_, weekly) = rows.remove(
            rows.iter()
                .position(|(k, _)| k == "weekly_report_template")
                .unwrap(),
        );
        assert!(
            weekly.starts_with(r#"{"title":"{開始日}の週","sections":[{"heading":"今週の作業","enabled":true,"kind":"work","options":{"project_headings":true,"#),
            "{weekly}"
        );
        assert_eq!(
            rows,
            vec![
                ("autostart".to_owned(), "true".to_owned()),
                ("close_to_tray".to_owned(), "false".to_owned()),
                (
                    "custom_holidays".to_owned(),
                    r#"[{"name":"年末年始","start":"2026-12-29","end":"2027-01-03","yearly":true}]"#
                        .to_owned()
                ),
                ("day_count".to_owned(), r#""business""#.to_owned()),
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
                ("zoom".to_owned(), "125".to_owned()),
            ]
        );
    }

    #[tokio::test]
    async fn zoom_is_one_of_the_levels() {
        let (_pool, mut c) = conn().await;
        // No row: 100.
        assert_eq!(load(&mut c).await.unwrap().zoom, 100);
        for (stored, zoom) in [
            ("70", 70),
            ("80", 80),
            ("90", 90),
            ("100", 100),
            ("110", 110),
            ("125", 125),
            ("150", 150),
            // Not one of the levels.
            ("0", 100),
            ("75", 100),
            ("120", 100),
            ("200", 100),
            ("-80", 100),
            // Unreadable.
            ("125.0", 100),
            ("1.25", 100),
            (r#""125""#, 100),
            ("null", 100),
            ("[125]", 100),
        ] {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES ('zoom', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(stored)
            .execute(&mut *c)
            .await
            .unwrap();
            assert_eq!(load(&mut c).await.unwrap().zoom, zoom, "{stored}");
        }
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
    async fn report_templates_are_normalized_when_read() {
        let (_pool, mut c) = conn().await;
        // 所感 first, 待ち twice, the rest missing.
        let stored = r#"{"title":" ## 日報 {日付} ","sections":[
            {"kind":"note","heading":"連絡","enabled":false,"options":{}},
            {"kind":"waiting","heading":"待ち状況","enabled":false,"options":{"waiting_on":true,"waiting_days":false,"follow_up_date":true}},
            {"kind":"waiting","heading":"待ち","enabled":true,"options":{"waiting_on":true,"waiting_days":true,"follow_up_date":true}}
        ]}"#;
        sqlx::query("INSERT INTO settings (key, value) VALUES ('report_template', ?1)")
            .bind(stored)
            .execute(&mut *c)
            .await
            .unwrap();
        let t = load(&mut c).await.unwrap().report_template;
        assert_eq!(t.title, "日報 {日付}");
        let kinds: Vec<SectionKind> = t.sections.iter().map(|s| s.kind()).collect();
        assert_eq!(
            kinds,
            [
                SectionKind::Waiting,
                SectionKind::Work,
                SectionKind::Closed,
                SectionKind::Tomorrow,
                SectionKind::Attention,
                SectionKind::Note,
            ]
        );
        let waiting = &t.sections[0];
        assert_eq!(waiting.heading, "待ち状況");
        assert!(!waiting.enabled);
        assert!(matches!(&waiting.body, SectionBody::Waiting(o) if !o.waiting_days));
        assert_eq!(t.note_heading(), "連絡");
        assert!(t.sections[5].enabled);
    }

    #[tokio::test]
    async fn weekly_report_templates_are_read_as_the_daily_ones() {
        let (_pool, mut c) = conn().await;
        // 所感 first, 来週の予定 twice, the rest missing.
        let stored = r#"{"title":" # 週報 {開始日} ","sections":[
            {"kind":"note","heading":"連絡","enabled":false,"options":{}},
            {"kind":"next_week","heading":"翌週","enabled":false,"options":{"planned_period":false,"due_date":true}},
            {"kind":"next_week","heading":"来週の予定","enabled":true,"options":{"planned_period":true,"due_date":true}}
        ]}"#;
        sqlx::query("INSERT INTO settings (key, value) VALUES ('weekly_report_template', ?1)")
            .bind(stored)
            .execute(&mut *c)
            .await
            .unwrap();
        let t = load(&mut c).await.unwrap().weekly_report_template;
        assert_eq!(t.title, "週報 {開始日}");
        let kinds: Vec<WeeklySectionKind> = t.sections.iter().map(|s| s.kind()).collect();
        assert_eq!(
            kinds,
            [
                WeeklySectionKind::NextWeek,
                WeeklySectionKind::Work,
                WeeklySectionKind::Closed,
                WeeklySectionKind::Waiting,
                WeeklySectionKind::Attention,
                WeeklySectionKind::Note,
            ]
        );
        assert_eq!(t.sections[0].heading, "翌週");
        assert!(!t.sections[0].enabled);
        assert!(matches!(&t.sections[0].body, WeeklySectionBody::NextWeek(o) if !o.planned_period));
        assert_eq!(t.note_heading(), "連絡");
        assert!(t.sections[5].enabled);

        // Unreadable or not passing the checks: the default as a whole.
        for bad in [
            r#""週報""#,
            r#"{"title":"","sections":[]}"#,
            // A daily kind.
            r#"{"title":"T","sections":[{"kind":"tomorrow","heading":"明日","enabled":true,"options":{"planned_period":true}}]}"#,
            r#"{"title":"T","sections":[{"kind":"work","heading":"待ち","enabled":true,"options":{"project_headings":true,"status":true,"due_date":true,"work_days":true,"work_time":true,"total_time":true,"log_body":false}}]}"#,
            // Missing an item.
            r#"{"title":"T","sections":[{"kind":"closed","heading":"完了","enabled":true,"options":{"close_reason":true}}]}"#,
        ] {
            sqlx::query("UPDATE settings SET value = ?1 WHERE key = 'weekly_report_template'")
                .bind(bad)
                .execute(&mut *c)
                .await
                .unwrap();
            assert_eq!(
                load(&mut c).await.unwrap().weekly_report_template,
                WeeklyReportTemplate::default(),
                "{bad}"
            );
        }
        // The daily template is not touched by the weekly one.
        assert_eq!(
            load(&mut c).await.unwrap().report_template,
            ReportTemplate::default()
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
            ("notify_enabled", "\"yes\""),
            ("notify_times", r#"["09:00","09:00"]"#),
            ("last_notified_at", r#""yesterday""#),
            ("close_to_tray", "false"),
            ("autostart", "1"),
            ("tray_hint_shown", "null"),
            ("day_count", "\"weekdays\""),
            (
                "custom_holidays",
                r#"[{"name":"","start":"2026-12-29","end":"2026-12-30","yearly":false}]"#,
            ),
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

        // Report templates that cannot be read or do not pass the checks.
        for bad in [
            r#""日報""#,
            r#"{"title":"","sections":[]}"#,
            r#"{"title":"T","sections":[{"kind":"work","heading":"待ち","enabled":true,"options":{"project_headings":true,"status":true,"due_date":true,"work_time":true}}]}"#,
            r#"{"title":"T","sections":[{"kind":"weekly","heading":"週","enabled":true,"options":{}}]}"#,
            r#"{"title":"T","sections":[{"kind":"closed","heading":"完了","enabled":true,"options":{"close_reason":"yes"}}]}"#,
        ] {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES ('report_template', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(bad)
            .execute(&mut *c)
            .await
            .unwrap();
            assert_eq!(
                load(&mut c).await.unwrap().report_template,
                ReportTemplate::default(),
                "{bad}"
            );
        }

        // Custom holidays: one bad entry drops the stored value.
        for bad in [
            r#"[{"name":"逆","start":"2026-12-30","end":"2026-12-29","yearly":false}]"#,
            r#"[{"name":"長い","start":"2026-08-01","end":"2026-09-01","yearly":false}]"#,
            r#"[{"name":"形","start":"2026/12/29","end":"2026-12-30","yearly":false}]"#,
            r#"{"name":"一つ"}"#,
        ] {
            sqlx::query("UPDATE settings SET value = ?1 WHERE key = 'custom_holidays'")
                .bind(bad)
                .execute(&mut *c)
                .await
                .unwrap();
            assert!(
                load(&mut c).await.unwrap().custom_holidays.is_empty(),
                "{bad}"
            );
        }
    }
}
