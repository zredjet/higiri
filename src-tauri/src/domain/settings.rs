//! Settings and their defaults. Stored one key per row in `settings`, with the
//! value as JSON (see `repo::settings`).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

pub use super::calendar::{CustomHoliday, DayCount};
pub use super::report_template::ReportTemplate;

/// When the startup reminder dialog is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ReminderFrequency {
    /// 起動のたび (default)
    #[default]
    EveryLaunch,
    /// 1日1回
    OncePerDay,
}

/// Colour scheme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// ライト (default; selected in the settings mock)
    #[default]
    Light,
    /// ダーク
    Dark,
    /// OSの設定に合わせる
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Settings {
    pub reminder_frequency: ReminderFrequency,
    /// Days counted as 「間近」.
    pub soon_days: u32,
    /// Default follow-up date when moving to 待ち: today + N days.
    pub follow_up_days: u32,
    /// Last day the reminder was acknowledged.
    pub last_reminded_on: Option<NaiveDate>,
    pub theme: Theme,
    /// OS notifications at `notify_times` (v1.1).
    pub notify_enabled: bool,
    /// Notification times, `HH:MM`, 1 to 5, distinct, in time order.
    pub notify_times: Vec<String>,
    /// When the timer last handled a notification time (written by the
    /// timer, not by update_settings).
    pub last_notified_at: Option<DateTime<Utc>>,
    /// × hides the window to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Start at login (registered with the OS by release builds only).
    pub autostart: bool,
    /// The tray hint was shown (written by the app, not by update_settings).
    pub tray_hint_shown: bool,
    /// 暦日 or 営業日 (v1.1).
    pub day_count: DayCount,
    /// The user's own holidays (v1.1).
    pub custom_holidays: Vec<CustomHoliday>,
    /// Sections, headings and items of the daily report (v1.1).
    pub report_template: ReportTemplate,
}

pub const DEFAULT_SOON_DAYS: u32 = 3;
pub const DEFAULT_FOLLOW_UP_DAYS: u32 = 3;
pub use super::notify::DEFAULT_TIMES as DEFAULT_NOTIFY_TIMES;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            reminder_frequency: ReminderFrequency::EveryLaunch,
            soon_days: DEFAULT_SOON_DAYS,
            follow_up_days: DEFAULT_FOLLOW_UP_DAYS,
            last_reminded_on: None,
            theme: Theme::Light,
            notify_enabled: true,
            notify_times: DEFAULT_NOTIFY_TIMES.iter().map(|t| t.to_string()).collect(),
            last_notified_at: None,
            close_to_tray: true,
            autostart: false,
            tray_hint_shown: false,
            day_count: DayCount::Calendar,
            custom_holidays: Vec::new(),
            report_template: ReportTemplate::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_design() {
        let s = Settings::default();
        assert_eq!(s.reminder_frequency, ReminderFrequency::EveryLaunch);
        assert_eq!(s.soon_days, 3);
        assert_eq!(s.follow_up_days, 3);
        assert_eq!(s.last_reminded_on, None);
        assert_eq!(s.theme, Theme::Light);
        assert!(s.notify_enabled);
        assert_eq!(s.notify_times, ["09:00", "12:55"]);
        assert_eq!(s.last_notified_at, None);
        assert!(s.close_to_tray);
        assert!(!s.autostart);
        assert!(!s.tray_hint_shown);
        assert_eq!(s.day_count, DayCount::Calendar);
        assert!(s.custom_holidays.is_empty());
        assert_eq!(s.report_template, ReportTemplate::default());
    }
}
