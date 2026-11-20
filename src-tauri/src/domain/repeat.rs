//! 繰り返しのTODO (T-12, v0.3.0). Pure.
//!
//! The rule is stored as JSON in `todo_repeats.rule`:
//!
//! | 周期 | `rule` |
//! | --- | --- |
//! | 毎日 | `{"kind":"daily"}` |
//! | 毎週 | `{"kind":"weekly","weekdays":["mon","thu"]}` (1–7, `mon`–`sun`) |
//! | 毎月(日付) | `{"kind":"monthly_day","day":15}` (1–31) |
//! | 毎月(第N◯曜日) | `{"kind":"monthly_weekday","week":2,"weekday":"tue"}` (`week` 1–4, or −1 for the last) |
//! | 毎月(月末) | `{"kind":"monthly_last"}` |
//! | 毎年 | `{"kind":"yearly","month":3,"day":31}` (a day of the calendar; 2/29 allowed) |
//! | N日ごと | `{"kind":"every_days","days":10}` (2–365) |
//!
//! This phase has the types and the range check only; the next 日限 comes
//! with phase 21.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// A day of the week in a rule (`mon` … `sun`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum DayOfWeek {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

/// The 周期 of a repeating TODO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepeatRule {
    /// 毎日
    Daily,
    /// 毎週: on these days.
    Weekly { weekdays: Vec<DayOfWeek> },
    /// 毎月: on this day (the month's last day when it has no such day).
    MonthlyDay { day: u32 },
    /// 毎月: the `week`-th `weekday` (−1 = the last one).
    MonthlyWeekday { week: i32, weekday: DayOfWeek },
    /// 毎月: the last day.
    MonthlyLast,
    /// 毎年: on this month and day (2/29 is 2/28 in other years).
    Yearly { month: u32, day: u32 },
    /// N日ごと
    EveryDays { days: u32 },
}

/// Fewest days of `every_days`.
pub const EVERY_DAYS_MIN: u32 = 2;
/// Most days of `every_days`.
pub const EVERY_DAYS_MAX: u32 = 365;

/// What is out of range in a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatRuleError {
    /// 毎週 without any day.
    NoWeekday,
    /// 毎週 with the same day twice.
    WeekdayTwice,
    /// 毎月(日付): not 1–31.
    DayOutOfRange,
    /// 毎月(第N◯曜日): not 1–4 or −1.
    WeekOutOfRange,
    /// 毎年: a month and day no year has.
    NoSuchDate,
    /// N日ごと: not 2–365.
    DaysOutOfRange,
}

impl RepeatRule {
    /// Checks the ranges (the table above).
    pub fn check(&self) -> Result<(), RepeatRuleError> {
        match self {
            RepeatRule::Daily | RepeatRule::MonthlyLast => Ok(()),
            RepeatRule::Weekly { weekdays } => {
                if weekdays.is_empty() {
                    return Err(RepeatRuleError::NoWeekday);
                }
                for (i, day) in weekdays.iter().enumerate() {
                    if weekdays[..i].contains(day) {
                        return Err(RepeatRuleError::WeekdayTwice);
                    }
                }
                Ok(())
            }
            RepeatRule::MonthlyDay { day } => {
                if (1..=31).contains(day) {
                    Ok(())
                } else {
                    Err(RepeatRuleError::DayOutOfRange)
                }
            }
            RepeatRule::MonthlyWeekday { week, .. } => {
                if (1..=4).contains(week) || *week == -1 {
                    Ok(())
                } else {
                    Err(RepeatRuleError::WeekOutOfRange)
                }
            }
            RepeatRule::Yearly { month, day } => {
                // 2000 is a leap year, so 2/29 is a day of the calendar.
                if NaiveDate::from_ymd_opt(2000, *month, *day).is_some() {
                    Ok(())
                } else {
                    Err(RepeatRuleError::NoSuchDate)
                }
            }
            RepeatRule::EveryDays { days } => {
                if (EVERY_DAYS_MIN..=EVERY_DAYS_MAX).contains(days) {
                    Ok(())
                } else {
                    Err(RepeatRuleError::DaysOutOfRange)
                }
            }
        }
    }

    /// The stored JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Reads stored JSON. `None` when it is not a rule or is out of range.
    pub fn from_json(s: &str) -> Option<RepeatRule> {
        let rule: RepeatRule = serde_json::from_str(s).ok()?;
        rule.check().ok()?;
        Some(rule)
    }
}

