//! Core records. Field names follow the DB columns.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

/// TODO status (`todos.status`). The stored value is [`Status::as_str`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// 未着手
    Todo,
    /// 作業中
    Doing,
    /// 待ち
    Waiting,
    /// 保留
    Hold,
    /// 完了
    Done,
    /// 中止
    Cancelled,
}

impl Status {
    pub const ALL: [Status; 6] = [
        Status::Todo,
        Status::Doing,
        Status::Waiting,
        Status::Hold,
        Status::Done,
        Status::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::Doing => "doing",
            Status::Waiting => "waiting",
            Status::Hold => "hold",
            Status::Done => "done",
            Status::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        Status::ALL.into_iter().find(|st| st.as_str() == s)
    }

    /// Display name used in UI text and reports.
    pub fn label(self) -> &'static str {
        match self {
            Status::Todo => "未着手",
            Status::Doing => "作業中",
            Status::Waiting => "待ち",
            Status::Hold => "保留",
            Status::Done => "完了",
            Status::Cancelled => "中止",
        }
    }

    /// 完了 and 中止 are closed; the other four are open.
    pub fn is_closed(self) -> bool {
        matches!(self, Status::Done | Status::Cancelled)
    }

    pub fn is_open(self) -> bool {
        !self.is_closed()
    }
}

/// Priority (`todos.priority`: 1 = 高, 2 = 中, 3 = 低).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    High,
    #[default]
    Medium,
    Low,
}

impl Priority {
    pub fn as_i64(self) -> i64 {
        match self {
            Priority::High => 1,
            Priority::Medium => 2,
            Priority::Low => 3,
        }
    }

    pub fn from_i64(v: i64) -> Option<Priority> {
        match v {
            1 => Some(Priority::High),
            2 => Some(Priority::Medium),
            3 => Some(Priority::Low),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Priority::High => "高",
            Priority::Medium => "中",
            Priority::Low => "低",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub description: String,
    pub sort_order: i64,
    /// `None` = active.
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    pub id: i64,
    pub project_id: i64,
    /// Reserved for sub-TODOs (future). Always `None` in the MVP.
    pub parent_id: Option<i64>,
    pub title: String,
    pub status: Status,
    pub priority: Priority,
    /// 日限
    pub due_date: Option<NaiveDate>,
    pub planned_start: Option<NaiveDate>,
    pub planned_end: Option<NaiveDate>,
    pub actual_start: Option<NaiveDate>,
    pub actual_end: Option<NaiveDate>,
    /// 待ち: who / what we are waiting on.
    pub waiting_on: Option<String>,
    /// 待ち: the day waiting started (set automatically).
    pub waiting_since: Option<NaiveDate>,
    /// 待ち: next day to check (確認日).
    pub follow_up_date: Option<NaiveDate>,
    /// When the TODO was completed or cancelled.
    pub closed_at: Option<DateTime<Utc>>,
    /// 中止の理由
    pub close_reason: Option<String>,
    pub memo_md: String,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkLog {
    pub id: i64,
    pub todo_id: i64,
    pub log_date: NaiveDate,
    pub body_md: String,
    pub minutes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_through_its_stored_name() {
        for st in Status::ALL {
            assert_eq!(Status::parse(st.as_str()), Some(st));
        }
        assert_eq!(Status::parse("DONE"), None);
        assert_eq!(Status::parse(""), None);
    }

    #[test]
    fn closed_statuses_are_done_and_cancelled() {
        let closed: Vec<_> = Status::ALL.into_iter().filter(|s| s.is_closed()).collect();
        assert_eq!(closed, vec![Status::Done, Status::Cancelled]);
    }

    #[test]
    fn priority_round_trips_and_orders_high_first() {
        for p in [Priority::High, Priority::Medium, Priority::Low] {
            assert_eq!(Priority::from_i64(p.as_i64()), Some(p));
        }
        assert_eq!(Priority::from_i64(0), None);
        assert!(Priority::High < Priority::Medium && Priority::Medium < Priority::Low);
        assert_eq!(Priority::default(), Priority::Medium);
    }
}
