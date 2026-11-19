//! Test helpers shared by domain and repo tests.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use super::date::parse_date;
use super::model::{Priority, Status, Todo};

/// `"YYYY-MM-DD"` -> `NaiveDate` (panics on bad input; tests only).
pub fn d(s: &str) -> NaiveDate {
    parse_date(s).unwrap_or_else(|| panic!("bad date in test: {s}"))
}

/// A fixed UTC timestamp for tests.
pub fn ts(y: i32, m: u32, day: u32, h: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, day, h, min, 0).unwrap()
}

/// A minimal open TODO with no dates set.
pub fn todo(status: Status) -> Todo {
    let created = ts(2026, 11, 1, 0, 0);
    Todo {
        id: 1,
        project_id: 1,
        parent_id: None,
        title: "テスト".into(),
        status,
        priority: Priority::Medium,
        due_date: None,
        planned_start: None,
        planned_end: None,
        actual_start: None,
        actual_end: None,
        waiting_on: None,
        waiting_since: None,
        follow_up_date: None,
        closed_at: None,
        close_reason: None,
        memo_md: String::new(),
        sort_order: 0,
        created_at: created,
        updated_at: created,
    }
}
