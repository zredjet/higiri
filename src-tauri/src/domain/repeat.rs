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
//! Closing a repeating TODO (完了 or 中止) makes the next round (phase 21):
//! [`next_due`] decides its 日限, [`RoundDates`] moves its 予定期間 (and its
//! children's dates) by as many days as the 日限 moved, and the repetition
//! moves to the new round. Only parentless, open TODOs with a 日限 get a
//! repetition ([`check_target`]).

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Days, Months, NaiveDate, Utc, Weekday};
use serde::{Deserialize, Serialize};

use super::date::{days_between, month_day_ja};
use super::model::{Status, Todo};
use super::urgency::Rules;

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

impl DayOfWeek {
    pub const ALL: [DayOfWeek; 7] = [
        DayOfWeek::Mon,
        DayOfWeek::Tue,
        DayOfWeek::Wed,
        DayOfWeek::Thu,
        DayOfWeek::Fri,
        DayOfWeek::Sat,
        DayOfWeek::Sun,
    ];

    pub fn weekday(self) -> Weekday {
        match self {
            DayOfWeek::Mon => Weekday::Mon,
            DayOfWeek::Tue => Weekday::Tue,
            DayOfWeek::Wed => Weekday::Wed,
            DayOfWeek::Thu => Weekday::Thu,
            DayOfWeek::Fri => Weekday::Fri,
            DayOfWeek::Sat => Weekday::Sat,
            DayOfWeek::Sun => Weekday::Sun,
        }
    }

    /// The day of the week of `date`.
    pub fn of(date: NaiveDate) -> DayOfWeek {
        DayOfWeek::ALL[date.weekday().num_days_from_monday() as usize]
    }
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

/// What is out of range in a rule. The text is shown as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RepeatRuleError {
    /// 毎週 without any day.
    #[error("曜日が選ばれていません。曜日を1つ以上選んでください。")]
    NoWeekday,
    /// 毎週 with the same day twice.
    #[error("同じ曜日が2回選ばれています。曜日を選び直してください。")]
    WeekdayTwice,
    /// 毎月(日付): not 1–31.
    #[error("日付が1〜31日の範囲にありません。1〜31日から選んでください。")]
    DayOutOfRange,
    /// 毎月(第N◯曜日): not 1–4 or −1.
    #[error("週が第1〜第4週と最終週のどれでもありません。週を選び直してください。")]
    WeekOutOfRange,
    /// 毎年: a month and day no year has.
    #[error("暦にない月日です。月日を選び直してください。")]
    NoSuchDate,
    /// N日ごと: not 2–365.
    #[error("日数が2〜365日の範囲にありません。2〜365日で入力してください。")]
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

/// Repetitions by TODO id (lists read them all at once).
pub type RepeatsByTodo = HashMap<i64, TodoRepeat>;

pub fn by_todo(repeats: Vec<TodoRepeat>) -> RepeatsByTodo {
    repeats.into_iter().map(|r| (r.todo_id, r)).collect()
}

impl TodoRepeat {
    /// The 日限 of the round made when this one (due `due`) is closed today,
    /// or `None` when the repetition ends here: the next 日限 falls after
    /// `until_date`, or there is no 日限 to count from (only data the app
    /// did not make).
    pub fn next_round_due(
        &self,
        due: Option<NaiveDate>,
        today: NaiveDate,
        rules: &Rules,
    ) -> Option<NaiveDate> {
        let next = next_due(&self.rule, due?, today, rules)?;
        if self.until_date.is_some_and(|until| next > until) {
            return None;
        }
        Some(next)
    }

