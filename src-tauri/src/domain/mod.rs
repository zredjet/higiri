//! Pure functions (urgency classification, status transitions, report assembly).
//!
//! Nothing in here reads the clock or touches the database: `today` (local date)
//! and `now` (UTC timestamp) are always passed in by the caller (`commands/`).

pub mod calendar;
pub mod dashboard;
pub mod date;
pub mod events;
pub mod gantt;
pub mod holidays;
pub mod listing;
pub mod model;
pub mod notify;
pub mod repeat;
pub mod report;
pub mod report_template;
pub mod search;
pub mod settings;
pub mod status;
pub mod subtodo;
pub mod template;
pub mod timeline;
pub mod urgency;
pub mod views;

#[cfg(test)]
pub(crate) mod testing;
