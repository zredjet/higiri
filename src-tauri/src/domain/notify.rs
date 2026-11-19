//! OS notifications at fixed times (D-6, D-7). Pure: the caller passes the
//! local wall-clock time and the settings.
//!
//! - Only the latest of today's times that has come and has not been handled
//!   yet is notified, so a PC that was off or asleep gets one notification at
//!   startup or when it wakes, never one per missed time, and never one for a
//!   time of a past day.
//! - The resident timer sleeps until the next time, but never longer than
//!   [`MAX_WAIT`], and reads the clock again each time it wakes (a sleeping PC
//!   does not move the timer on).

use std::time::Duration;

use chrono::{Days, NaiveDateTime, NaiveTime, Timelike};

use super::dashboard::UrgencyCounts;
use super::urgency::Urgency;

/// The longest the resident timer sleeps before reading the clock again.
pub const MAX_WAIT: Duration = Duration::from_secs(10 * 60);
/// Number of notification times allowed.
pub const TIMES_MIN: usize = 1;
pub const TIMES_MAX: usize = 5;
/// Default notification times.
pub const DEFAULT_TIMES: [&str; 2] = ["09:00", "12:55"];

/// Why a list of notification times cannot be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimesError {
    /// Not `H:MM` / `HH:MM` (the entry as given).
    Format(String),
    TooFew,
    TooMany,
    /// The same time twice (as `HH:MM`).
    Duplicate(String),
}

/// Reads `HH:MM` (or `H:MM`), 24-hour. Full-width digits and colon, as a
/// Japanese input method may type them, are read as half-width.
pub fn parse_time(s: &str) -> Option<NaiveTime> {
    let half: String = s
        .trim()
        .chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            '：' => ':',
            _ => c,
        })
        .collect();
    let (h, m) = half.split_once(':')?;
    let digits = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
    if !digits(h) || h.len() > 2 || !digits(m) || m.len() != 2 {
        return None;
    }
    NaiveTime::from_hms_opt(h.parse().ok()?, m.parse().ok()?, 0)
}

/// Storage and display form: `HH:MM`.
pub fn format_time(t: NaiveTime) -> String {
    format!("{:02}:{:02}", t.hour(), t.minute())
}

/// Checks a list of notification times (1 to 5, each `HH:MM`, no duplicates)
/// and returns it as `HH:MM` in time order.
pub fn normalize_times(times: &[String]) -> Result<Vec<String>, TimesError> {
    let mut parsed = Vec::with_capacity(times.len());
    for s in times {
        let t = parse_time(s).ok_or_else(|| TimesError::Format(s.trim().to_owned()))?;
        if parsed.contains(&t) {
            return Err(TimesError::Duplicate(format_time(t)));
        }
        parsed.push(t);
    }
    if parsed.len() < TIMES_MIN {
        return Err(TimesError::TooFew);
    }
    if parsed.len() > TIMES_MAX {
        return Err(TimesError::TooMany);
    }
    parsed.sort();
    Ok(parsed.into_iter().map(format_time).collect())
}

/// The notification times of a saved list (unreadable entries are skipped;
/// the list was checked when it was saved).
pub fn times_of(times: &[String]) -> Vec<NaiveTime> {
    let mut out: Vec<NaiveTime> = times.iter().filter_map(|s| parse_time(s)).collect();
    out.sort();
    out.dedup();
    out
}

/// The time to notify now, if any: the latest of today's `times` that is at
/// or before `now` and after `last_notified` (when the last one was handled).
pub fn pending_slot(
    now: NaiveDateTime,
    times: &[NaiveTime],
    last_notified: Option<NaiveDateTime>,
) -> Option<NaiveDateTime> {
    let today = now.date();
    times
        .iter()
        .map(|t| today.and_time(*t))
        .filter(|slot| *slot <= now && last_notified.is_none_or(|last| *slot > last))
        .max()
}

/// How long to sleep: until the next of `times` after `now` (today's, or the
/// first one tomorrow), at most [`MAX_WAIT`].
pub fn next_wake(now: NaiveDateTime, times: &[NaiveTime]) -> Duration {
    let today = now.date();
    let later_today = times
        .iter()
        .map(|t| today.and_time(*t))
        .filter(|slot| *slot > now)
        .min();
    let next = later_today.or_else(|| {
        let tomorrow = today.checked_add_days(Days::new(1))?;
        times.iter().min().map(|t| tomorrow.and_time(*t))
    });
    next.and_then(|n| (n - now).to_std().ok())
        .map_or(MAX_WAIT, |wait| wait.min(MAX_WAIT))
}

