//! Pure functions (urgency classification, status transitions, report assembly).
//!
//! Nothing in here reads the clock or touches the database: `today` (local date)
//! and `now` (UTC timestamp) are always passed in by the caller (`commands/`).

pub mod date;
pub mod events;
pub mod listing;
pub mod model;
pub mod settings;
pub mod status;
pub mod timeline;
pub mod urgency;
pub mod views;

#[cfg(test)]
pub(crate) mod testing;
