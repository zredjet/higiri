//! 日本の祝日 (Japanese national holidays), computed from the rules of the
//! 国民の祝日に関する法律 as in force from 2020. Pure.
//!
//! Only 2020–2099 are computed (decided in the design): fixed days, the
//! Mondays of the Happy Monday system, the equinoxes (a formula valid for
//! these years), 振替休日, 国民の休日, and the moves for the Tokyo Olympics in
//! 2020 and 2021. Earlier amendments are not modelled. Other years have no
//! national holidays (only weekends are off).

use std::sync::OnceLock;

use chrono::{Datelike, NaiveDate, Weekday};

/// First and last year with national holidays.
pub const FIRST_YEAR: i32 = 2020;
pub const LAST_YEAR: i32 = 2099;

/// A 祝日 on Sunday moves to the next day that is not a 祝日 (第3条第2項).
pub const SUBSTITUTE: &str = "振替休日";
/// A day between two 祝日 (第3条第3項).
pub const CITIZENS: &str = "国民の休日";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NationalHoliday {
    pub date: NaiveDate,
    pub name: &'static str,
}

fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid holiday date")
}

fn nth_monday(year: i32, month: u32, n: u8) -> NaiveDate {
    NaiveDate::from_weekday_of_month_opt(year, month, Weekday::Mon, n).expect("valid Monday")
}

/// Day of the equinox in March (`base` 20.8431) or September (23.2488).
/// The usual approximation, valid from 1980 to 2099.
fn equinox_day(year: i32, base: f64) -> u32 {
    let n = year - 1980;
    (base + 0.242194 * f64::from(n) - f64::from(n / 4)).floor() as u32
}

/// The 祝日 named in 第2条, in date order.
fn named(year: i32) -> Vec<NationalHoliday> {
    // 海の日, スポーツの日 and 山の日 moved for the Olympics (special measures act).
    let (marine, sports, mountain) = match year {
        2020 => (ymd(2020, 7, 23), ymd(2020, 7, 24), ymd(2020, 8, 10)),
        2021 => (ymd(2021, 7, 22), ymd(2021, 7, 23), ymd(2021, 8, 8)),
        _ => (
            nth_monday(year, 7, 3),
            nth_monday(year, 10, 2),
            ymd(year, 8, 11),
        ),
    };
    let mut days = vec![
        (ymd(year, 1, 1), "元日"),
        (nth_monday(year, 1, 2), "成人の日"),
        (ymd(year, 2, 11), "建国記念の日"),
        (ymd(year, 2, 23), "天皇誕生日"),
        (ymd(year, 3, equinox_day(year, 20.8431)), "春分の日"),
        (ymd(year, 4, 29), "昭和の日"),
        (ymd(year, 5, 3), "憲法記念日"),
        (ymd(year, 5, 4), "みどりの日"),
        (ymd(year, 5, 5), "こどもの日"),
        (marine, "海の日"),
        (mountain, "山の日"),
        (nth_monday(year, 9, 3), "敬老の日"),
        (ymd(year, 9, equinox_day(year, 23.2488)), "秋分の日"),
        (sports, "スポーツの日"),
        (ymd(year, 11, 3), "文化の日"),
        (ymd(year, 11, 23), "勤労感謝の日"),
    ];
    days.sort_by_key(|(d, _)| *d);
    days.into_iter()
        .map(|(date, name)| NationalHoliday { date, name })
        .collect()
}

/// The national holidays of `year` in date order (empty outside 2020–2099).
pub fn national_holidays(year: i32) -> Vec<NationalHoliday> {
    if !(FIRST_YEAR..=LAST_YEAR).contains(&year) {
        return Vec::new();
    }
    let named = named(year);
    let is_named = |d: NaiveDate| named.iter().any(|h| h.date == d);
    let mut out = named.clone();

    for h in &named {
        if h.date.weekday() != Weekday::Sun {
            continue;
        }
        let mut day = h.date;
        while let Some(next) = day.succ_opt() {
            day = next;
            if !is_named(day) {
                break;
            }
        }
        if !out.iter().any(|o| o.date == day) {
            out.push(NationalHoliday {
                date: day,
                name: SUBSTITUTE,
            });
        }
    }

    for pair in named.windows(2) {
        let (a, b) = (pair[0].date, pair[1].date);
        if (b - a).num_days() != 2 {
            continue;
        }
        let between = a.succ_opt().expect("date between two holidays");
        if !out.iter().any(|o| o.date == between) {
            out.push(NationalHoliday {
                date: between,
                name: CITIZENS,
            });
        }
    }

    out.sort_by_key(|h| h.date);
    out
}

/// Every national holiday from 2020 to 2099, in date order. Computed once.
pub fn all() -> &'static [NationalHoliday] {
    static ALL: OnceLock<Vec<NationalHoliday>> = OnceLock::new();
    ALL.get_or_init(|| {
        (FIRST_YEAR..=LAST_YEAR)
            .flat_map(national_holidays)
            .collect()
    })
}

