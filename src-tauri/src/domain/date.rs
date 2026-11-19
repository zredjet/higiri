//! Date helpers. Dates are calendar days (`NaiveDate`, the PC's local date);
//! timestamps are UTC.

use chrono::{DateTime, Datelike, Days, NaiveDate, SecondsFormat, Utc};

/// Signed number of calendar days from `from` to `to` (`to - from`).
pub fn days_between(from: NaiveDate, to: NaiveDate) -> i64 {
    (to - from).num_days()
}

/// `date + days`, saturating at the maximum representable date.
pub fn add_days(date: NaiveDate, days: u32) -> NaiveDate {
    date.checked_add_days(Days::new(u64::from(days)))
        .unwrap_or(NaiveDate::MAX)
}

/// Storage format for dates: `YYYY-MM-DD`.
pub fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Parses a `YYYY-MM-DD` date (exactly 10 characters, zero-padded).
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    if s.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// Short date as the mocks write it in running text: 「11/20」.
pub fn month_day_slash(date: NaiveDate) -> String {
    format!("{}/{}", date.month(), date.day())
}

/// Date inside a Japanese sentence: 「11月20日」.
pub fn month_day_ja(date: NaiveDate) -> String {
    format!("{}月{}日", date.month(), date.day())
}

/// Storage format for timestamps: UTC ISO 8601 with milliseconds and a `Z`
/// suffix (fixed width, so string order equals time order).
pub fn format_timestamp(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Parses a stored timestamp (any RFC 3339 form) into UTC.
pub fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn d(s: &str) -> NaiveDate {
        parse_date(s).unwrap()
    }

    #[test]
    fn days_between_counts_calendar_days_across_month_and_year() {
        assert_eq!(days_between(d("2026-11-19"), d("2026-11-19")), 0);
        assert_eq!(days_between(d("2026-11-19"), d("2026-11-20")), 1);
        assert_eq!(days_between(d("2026-11-20"), d("2026-11-19")), -1);
        assert_eq!(days_between(d("2026-11-30"), d("2026-12-01")), 1);
        assert_eq!(days_between(d("2026-12-31"), d("2027-01-01")), 1);
        assert_eq!(days_between(d("2028-02-28"), d("2028-03-01")), 2); // leap year
    }

    #[test]
    fn add_days_crosses_month_and_year() {
        assert_eq!(add_days(d("2026-11-29"), 3), d("2026-12-02"));
        assert_eq!(add_days(d("2026-12-30"), 3), d("2027-01-02"));
        assert_eq!(add_days(NaiveDate::MAX, 1), NaiveDate::MAX);
    }

    #[test]
    fn date_round_trip() {
        assert_eq!(format_date(d("2026-01-05")), "2026-01-05");
        assert_eq!(parse_date("2026-1-5"), None);
        assert_eq!(parse_date("2026-02-30"), None);
        assert_eq!(parse_date(""), None);
    }

    #[test]
    fn month_day_ja_has_no_padding() {
        assert_eq!(month_day_ja(d("2026-11-20")), "11月20日");
        assert_eq!(month_day_ja(d("2027-01-05")), "1月5日");
    }

    #[test]
    fn timestamp_is_fixed_width_utc() {
        let ts = Utc.with_ymd_and_hms(2026, 11, 19, 0, 5, 9).unwrap();
        assert_eq!(format_timestamp(ts), "2026-11-19T00:05:09.000Z");
        assert_eq!(parse_timestamp("2026-11-19T00:05:09.000Z"), Some(ts));
        assert_eq!(parse_timestamp("2026-11-19T09:05:09+09:00"), Some(ts));
        assert_eq!(parse_timestamp("not a time"), None);
    }
}
