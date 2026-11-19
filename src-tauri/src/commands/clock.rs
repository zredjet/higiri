//! The only place that reads the clock (used by the commands and by the
//! resident timer, `notifier`).

use chrono::{DateTime, Local, NaiveDate, Utc};

/// Today on this PC (local date).
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The current time in UTC.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// The PC's time zone, for turning a UTC time into the local wall-clock time
/// (the notification times are local). It reads no clock itself.
pub fn zone() -> Local {
    Local
}