/// The repetition of a TODO (`todo_repeats`). Only the current round has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoRepeat {
    /// The current round.
    pub todo_id: i64,
    pub rule: RepeatRule,
    /// The last day a round may fall on. `None` = no end.
    pub until_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(s: &str) -> Option<RepeatRule> {
        RepeatRule::from_json(s)
    }

    #[test]
    fn every_kind_of_the_design_round_trips() {
        for (json, rule) in [
            (r#"{"kind":"daily"}"#, RepeatRule::Daily),
            (
                r#"{"kind":"weekly","weekdays":["mon","thu"]}"#,
                RepeatRule::Weekly {
                    weekdays: vec![DayOfWeek::Mon, DayOfWeek::Thu],
                },
            ),
            (
                r#"{"kind":"monthly_day","day":15}"#,
                RepeatRule::MonthlyDay { day: 15 },
            ),
            (
                r#"{"kind":"monthly_weekday","week":2,"weekday":"tue"}"#,
                RepeatRule::MonthlyWeekday {
                    week: 2,
                    weekday: DayOfWeek::Tue,
                },
            ),
            (r#"{"kind":"monthly_last"}"#, RepeatRule::MonthlyLast),
            (
                r#"{"kind":"yearly","month":3,"day":31}"#,
                RepeatRule::Yearly { month: 3, day: 31 },
            ),
            (
                r#"{"kind":"every_days","days":10}"#,
                RepeatRule::EveryDays { days: 10 },
            ),
        ] {
            assert_eq!(read(json), Some(rule.clone()), "{json}");
            assert_eq!(rule.to_json(), json);
        }
    }

    #[test]
    fn every_day_of_the_week_is_read() {
        let rule =
            read(r#"{"kind":"weekly","weekdays":["sun","sat","fri","thu","wed","tue","mon"]}"#)
                .unwrap();
        let RepeatRule::Weekly { weekdays } = rule else {
            panic!("not weekly");
        };
        assert_eq!(weekdays.len(), 7);
        assert_eq!(weekdays[0], DayOfWeek::Sun);
    }

    #[test]
    fn json_of_another_shape_is_not_read() {
        for bad in [
            "",
            "{}",
            "null",
            r#""daily""#,
            r#"{"kind":"hourly"}"#,
            r#"{"kind":"Daily"}"#,
            r#"{"kind":"weekly"}"#,
            r#"{"kind":"weekly","weekdays":["monday"]}"#,
            r#"{"kind":"weekly","weekdays":"mon"}"#,
            r#"{"kind":"monthly_day"}"#,
            r#"{"kind":"monthly_day","day":-1}"#,
            r#"{"kind":"monthly_day","day":"15"}"#,
            r#"{"kind":"monthly_weekday","week":2}"#,
            r#"{"kind":"yearly","month":3}"#,
            r#"{"kind":"every_days","days":2.5}"#,
        ] {
            assert_eq!(read(bad), None, "{bad}");
        }
    }

    #[test]
    fn weekdays_are_one_to_seven_without_repeats() {
        assert_eq!(
            RepeatRule::Weekly { weekdays: vec![] }.check(),
            Err(RepeatRuleError::NoWeekday)
        );
        assert_eq!(
            RepeatRule::Weekly {
                weekdays: vec![DayOfWeek::Mon, DayOfWeek::Thu, DayOfWeek::Mon]
            }
            .check(),
            Err(RepeatRuleError::WeekdayTwice)
        );
        assert_eq!(read(r#"{"kind":"weekly","weekdays":[]}"#), None);
        assert_eq!(read(r#"{"kind":"weekly","weekdays":["mon","mon"]}"#), None);
    }

    #[test]
    fn day_of_the_month_is_one_to_thirty_one() {
        for (day, ok) in [(0, false), (1, true), (31, true), (32, false)] {
            let r = RepeatRule::MonthlyDay { day }.check();
            assert_eq!(r.is_ok(), ok, "{day}");
            if !ok {
                assert_eq!(r, Err(RepeatRuleError::DayOutOfRange));
            }
        }
    }

    #[test]
    fn week_of_the_month_is_one_to_four_or_the_last() {
        for (week, ok) in [
            (-2, false),
            (-1, true),
            (0, false),
            (1, true),
            (4, true),
            (5, false),
        ] {
            let r = RepeatRule::MonthlyWeekday {
                week,
                weekday: DayOfWeek::Fri,
            }
            .check();
            assert_eq!(r.is_ok(), ok, "{week}");
            if !ok {
                assert_eq!(r, Err(RepeatRuleError::WeekOutOfRange));
            }
        }
    }

    #[test]
    fn yearly_takes_days_of_the_calendar_and_february_29() {
        for (month, day, ok) in [
            (1, 1, true),
            (1, 31, true),
            (2, 29, true),
            (2, 30, false),
            (4, 30, true),
            (4, 31, false),
            (12, 31, true),
            (0, 1, false),
            (13, 1, false),
            (1, 0, false),
            (1, 32, false),
        ] {
            let r = RepeatRule::Yearly { month, day }.check();
            assert_eq!(r.is_ok(), ok, "{month}/{day}");
            if !ok {
                assert_eq!(r, Err(RepeatRuleError::NoSuchDate));
            }
        }
    }

    #[test]
    fn every_days_is_two_to_three_hundred_sixty_five() {
        for (days, ok) in [(0, false), (1, false), (2, true), (365, true), (366, false)] {
            let r = RepeatRule::EveryDays { days }.check();
            assert_eq!(r.is_ok(), ok, "{days}");
            if !ok {
                assert_eq!(r, Err(RepeatRuleError::DaysOutOfRange));
            }
        }
        assert_eq!(read(r#"{"kind":"every_days","days":1}"#), None);
        assert_eq!(
            read(r#"{"kind":"every_days","days":365}"#),
            Some(RepeatRule::EveryDays { days: 365 })
        );
    }
}