/// The national holidays from `start` to `end` (both included).
pub fn between(start: NaiveDate, end: NaiveDate) -> &'static [NationalHoliday] {
    let all = all();
    let from = all.partition_point(|h| h.date < start);
    let to = all.partition_point(|h| h.date <= end);
    if from >= to { &[] } else { &all[from..to] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::date::parse_date;
    use crate::domain::testing::d;

    /// The Cabinet Office's list (内閣府「国民の祝日について」, syukujitsu.csv)
    /// for 2020–2027, where 「休日」 is a 振替休日 or a 国民の休日.
    const CAO_2020_2027: &str = include_str!("../../tests/fixtures/cao_holidays_2020-2027.txt");

    fn cabinet_office_list() -> Vec<(NaiveDate, &'static str)> {
        CAO_2020_2027
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| {
                let (date, name) = l.split_once(' ').expect("date and name");
                (parse_date(date).expect("date"), name)
            })
            .collect()
    }

    #[test]
    fn matches_the_cabinet_office_list_from_2020_to_2027() {
        let expected = cabinet_office_list();
        assert_eq!(expected.len(), 143);
        let actual: Vec<NationalHoliday> = (2020..=2027).flat_map(national_holidays).collect();
        let dates = |v: &[(NaiveDate, &str)]| v.iter().map(|(d, _)| *d).collect::<Vec<_>>();
        assert_eq!(
            actual.iter().map(|h| h.date).collect::<Vec<_>>(),
            dates(&expected)
        );
        for (h, (date, name)) in actual.iter().zip(&expected) {
            if *name == "休日" {
                assert!(
                    h.name == SUBSTITUTE || h.name == CITIZENS,
                    "{date}: {}",
                    h.name
                );
            } else {
                assert_eq!(h.name, *name, "{date}");
            }
        }
    }

    #[test]
    fn substitute_and_citizens_holidays_are_told_apart() {
        let name = |date: &str| {
            national_holidays(d(date).year())
                .into_iter()
                .find(|h| h.date == d(date))
                .map(|h| h.name)
        };
        // 2026: 5/3 is a Sunday, 5/4 and 5/5 are holidays, so 5/6 is the substitute.
        assert_eq!(name("2026-05-06"), Some(SUBSTITUTE));
        // 2026: 敬老の日 9/21 and 秋分の日 9/23 enclose 9/22.
        assert_eq!(name("2026-09-22"), Some(CITIZENS));
        // 2021: 山の日 moved to Sunday 8/8 for the Olympics, so 8/9 is off.
        assert_eq!(name("2021-08-08"), Some("山の日"));
        assert_eq!(name("2021-08-09"), Some(SUBSTITUTE));
        // 2026-11-23 (Monday) is 勤労感謝の日.
        assert_eq!(name("2026-11-23"), Some("勤労感謝の日"));
        assert_eq!(name("2026-11-24"), None);
    }

    #[test]
    fn only_2020_to_2099_have_national_holidays() {
        assert!(national_holidays(2019).is_empty());
        assert!(national_holidays(2100).is_empty());
        assert_eq!(
            national_holidays(2099).first().map(|h| h.date),
            Some(d("2099-01-01"))
        );
        assert!(between(d("2100-01-01"), d("2100-12-31")).is_empty());
    }

    #[test]
    fn every_year_is_sorted_distinct_and_complete() {
        for year in FIRST_YEAR..=LAST_YEAR {
            let hs = national_holidays(year);
            assert!(hs.len() >= 16, "{year}");
            assert!(hs.iter().all(|h| h.date.year() == year), "{year}");
            assert!(hs.windows(2).all(|w| w[0].date < w[1].date), "{year}");
            // No holiday is left on a Sunday without a substitute after it.
            for h in hs.iter().filter(|h| h.date.weekday() == Weekday::Sun) {
                let mut day = h.date.succ_opt().unwrap();
                while hs.iter().any(|o| o.date == day && o.name != SUBSTITUTE) {
                    day = day.succ_opt().unwrap();
                }
                assert!(
                    hs.iter().any(|o| o.date == day && o.name == SUBSTITUTE) || day.year() != year,
                    "{year}: {}",
                    h.date
                );
            }
        }
        assert_eq!(
            all().len(),
            (FIRST_YEAR..=LAST_YEAR)
                .map(|y| national_holidays(y).len())
                .sum::<usize>()
        );
    }

    #[test]
    fn between_includes_both_ends() {
        let hs = between(d("2026-11-03"), d("2026-11-23"));
        assert_eq!(
            hs.iter().map(|h| h.name).collect::<Vec<_>>(),
            vec!["文化の日", "勤労感謝の日"]
        );
        assert!(between(d("2026-11-24"), d("2026-11-30")).is_empty());
        assert!(between(d("2026-11-30"), d("2026-11-01")).is_empty());
    }
}