/// Title and body of an OS notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub title: String,
    pub body: String,
}

/// The notification for the 日限注意 counts, or None when there are none.
/// The body lists only the categories with at least one TODO, in category
/// order: 「超過1・本日2」.
pub fn message(counts: &UrgencyCounts) -> Option<Message> {
    let per = |u: Urgency| match u {
        Urgency::Overdue => counts.overdue,
        Urgency::DueToday => counts.due_today,
        Urgency::DueSoon => counts.due_soon,
        Urgency::FollowUp => counts.follow_up,
        Urgency::LateStart => counts.late_start,
    };
    let total: i64 = Urgency::ALL.iter().map(|u| per(*u)).sum();
    if total <= 0 {
        return None;
    }
    let body = Urgency::ALL
        .iter()
        .filter(|u| per(**u) > 0)
        .map(|u| format!("{}{}", u.label(), per(*u)))
        .collect::<Vec<_>>()
        .join("・");
    Some(Message {
        title: format!("日限注意が{total}件あります"),
        body,
    })
}

/// Shown once, the first time × hides the window to the tray.
pub fn tray_hint() -> Message {
    Message {
        title: "Higiri はトレイで動いています".into(),
        body: "終了するときは、トレイのメニューから「終了」を選んでください。".into(),
    }
}