    /// What a TODO's item carries (`TodoItem.repeat`).
    pub fn info(&self, due: Option<NaiveDate>, today: NaiveDate, rules: &Rules) -> RepeatInfo {
        RepeatInfo {
            rule: self.rule.clone(),
            until_date: self.until_date,
            next_due: self.next_round_due(due, today, rules),
        }
    }
}

/// The repetition of a TODO as the screens get it (`TodoItem.repeat`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct RepeatInfo {
    pub rule: RepeatRule,
    /// The last day a round may fall on. `null` = no end.
    pub until_date: Option<NaiveDate>,
    /// The next round's 日限 if this one is closed today. `null` when the
    /// repetition would end (after `until_date`).
    pub next_due: Option<NaiveDate>,
}

/// Why a TODO cannot get a repetition. The text is shown as it is.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepeatTargetError {
    #[error(
        "子TODOには繰り返しを付けられません(繰り返しを付けられるのは親のないTODOだけです)。親から外してから付けてください。"
    )]
    Child,

    #[error("{}にしたTODOには繰り返しを付けられません。ステータスを戻してから付けてください。", .0.label())]
    Closed(Status),

    #[error("繰り返しを付けるには日限が要ります。先に日限を入力してください。")]
    NoDue,

    #[error(
        "終わりの日が今の日限({})より前です。日限と同じ日か、それより後の日を選んでください。",
        month_day_ja(*.0)
    )]
    UntilBeforeDue(NaiveDate),
}

/// Whether `todo` may get (or keep, with a new rule) a repetition ending on
/// `until_date`: no parent, open, with a 日限, and the end not before it.
pub fn check_target(todo: &Todo, until_date: Option<NaiveDate>) -> Result<(), RepeatTargetError> {
    if todo.parent_id.is_some() {
        return Err(RepeatTargetError::Child);
    }
    if todo.status.is_closed() {
        return Err(RepeatTargetError::Closed(todo.status));
    }
    let due = todo.due_date.ok_or(RepeatTargetError::NoDue)?;
    if until_date.is_some_and(|until| until < due) {
        return Err(RepeatTargetError::UntilBeforeDue(due));
    }
    Ok(())
}

/// Rounds skipped at most while looking for one whose moved 日限 is after
/// the previous round (営業日). Far more than any run of holidays.
const MAX_SKIPS: usize = 400;

/// The next round's 日限 (設計書「次の回の日限の決め方」):
///
/// 1. The first day after `prev_due` that the rule falls on.
/// 2. When that is before `today` (closed late), the first such day from
///    `today` on instead: rounds already past are not made.
/// 3. In 営業日, one rule: a day whose 日限 the judgement moves (to the
///    business day before a holiday) onto or before `prev_due`, or before
///    `today`, is skipped. A daily round on Saturday would be judged on
///    Friday again, and one made on a holiday would be 超過 at once
///    (decided by the orchestrator, phase 21).
///
/// `every_days` counts from `prev_due`. In 暦日, 3 changes nothing. `None`
/// only past the last representable date.
pub fn next_due(
    rule: &RepeatRule,
    prev_due: NaiveDate,
    today: NaiveDate,
    rules: &Rules,
) -> Option<NaiveDate> {
    let from = prev_due.succ_opt()?.max(today);
    let mut day = first_on_or_after(rule, from, prev_due)?;
    for _ in 0..MAX_SKIPS {
        let judged = rules.calendar.effective_due(day);
        if judged > prev_due && judged >= today {
            return Some(day);
        }
        day = first_on_or_after(rule, day.succ_opt()?, prev_due)?;
    }
    Some(day)
}

