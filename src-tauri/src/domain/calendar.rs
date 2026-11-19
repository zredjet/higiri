//! 営業日の暦 (WorkCalendar): weekends, national holidays and the user's own
//! holidays, and the business-day arithmetic `classify` uses. Pure.
//!
//! Business days are counted without walking the days one by one: whole
//! weeks are counted arithmetically, and the holidays that fall on weekdays
//! are kept sorted and counted with a binary search.

use chrono::{Datelike, Months, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

use super::date::add_days;
use super::holidays::{self, LAST_YEAR};

/// How day counts are made (`day_count`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DayCount {
    /// 暦日 (default): every day counts.
    #[default]
    Calendar,
    /// 営業日: weekends, national holidays and custom holidays do not count.
    Business,
}

/// A holiday of the user's own (年末年始 and the like), one entry of
/// `custom_holidays`. `start` and `end` are the dates in the first year; a
/// yearly holiday repeats every year from then on and may cross the new year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CustomHoliday {
    pub name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub yearly: bool,
}

/// Longest name of a custom holiday (characters).
pub const CUSTOM_NAME_MAX: usize = 30;
/// Longest custom holiday (days, both ends included).
pub const CUSTOM_DAYS_MAX: i64 = 31;

/// What is wrong with a custom holiday.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomHolidayError {
    EmptyName,
    NameTooLong,
    Reversed,
    TooLong,
}

impl CustomHoliday {
    /// The holiday with its name trimmed, or what is wrong with it.
    pub fn checked(&self) -> Result<CustomHoliday, CustomHolidayError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(CustomHolidayError::EmptyName);
        }
        if name.chars().count() > CUSTOM_NAME_MAX {
            return Err(CustomHolidayError::NameTooLong);
        }
        if self.start > self.end {
            return Err(CustomHolidayError::Reversed);
        }
        if (self.end - self.start).num_days() + 1 > CUSTOM_DAYS_MAX {
            return Err(CustomHolidayError::TooLong);
        }
        Ok(CustomHoliday {
            name: name.to_owned(),
            ..self.clone()
        })
    }

    /// The periods it covers: once, or every year from `start`'s year to
    /// 2099 (the last year with national holidays). A yearly 2/29 falls on
    /// 2/28 in other years.
    fn periods(&self) -> Vec<(NaiveDate, NaiveDate)> {
        let length = (self.end - self.start).num_days().max(0) as u32;
        if !self.yearly {
            return vec![(self.start, self.end)];
        }
        let last = LAST_YEAR.max(self.start.year());
        (0..=(last - self.start.year()) as u32)
            .filter_map(|k| self.start.checked_add_months(Months::new(12 * k)))
            .map(|s| (s, add_days(s, length)))
            .collect()
    }
}

/// A holiday with its name, as `list_holidays` returns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Holiday {
    pub date: NaiveDate,
    pub name: String,
    /// A custom holiday (false: a national holiday).
    pub custom: bool,
}

