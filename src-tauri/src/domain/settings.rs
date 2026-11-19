//! Settings and their defaults. Stored one key per row in `settings`, with the
//! value as JSON (see `repo::settings`).

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// When the startup reminder dialog is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderFrequency {
    /// 起動のたび (default)
    #[default]
    EveryLaunch,
    /// 1日1回
    OncePerDay,
}

/// Colour scheme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub reminder_frequency: ReminderFrequency,
    /// Days counted as 「間近」.
    pub soon_days: u32,
    /// Default follow-up date when moving to 待ち: today + N days.
    pub follow_up_days: u32,
    /// Last day the reminder was acknowledged.
    pub last_reminded_on: Option<NaiveDate>,
    pub theme: Theme,
}

pub const DEFAULT_SOON_DAYS: u32 = 3;
pub const DEFAULT_FOLLOW_UP_DAYS: u32 = 3;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            reminder_frequency: ReminderFrequency::EveryLaunch,
            soon_days: DEFAULT_SOON_DAYS,
            follow_up_days: DEFAULT_FOLLOW_UP_DAYS,
            last_reminded_on: None,
            theme: Theme::Light,
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
    }
}
