//! The only place that reads the clock.

use chrono::{DateTime, Local, NaiveDate, Utc};

/// Today on this PC (local date).
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The current time in UTC.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}