/// The first day on or after `from` that `rule` falls on. `anchor` is the
/// day `every_days` counts from (before `from`).
fn first_on_or_after(rule: &RepeatRule, from: NaiveDate, anchor: NaiveDate) -> Option<NaiveDate> {
    match rule {
        RepeatRule::Daily => Some(from),
        RepeatRule::Weekly { weekdays } => (0..7)
            .filter_map(|i| from.checked_add_days(Days::new(i)))
            .find(|day| weekdays.contains(&DayOfWeek::of(*day))),
        RepeatRule::MonthlyDay { day } => first_in_months(from, |y, m| day_in_month(y, m, *day)),
        RepeatRule::MonthlyWeekday { week, weekday } => {
            first_in_months(from, |y, m| weekday_in_month(y, m, *week, *weekday))
        }
        RepeatRule::MonthlyLast => last_of_month(from.year(), from.month()),
        RepeatRule::Yearly { month, day } => {
            let on = |year: i32| {
                NaiveDate::from_ymd_opt(year, *month, *day).or_else(|| {
                    // 2/29 is 2/28 in other years.
                    (*month == 2 && *day == 29)
                        .then(|| NaiveDate::from_ymd_opt(year, 2, 28))
                        .flatten()
                })
            };
            match on(from.year()) {
                Some(date) if date >= from => Some(date),
                _ => on(from.year().checked_add(1)?),
            }
        }
        RepeatRule::EveryDays { days } => {
            let step = i64::from((*days).max(1));
            let behind = days_between(anchor, from).max(1);
            let rounds = (behind + step - 1) / step;
            anchor.checked_add_days(Days::new(u64::try_from(rounds * step).ok()?))
        }
    }
}

/// The day `on` gives in `from`'s month when it is not before `from`, else
/// the one in the next month.
fn first_in_months(
    from: NaiveDate,
    on: impl Fn(i32, u32) -> Option<NaiveDate>,
) -> Option<NaiveDate> {
    match on(from.year(), from.month()) {
        Some(date) if date >= from => Some(date),
        _ => {
            let next = from.with_day(1)?.checked_add_months(Months::new(1))?;
            on(next.year(), next.month())
        }
    }
}

fn last_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, 1)?
        .checked_add_months(Months::new(1))?
        .pred_opt()
}

/// `day` of the month, or its last day when it has no such day.
fn day_in_month(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    let last = last_of_month(year, month)?;
    NaiveDate::from_ymd_opt(year, month, day.min(last.day()))
}

/// The `week`-th `weekday` of the month (−1 = the last one).
fn weekday_in_month(year: i32, month: u32, week: i32, weekday: DayOfWeek) -> Option<NaiveDate> {
    let wd = weekday.weekday();
    if week == -1 {
        let last = last_of_month(year, month)?;
        let back = (7 + last.weekday().num_days_from_monday() - wd.num_days_from_monday()) % 7;
        return last.checked_sub_days(Days::new(u64::from(back)));
    }
    NaiveDate::from_weekday_of_month_opt(year, month, wd, u8::try_from(week).ok()?)
}

/// `date` moved by `days` calendar days (`None` stays `None`, and so does a
/// day past the representable range).
pub fn shift(date: Option<NaiveDate>, days: i64) -> Option<NaiveDate> {
    let date = date?;
    let by = Days::new(days.unsigned_abs());
    if days >= 0 {
        date.checked_add_days(by)
    } else {
        date.checked_sub_days(by)
    }
}

/// The dates a TODO takes into the next round, each moved by as many days
/// as the 日限 moved. A missing date stays missing (a 予定期間 with one end
/// keeps only that end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundDates {
    pub due_date: Option<NaiveDate>,
    pub planned_start: Option<NaiveDate>,
    pub planned_end: Option<NaiveDate>,
}

impl RoundDates {
    pub fn of(todo: &Todo) -> RoundDates {
        RoundDates {
            due_date: todo.due_date,
            planned_start: todo.planned_start,
            planned_end: todo.planned_end,
        }
    }

    pub fn shifted(self, days: i64) -> RoundDates {
        RoundDates {
            due_date: shift(self.due_date, days),
            planned_start: shift(self.planned_start, days),
            planned_end: shift(self.planned_end, days),
        }
    }
}