/// 「通知を試す」 on the settings screen.
pub fn test_message() -> Message {
    Message {
        title: "通知を試しています".into(),
        body: "この通知が見えていれば、決まった時刻に日限注意の件数をお知らせします。".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::d;

    fn t(s: &str) -> NaiveTime {
        parse_time(s).unwrap_or_else(|| panic!("bad time in test: {s}"))
    }

    fn at(date: &str, time: &str) -> NaiveDateTime {
        d(date).and_time(t(time))
    }

    fn defaults() -> Vec<NaiveTime> {
        vec![t("09:00"), t("12:55")]
    }

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn times_are_read_as_hh_mm() {
        assert_eq!(parse_time("09:00"), Some(t("09:00")));
        assert_eq!(parse_time("9:05"), Some(t("09:05")));
        assert_eq!(parse_time(" 23:59 "), Some(t("23:59")));
        assert_eq!(parse_time("０９：３０"), Some(t("09:30")));
        for bad in [
            "", "9", "9:5", "24:00", "12:60", "012:00", "09:00:00", "a9:00", "-1:00",
        ] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
        assert_eq!(format_time(t("9:05")), "09:05");
    }

    #[test]
    fn normalize_checks_count_format_and_duplicates() {
        assert_eq!(
            normalize_times(&strings(&["12:55", "9:00"])).unwrap(),
            ["09:00", "12:55"]
        );
        assert_eq!(
            normalize_times(&strings(&["9:00", "09:00"])),
            Err(TimesError::Duplicate("09:00".into()))
        );
        assert_eq!(normalize_times(&[]), Err(TimesError::TooFew));
        assert_eq!(
            normalize_times(&strings(&[
                "08:00", "09:00", "10:00", "11:00", "12:00", "13:00"
            ])),
            Err(TimesError::TooMany)
        );
        assert_eq!(
            normalize_times(&strings(&["08:00", "09:00", "10:00", "11:00", "12:00"]))
                .unwrap()
                .len(),
            5
        );
        assert_eq!(
            normalize_times(&strings(&["09:00", " 25:00 "])),
            Err(TimesError::Format("25:00".into()))
        );
        assert_eq!(
            times_of(&strings(&["12:55", "x", "09:00", "9:00"])),
            defaults()
        );
    }

    #[test]
    fn started_before_the_first_time_waits_for_it() {
        let now = at("2026-11-19", "08:59");
        assert_eq!(pending_slot(now, &defaults(), None), None);
        assert_eq!(next_wake(now, &defaults()), Duration::from_secs(60));
    }

    #[test]
    fn exactly_at_the_time_notifies_it() {
        let now = at("2026-11-19", "09:00");
        assert_eq!(
            pending_slot(now, &defaults(), None),
            Some(at("2026-11-19", "09:00"))
        );
    }

    #[test]
    fn started_after_a_time_notifies_it_once() {
        let now = at("2026-11-19", "09:30");
        assert_eq!(
            pending_slot(now, &defaults(), None),
            Some(at("2026-11-19", "09:00"))
        );
        // Handled at 9:30: nothing more until 12:55.
        let last = Some(now);
        assert_eq!(
            pending_slot(at("2026-11-19", "09:40"), &defaults(), last),
            None
        );
        assert_eq!(
            pending_slot(at("2026-11-19", "12:54"), &defaults(), last),
            None
        );
        assert_eq!(
            pending_slot(at("2026-11-19", "12:55"), &defaults(), last),
            Some(at("2026-11-19", "12:55"))
        );
    }

    #[test]
    fn started_after_both_times_notifies_only_the_later() {
        let now = at("2026-11-19", "13:00");
        assert_eq!(
            pending_slot(now, &defaults(), None),
            Some(at("2026-11-19", "12:55"))
        );
        // Once handled, the 9:00 time is not notified afterwards.
        assert_eq!(
            pending_slot(at("2026-11-19", "18:00"), &defaults(), Some(now)),
            None
        );
    }

    #[test]
    fn a_past_day_is_never_notified() {
        // Handled yesterday at 12:55; the new day starts with nothing to notify.
        let last = Some(at("2026-11-18", "12:55"));
        assert_eq!(
            pending_slot(at("2026-11-19", "00:00"), &defaults(), last),
            None
        );
        assert_eq!(
            pending_slot(at("2026-11-19", "08:59"), &defaults(), last),
            None
        );
        assert_eq!(
            pending_slot(at("2026-11-19", "09:00"), &defaults(), last),
            Some(at("2026-11-19", "09:00"))
        );
        // Off since yesterday morning: yesterday's 12:55 is not made up for.
        let last = Some(at("2026-11-18", "09:00"));
        assert_eq!(
            pending_slot(at("2026-11-19", "07:00"), &defaults(), last),
            None
        );
        // A time at midnight belongs to the new day.
        let midnight = vec![t("00:00")];
        assert_eq!(
            pending_slot(
                at("2026-11-19", "00:00"),
                &midnight,
                Some(at("2026-11-18", "00:00"))
            ),
            Some(at("2026-11-19", "00:00"))
        );
    }

    #[test]
    fn unsorted_and_repeated_times_work() {
        let times = vec![t("12:55"), t("09:00"), t("12:55")];
        assert_eq!(
            pending_slot(at("2026-11-19", "13:00"), &times, None),
            Some(at("2026-11-19", "12:55"))
        );
        assert_eq!(
            next_wake(at("2026-11-19", "12:50"), &times),
            Duration::from_secs(5 * 60)
        );
    }

    #[test]
    fn the_wait_is_capped_and_crosses_midnight() {
        // 9:01 -> 12:55 is far away: wake in 10 minutes to read the clock again.
        assert_eq!(next_wake(at("2026-11-19", "09:01"), &defaults()), MAX_WAIT);
        // After the last time of the day, the next is tomorrow's first.
        assert_eq!(
            next_wake(at("2026-12-31", "23:58"), &[t("00:03")]),
            Duration::from_secs(5 * 60)
        );
        assert_eq!(next_wake(at("2026-11-19", "13:00"), &defaults()), MAX_WAIT);
        // Seconds count: 8:59:30 wakes at 9:00:00.
        let now = d("2026-11-19").and_hms_opt(8, 59, 30).unwrap();
        assert_eq!(next_wake(now, &defaults()), Duration::from_secs(30));
        assert_eq!(next_wake(now, &[]), MAX_WAIT);
    }

    #[test]
    fn the_body_lists_only_categories_with_todos() {
        let none = UrgencyCounts::default();
        assert_eq!(message(&none), None);

        let counts = UrgencyCounts {
            overdue: 1,
            due_today: 2,
            ..UrgencyCounts::default()
        };
        assert_eq!(
            message(&counts),
            Some(Message {
                title: "日限注意が3件あります".into(),
                body: "超過1・本日2".into(),
            })
        );

        let all = UrgencyCounts {
            overdue: 1,
            due_today: 1,
            due_soon: 2,
            follow_up: 1,
            late_start: 3,
        };
        assert_eq!(
            message(&all).unwrap().body,
            "超過1・本日1・間近2・要確認1・着手遅れ3"
        );
        let late = UrgencyCounts {
            late_start: 1,
            ..UrgencyCounts::default()
        };
        assert_eq!(
            message(&late).unwrap(),
            Message {
                title: "日限注意が1件あります".into(),
                body: "着手遅れ1".into(),
            }
        );
    }

    #[test]
    fn fixed_texts_follow_the_design() {
        assert_eq!(tray_hint().title, "Higiri はトレイで動いています");
        assert_eq!(
            tray_hint().body,
            "終了するときは、トレイのメニューから「終了」を選んでください。"
        );
        assert!(test_message().body.ends_with('。'));
    }
}
