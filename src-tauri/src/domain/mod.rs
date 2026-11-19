//! Pure functions (urgency classification, status transitions, report assembly).
//!
//! Nothing in here reads the clock or touches the database: `today` (local date)
//! and `now` (UTC timestamp) are always passed in by the caller (`commands/`).

pub mod date;
pub mod events;
pub mod model;
pub mod settings;
pub mod status;
pub mod urgency;

#[cfg(test)]
pub(crate) mod testing;