/// The children copied into the next round: all but those already 中止
/// before the parent was closed (`before` = the children as they were
/// then). Children closed with the parent (中止 too) are copied.
pub fn copied_children(before: &[Todo]) -> impl Iterator<Item = &Todo> {
    before.iter().filter(|c| c.status != Status::Cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{business_rules, d, rules, todo, ts};
    use crate::domain::urgency::{Urgency, classify};

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

    // ---- 次の回 (phase 21) ---------------------------------------------------

    fn next(rule: &RepeatRule, prev: &str, today: &str) -> NaiveDate {
        next_due(rule, d(prev), d(today), &rules(3)).unwrap()
    }

    fn next_business(rule: &RepeatRule, prev: &str, today: &str) -> NaiveDate {
        next_due(rule, d(prev), d(today), &business_rules(3, vec![])).unwrap()
    }

    /// The days a rule gives one after another from `prev`, looking early.
    fn chain(rule: &RepeatRule, prev: &str, n: usize) -> Vec<NaiveDate> {
        let mut out = Vec::new();
        let mut day = d(prev);
        for _ in 0..n {
            day = next_due(rule, day, d("2000-01-01"), &rules(3)).unwrap();
            out.push(day);
        }
        out
    }

    fn days(list: &[&str]) -> Vec<NaiveDate> {
        list.iter().map(|s| d(s)).collect()
    }

    fn weekly(list: &[DayOfWeek]) -> RepeatRule {
        RepeatRule::Weekly {
            weekdays: list.to_vec(),
        }
    }

    #[test]
    fn day_of_the_week_of_a_date() {
        // 2026-11-16 is a Monday.
        for (i, wd) in DayOfWeek::ALL.into_iter().enumerate() {
            let date = d("2026-11-16") + Days::new(i as u64);
            assert_eq!(DayOfWeek::of(date), wd);
            assert_eq!(wd.weekday(), date.weekday());
        }
    }

    #[test]
    fn daily_is_the_next_day() {
        assert_eq!(
            next(&RepeatRule::Daily, "2026-11-19", "2026-11-19"),
            d("2026-11-20")
        );
        // Across the year.
        assert_eq!(
            next(&RepeatRule::Daily, "2026-12-31", "2026-12-01"),
            d("2027-01-01")
        );
    }

    #[test]
    fn the_thirty_first_of_every_month_falls_back_to_the_last_day() {
        let rule = RepeatRule::MonthlyDay { day: 31 };
        assert_eq!(
            chain(&rule, "2027-01-31", 4),
            days(&["2027-02-28", "2027-03-31", "2027-04-30", "2027-05-31"])
        );
        // A leap year has 2/29.
        assert_eq!(chain(&rule, "2028-01-31", 1), days(&["2028-02-29"]));
        // From the 15th: this month's 15th is gone, the next month's comes.
        assert_eq!(
            chain(&RepeatRule::MonthlyDay { day: 15 }, "2026-12-15", 2),
            days(&["2027-01-15", "2027-02-15"])
        );
    }

    #[test]
    fn february_29_every_year_is_february_28_in_other_years() {
        let rule = RepeatRule::Yearly { month: 2, day: 29 };
        assert_eq!(
            chain(&rule, "2028-02-29", 4),
            days(&["2029-02-28", "2030-02-28", "2031-02-28", "2032-02-29"])
        );
        let rule = RepeatRule::Yearly { month: 3, day: 31 };
        assert_eq!(chain(&rule, "2026-03-31", 1), days(&["2027-03-31"]));
        // Later in the same year.
        assert_eq!(chain(&rule, "2026-01-10", 1), days(&["2026-03-31"]));
    }

    #[test]
    fn the_second_tuesday_and_the_last_friday() {
        let second_tuesday = RepeatRule::MonthlyWeekday {
            week: 2,
            weekday: DayOfWeek::Tue,
        };
        // 2026-11-10 is November's second Tuesday; December 1 is a Tuesday.
        assert_eq!(
            chain(&second_tuesday, "2026-11-10", 3),
            days(&["2026-12-08", "2027-01-12", "2027-02-09"])
        );
        // Earlier in the month: this month's.
        assert_eq!(
            chain(&second_tuesday, "2026-11-02", 1),
            days(&["2026-11-10"])
        );

        let last_friday = RepeatRule::MonthlyWeekday {
            week: -1,
            weekday: DayOfWeek::Fri,
        };
        // 2027-01-01 is a Friday, and so is the 29th.
        assert_eq!(
            chain(&last_friday, "2026-11-27", 3),
            days(&["2026-12-25", "2027-01-29", "2027-02-26"])
        );
        let fourth_sunday = RepeatRule::MonthlyWeekday {
            week: 4,
            weekday: DayOfWeek::Sun,
        };
        // November 2026 starts on a Sunday: its fourth is the 22nd (the
        // 29th is the fifth, and the last).
        assert_eq!(
            chain(&fourth_sunday, "2026-11-01", 1),
            days(&["2026-11-22"])
        );
    }

    #[test]
    fn the_last_day_of_every_month() {
        assert_eq!(
            chain(&RepeatRule::MonthlyLast, "2027-01-31", 3),
            days(&["2027-02-28", "2027-03-31", "2027-04-30"])
        );
        assert_eq!(
            chain(&RepeatRule::MonthlyLast, "2026-11-19", 1),
            days(&["2026-11-30"])
        );
    }

    #[test]
    fn weekly_on_several_days_goes_across_the_week() {
        let rule = weekly(&[DayOfWeek::Thu, DayOfWeek::Mon]);
        // Thu 11/19 → Mon 11/23 → Thu 11/26 → Mon 11/30.
        assert_eq!(
            chain(&rule, "2026-11-19", 3),
            days(&["2026-11-23", "2026-11-26", "2026-11-30"])
        );
        // One day: a week later. Across the year.
        assert_eq!(
            chain(&weekly(&[DayOfWeek::Fri]), "2026-12-25", 1),
            days(&["2027-01-01"])
        );
    }

    #[test]
    fn every_n_days_counts_from_the_previous_round() {
        let rule = RepeatRule::EveryDays { days: 10 };
        assert_eq!(
            chain(&rule, "2026-12-20", 2),
            days(&["2026-12-30", "2027-01-09"])
        );
        // Closed late: the rounds 11/29 and 12/9 are past, 12/19 comes.
        assert_eq!(next(&rule, "2026-11-19", "2026-12-15"), d("2026-12-19"));
        // On a round itself.
        assert_eq!(next(&rule, "2026-11-19", "2026-12-09"), d("2026-12-09"));
    }

    #[test]
    fn closing_late_skips_the_rounds_already_past() {
        let monday = weekly(&[DayOfWeek::Mon]);
        // Due Mon 11/2, closed Thu 11/19: not 11/9 or 11/16 but 11/23.
        assert_eq!(next(&monday, "2026-11-02", "2026-11-19"), d("2026-11-23"));
        assert_eq!(
            next(
                &RepeatRule::MonthlyDay { day: 5 },
                "2026-08-05",
                "2026-11-19"
            ),
            d("2026-12-05")
        );
        assert_eq!(
            next(&RepeatRule::Daily, "2026-11-02", "2026-11-19"),
            d("2026-11-19")
        );
    }

    #[test]
    fn a_round_on_today_is_made_for_today() {
        let thursday = weekly(&[DayOfWeek::Thu]);
        // Due Thu 11/12, closed Thu 11/19 (a week late).
        assert_eq!(next(&thursday, "2026-11-12", "2026-11-19"), d("2026-11-19"));
        // Closed on its own day: the next week.
        assert_eq!(next(&thursday, "2026-11-19", "2026-11-19"), d("2026-11-26"));
        // Closed early.
        assert_eq!(next(&thursday, "2026-11-19", "2026-11-10"), d("2026-11-26"));
    }

    #[test]
    fn in_business_days_daily_goes_from_friday_to_monday() {
        // Fri 11/27 → not Sat or Sun (judged on Friday again) but Mon 11/30.
        assert_eq!(
            next_business(&RepeatRule::Daily, "2026-11-27", "2026-11-27"),
            d("2026-11-30")
        );
        // Mon 11/23/2026 is 勤労感謝の日: Fri 11/20 → Tue 11/24.
        assert_eq!(
            next_business(&RepeatRule::Daily, "2026-11-20", "2026-11-20"),
            d("2026-11-24")
        );
        // Mid-week nothing changes.
        assert_eq!(
            next_business(&RepeatRule::Daily, "2026-11-17", "2026-11-17"),
            d("2026-11-18")
        );
        // In 暦日, Saturday.
        assert_eq!(
            next(&RepeatRule::Daily, "2026-11-27", "2026-11-27"),
            d("2026-11-28")
        );
    }

    #[test]
    fn in_business_days_the_first_on_a_sunday_stays_and_is_judged_on_friday() {
        let rule = RepeatRule::MonthlyDay { day: 1 };
        // 2026-11-01 is a Sunday: judged on Fri 10/30, still after 10/1.
        let r = business_rules(3, vec![]);
        let due = next_due(&rule, d("2026-10-01"), d("2026-09-20"), &r).unwrap();
        assert_eq!(due, d("2026-11-01"));
        assert_eq!(r.calendar.effective_due(due), d("2026-10-30"));
        assert_eq!(
            next_business(&rule, "2026-11-01", "2026-10-30"),
            d("2026-12-01")
        );
    }

    #[test]
    fn in_business_days_closing_late_on_a_holiday_makes_no_round_already_past() {
        // Due Thu 11/26, closed Sat 11/28: Sat and Sun are judged on Friday,
        // before today, so Monday.
        assert_eq!(
            next_business(&RepeatRule::Daily, "2026-11-26", "2026-11-28"),
            d("2026-11-30")
        );
        // In 暦日, today.
        assert_eq!(
            next(&RepeatRule::Daily, "2026-11-26", "2026-11-28"),
            d("2026-11-28")
        );
        // Weekly on Saturday: the next Saturday is judged on Friday, after
        // the previous one.
        let saturday = weekly(&[DayOfWeek::Sat]);
        assert_eq!(
            next_business(&saturday, "2026-11-21", "2026-11-21"),
            d("2026-11-28")
        );
    }

    #[test]
    fn in_business_days_closing_late_on_a_weekday_makes_todays_round() {
        // Due Thu 11/26, closed Fri 11/27: Friday's round, due today (本日).
        let r = business_rules(3, vec![]);
        let today = d("2026-11-27");
        let due = next_due(&RepeatRule::Daily, d("2026-11-26"), today, &r).unwrap();
        assert_eq!(due, today);
        let round = Todo {
            due_date: Some(due),
            ..todo(Status::Todo)
        };
        assert_eq!(classify(&round, today, &r), Some(Urgency::DueToday));
        // The same in 暦日.
        assert_eq!(next(&RepeatRule::Daily, "2026-11-26", "2026-11-27"), today);
    }

    fn repeat(rule: RepeatRule, until: Option<&str>) -> TodoRepeat {
        TodoRepeat {
            todo_id: 1,
            rule,
            until_date: until.map(d),
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        }
    }

    #[test]
    fn the_repetition_ends_after_its_last_day() {
        let monday = weekly(&[DayOfWeek::Mon]);
        let today = d("2026-11-19");
        let due = Some(d("2026-11-23"));
        let r = rules(3);
        assert_eq!(
            repeat(monday.clone(), None).next_round_due(due, today, &r),
            Some(d("2026-11-30"))
        );
        // The end day itself is still a round.
        assert_eq!(
            repeat(monday.clone(), Some("2026-11-30")).next_round_due(due, today, &r),
            Some(d("2026-11-30"))
        );
        assert_eq!(
            repeat(monday.clone(), Some("2026-11-29")).next_round_due(due, today, &r),
            None
        );
        // No 日限 to count from (not made by the app).
        assert_eq!(
            repeat(monday.clone(), None).next_round_due(None, today, &r),
            None
        );

        let info = repeat(monday.clone(), Some("2026-12-31")).info(due, today, &r);
        assert_eq!(
            info,
            RepeatInfo {
                rule: monday,
                until_date: Some(d("2026-12-31")),
                next_due: Some(d("2026-11-30")),
            }
        );
    }

    #[test]
    fn only_an_open_parentless_todo_with_a_due_date_gets_a_repetition() {
        let due = Todo {
            due_date: Some(d("2026-11-20")),
            ..todo(Status::Todo)
        };
        assert_eq!(check_target(&due, None), Ok(()));
        assert_eq!(check_target(&due, Some(d("2026-11-20"))), Ok(()));
        for status in [Status::Doing, Status::Waiting, Status::Hold] {
            assert_eq!(
                check_target(
                    &Todo {
                        status,
                        ..due.clone()
                    },
                    None
                ),
                Ok(())
            );
        }

        let child = Todo {
            parent_id: Some(9),
            ..due.clone()
        };
        assert_eq!(check_target(&child, None), Err(RepeatTargetError::Child));
        let done = Todo {
            status: Status::Done,
            ..due.clone()
        };
        assert_eq!(
            check_target(&done, None).unwrap_err().to_string(),
            "完了にしたTODOには繰り返しを付けられません。ステータスを戻してから付けてください。"
        );
        let cancelled = Todo {
            status: Status::Cancelled,
            ..due.clone()
        };
        assert_eq!(
            check_target(&cancelled, None),
            Err(RepeatTargetError::Closed(Status::Cancelled))
        );
        assert_eq!(
            check_target(&todo(Status::Todo), None),
            Err(RepeatTargetError::NoDue)
        );
        assert_eq!(
            check_target(&due, Some(d("2026-11-19")))
                .unwrap_err()
                .to_string(),
            "終わりの日が今の日限(11月20日)より前です。日限と同じ日か、それより後の日を選んでください。"
        );
    }

    #[test]
    fn dates_move_by_the_days_the_due_date_moved() {
        let t = Todo {
            due_date: Some(d("2026-11-23")),
            planned_start: Some(d("2026-11-16")),
            planned_end: Some(d("2026-11-20")),
            ..todo(Status::Done)
        };
        assert_eq!(
            RoundDates::of(&t).shifted(7),
            RoundDates {
                due_date: Some(d("2026-11-30")),
                planned_start: Some(d("2026-11-23")),
                planned_end: Some(d("2026-11-27")),
            }
        );
        // One end only; no 日限 (a child) stays without.
        let one = Todo {
            due_date: None,
            planned_start: None,
            planned_end: Some(d("2026-12-30")),
            ..todo(Status::Todo)
        };
        assert_eq!(
            RoundDates::of(&one).shifted(3),
            RoundDates {
                due_date: None,
                planned_start: None,
                planned_end: Some(d("2027-01-02")),
            }
        );
        assert_eq!(shift(Some(d("2026-03-01")), -1), Some(d("2026-02-28")));
        assert_eq!(shift(Some(NaiveDate::MAX), 1), None);
    }

    #[test]
    fn children_cancelled_before_are_not_copied() {
        let child = |id: i64, status: Status| Todo {
            id,
            status,
            ..todo(status)
        };
        let before = [
            child(1, Status::Todo),
            child(2, Status::Done),
            child(3, Status::Cancelled),
            child(4, Status::Waiting),
        ];
        let ids: Vec<i64> = copied_children(&before).map(|c| c.id).collect();
        assert_eq!(ids, vec![1, 2, 4]);
    }
}