fn is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Monday–Friday days up to and including `date`, counted from a fixed Monday.
/// Only differences of this are meaningful.
fn weekdays_through(date: NaiveDate) -> i64 {
    // 0001-01-01 (day 1 from CE) was a Monday, so `n` is 0 on Mondays mod 7.
    let n = i64::from(date.num_days_from_ce()) - 1;
    let days = n + 1;
    days.div_euclid(7) * 5 + days.rem_euclid(7).min(5)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkCalendar {
    pub mode: DayCount,
    pub custom: Vec<CustomHoliday>,
    /// National and custom holidays that fall on Monday–Friday: sorted, distinct.
    weekday_holidays: Vec<NaiveDate>,
}

impl Default for WorkCalendar {
    fn default() -> Self {
        WorkCalendar::new(DayCount::Calendar, Vec::new())
    }
}

impl WorkCalendar {
    pub fn new(mode: DayCount, custom: Vec<CustomHoliday>) -> WorkCalendar {
        let mut days: Vec<NaiveDate> = holidays::all()
            .iter()
            .map(|h| h.date)
            .filter(|d| !is_weekend(*d))
            .collect();
        for h in &custom {
            for (start, end) in h.periods() {
                let mut day = start;
                while day <= end {
                    if !is_weekend(day) {
                        days.push(day);
                    }
                    match day.succ_opt() {
                        Some(next) => day = next,
                        None => break,
                    }
                }
            }
        }
        days.sort_unstable();
        days.dedup();
        WorkCalendar {
            mode,
            custom,
            weekday_holidays: days,
        }
    }

    /// Counting in business days (`day_count` is 営業日).
    pub fn business(&self) -> bool {
        self.mode == DayCount::Business
    }

    fn is_weekday_holiday(&self, date: NaiveDate) -> bool {
        self.weekday_holidays.binary_search(&date).is_ok()
    }

    /// Not a weekend, national holiday or custom holiday.
    pub fn is_business_day(&self, date: NaiveDate) -> bool {
        !is_weekend(date) && !self.is_weekday_holiday(date)
    }

    /// Business days in `(a, b]` (after `a`, up to and including `b`); 0 when
    /// `b <= a`. Does not depend on the length of the period.
    pub fn business_days_in(&self, a: NaiveDate, b: NaiveDate) -> i64 {
        if b <= a {
            return 0;
        }
        let weekdays = weekdays_through(b) - weekdays_through(a);
        let upto = |d: NaiveDate| self.weekday_holidays.partition_point(|h| *h <= d) as i64;
        weekdays - (upto(b) - upto(a))
    }

    /// The `n`-th business day after `date` (`date` itself when `n` is 0).
    pub fn add_business_days(&self, date: NaiveDate, n: u32) -> NaiveDate {
        let mut day = date;
        let mut left = n;
        while left > 0 {
            match day.succ_opt() {
                Some(next) => day = next,
                None => return day,
            }
            if self.is_business_day(day) {
                left -= 1;
            }
        }
        day
    }

    /// The last business day on or before `date`.
    pub fn previous_business_day(&self, date: NaiveDate) -> NaiveDate {
        let mut day = date;
        while !self.is_business_day(day) {
            match day.pred_opt() {
                Some(prev) => day = prev,
                None => return day,
            }
        }
        day
    }

    /// The due date the judgement uses: in business days, a due date on a
    /// holiday moves to the business day before it. Unchanged in 暦日.
    pub fn effective_due(&self, due: NaiveDate) -> NaiveDate {
        if self.business() {
            self.previous_business_day(due)
        } else {
            due
        }
    }

    /// National and custom holidays from `start` to `end` (both included), in
    /// date order; on a date with both, the national holiday comes first.
    pub fn holidays_in(&self, start: NaiveDate, end: NaiveDate) -> Vec<Holiday> {
        let mut out: Vec<Holiday> = holidays::between(start, end)
            .iter()
            .map(|h| Holiday {
                date: h.date,
                name: h.name.to_owned(),
                custom: false,
            })
            .collect();
        for h in &self.custom {
            for (from, to) in h.periods() {
                let (from, to) = (from.max(start), to.min(end));
                let mut day = from;
                while day <= to {
                    out.push(Holiday {
                        date: day,
                        name: h.name.clone(),
                        custom: true,
                    });
                    match day.succ_opt() {
                        Some(next) => day = next,
                        None => break,
                    }
                }
            }
        }
        out.sort_by(|a, b| (a.date, a.custom, &a.name).cmp(&(b.date, b.custom, &b.name)));
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::date::days_between;
    use crate::domain::testing::d;

    fn custom(name: &str, start: &str, end: &str, yearly: bool) -> CustomHoliday {
        CustomHoliday {
            name: name.into(),
            start: d(start),
            end: d(end),
            yearly,
        }
    }

    fn business(custom: Vec<CustomHoliday>) -> WorkCalendar {
        WorkCalendar::new(DayCount::Business, custom)
    }

    fn year_end() -> CustomHoliday {
        custom("年末年始", "2026-12-29", "2027-01-03", true)
    }

    /// Counts by walking the days, to check the fast count against.
    fn walk(cal: &WorkCalendar, a: NaiveDate, b: NaiveDate) -> i64 {
        let mut n = 0;
        let mut day = a;
        while day < b {
            day = day.succ_opt().unwrap();
            if cal.is_business_day(day) {
                n += 1;
            }
        }
        n
    }

    #[test]
    fn weekends_and_holidays_are_not_business_days() {
        let cal = business(vec![]);
        assert!(cal.is_business_day(d("2026-11-20"))); // Friday
        assert!(!cal.is_business_day(d("2026-11-21"))); // Saturday
        assert!(!cal.is_business_day(d("2026-11-22"))); // Sunday
        assert!(!cal.is_business_day(d("2026-11-23"))); // 勤労感謝の日 (Monday)
        assert!(cal.is_business_day(d("2026-11-24")));
        // Outside 2020–2099 only weekends are off.
        assert!(cal.is_business_day(d("2100-01-01"))); // Friday
        assert!(cal.is_business_day(d("2019-01-01"))); // Tuesday
    }

    #[test]
    fn friday_to_monday_is_one_business_day() {
        let cal = business(vec![]);
        assert_eq!(cal.business_days_in(d("2026-11-13"), d("2026-11-16")), 1);
        assert_eq!(cal.add_business_days(d("2026-11-13"), 1), d("2026-11-16"));
        // Over the holiday Monday 11/23: Friday → Tuesday.
        assert_eq!(cal.business_days_in(d("2026-11-20"), d("2026-11-24")), 1);
        assert_eq!(cal.add_business_days(d("2026-11-20"), 1), d("2026-11-24"));
        assert_eq!(cal.add_business_days(d("2026-11-20"), 0), d("2026-11-20"));
    }

    #[test]
    fn golden_week_and_silver_week_of_2026() {
        let cal = business(vec![]);
        // 4/29 (Wed) 昭和の日; 5/3 Sun, 5/4–5/5 holidays, 5/6 振替休日.
        assert_eq!(cal.add_business_days(d("2026-05-01"), 1), d("2026-05-07"));
        assert_eq!(cal.business_days_in(d("2026-05-01"), d("2026-05-07")), 1);
        assert_eq!(cal.previous_business_day(d("2026-05-06")), d("2026-05-01"));
        // 9/21 敬老の日, 9/22 国民の休日, 9/23 秋分の日 (Mon–Wed).
        assert_eq!(cal.add_business_days(d("2026-09-18"), 1), d("2026-09-24"));
        assert_eq!(cal.previous_business_day(d("2026-09-23")), d("2026-09-18"));
        assert_eq!(cal.business_days_in(d("2026-09-18"), d("2026-09-25")), 2);
    }

    #[test]
    fn yearly_custom_holiday_crosses_the_new_year() {
        let cal = business(vec![year_end()]);
        for day in [
            "2026-12-29",
            "2026-12-30",
            "2026-12-31",
            "2027-01-01",
            "2027-01-04",
        ] {
            let expected = day == "2027-01-04";
            assert_eq!(cal.is_business_day(d(day)), expected, "{day}");
        }
        // Repeats the next year: 2027-12-29 (Wed) to 2028-01-03 (Mon).
        assert!(!cal.is_business_day(d("2027-12-29")));
        assert!(!cal.is_business_day(d("2028-01-03")));
        assert!(cal.is_business_day(d("2028-01-04")));
        // But not before its first year.
        assert!(cal.is_business_day(d("2025-12-29")));
        // Friday 12/25 to Monday 1/4: only 12/28 (Mon) and 1/4.
        assert_eq!(cal.business_days_in(d("2026-12-25"), d("2027-01-04")), 2);
        assert_eq!(cal.add_business_days(d("2026-12-28"), 1), d("2027-01-04"));
    }

    #[test]
    fn one_off_custom_holiday_applies_once() {
        let cal = business(vec![custom(
            "創立記念日",
            "2026-11-26",
            "2026-11-26",
            false,
        )]);
        assert!(!cal.is_business_day(d("2026-11-26")));
        assert!(cal.is_business_day(d("2027-11-26")));
    }

    #[test]
    fn effective_due_moves_holidays_back_in_business_days_only() {
        let cal = business(vec![]);
        assert_eq!(cal.effective_due(d("2026-11-22")), d("2026-11-20")); // Sun → Fri
        assert_eq!(cal.effective_due(d("2026-11-23")), d("2026-11-20")); // holiday Mon → Fri
        assert_eq!(cal.effective_due(d("2026-11-24")), d("2026-11-24"));
        let calendar = WorkCalendar::new(DayCount::Calendar, vec![year_end()]);
        assert_eq!(calendar.effective_due(d("2026-11-22")), d("2026-11-22"));
        // Even in 暦日, the calendar knows the holidays (the Gantt shades them).
        assert!(!calendar.is_business_day(d("2026-12-30")));
    }

    #[test]
    fn fast_count_matches_walking_the_days() {
        let cal = business(vec![
            year_end(),
            custom("夏季休業", "2026-08-12", "2026-08-14", true),
        ]);
        let start = d("2025-12-20");
        for offset in [0u32, 1, 2, 5, 6, 7, 13, 40, 100, 380, 800] {
            for from in 0..14u32 {
                let a = add_days(start, from);
                let b = add_days(a, offset);
                assert_eq!(cal.business_days_in(a, b), walk(&cal, a, b), "{a}..{b}");
            }
        }
        assert_eq!(cal.business_days_in(d("2026-11-20"), d("2026-11-19")), 0);
    }

    #[test]
    fn weekday_count_over_a_long_period_and_old_dates() {
        let cal = WorkCalendar::new(DayCount::Business, vec![]);
        // 2100 has no national holidays: 52 weeks + 1 day (Friday 1/1 … Friday 12/31).
        assert_eq!(cal.business_days_in(d("2099-12-31"), d("2100-12-31")), 261);
        assert_eq!(days_between(d("2099-12-31"), d("2100-12-31")), 365);
        let old = WorkCalendar::default();
        assert_eq!(old.business_days_in(d("0001-01-01"), d("0001-01-08")), 5);
    }

    #[test]
    fn holidays_in_names_national_and_custom_days() {
        let cal = WorkCalendar::new(DayCount::Calendar, vec![year_end()]);
        let hs = cal.holidays_in(d("2026-12-30"), d("2027-01-02"));
        let rows: Vec<(String, &str, bool)> = hs
            .iter()
            .map(|h| (h.date.to_string(), h.name.as_str(), h.custom))
            .collect();
        assert_eq!(
            rows,
            vec![
                ("2026-12-30".into(), "年末年始", true),
                ("2026-12-31".into(), "年末年始", true),
                ("2027-01-01".into(), "元日", false),
                ("2027-01-01".into(), "年末年始", true),
                ("2027-01-02".into(), "年末年始", true),
            ]
        );
    }

    #[test]
    fn custom_holidays_are_checked() {
        let ok = custom(" 年末年始 ", "2026-12-29", "2027-01-03", true);
        assert_eq!(ok.checked().unwrap().name, "年末年始");
        assert_eq!(
            custom("  ", "2026-12-29", "2026-12-29", false).checked(),
            Err(CustomHolidayError::EmptyName)
        );
        assert_eq!(
            custom(&"休".repeat(31), "2026-12-29", "2026-12-29", false).checked(),
            Err(CustomHolidayError::NameTooLong)
        );
        assert!(
            custom(&"休".repeat(30), "2026-12-29", "2026-12-29", false)
                .checked()
                .is_ok()
        );
        assert_eq!(
            custom("逆", "2026-12-30", "2026-12-29", false).checked(),
            Err(CustomHolidayError::Reversed)
        );
        assert!(
            custom("31日", "2026-08-01", "2026-08-31", false)
                .checked()
                .is_ok()
        );
        assert_eq!(
            custom("32日", "2026-08-01", "2026-09-01", false).checked(),
            Err(CustomHolidayError::TooLong)
        );
    }

    #[test]
    fn yearly_leap_day_falls_on_february_28() {
        let cal = business(vec![custom("うるう日", "2028-02-29", "2028-02-29", true)]);
        assert!(!cal.is_business_day(d("2028-02-29")));
        assert!(!cal.is_business_day(d("2029-02-28"))); // Wednesday
        assert!(cal.is_business_day(d("2029-03-01")));
    }
}
