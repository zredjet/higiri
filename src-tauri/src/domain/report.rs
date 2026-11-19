//! 日報 (R-1, R-2). Pure.
//!
//! The text follows the design's example (docs/requirements-design.md
//! 「日報の生成」) to the character. 本日の作業 and 本日完了・中止 come from that
//! day's records; 待ち, 明日の予定 and 日限注意 judge the TODOs' current state
//! with the target date as "today" (decided for past dates).
//!
//! In 営業日 (v1.1), 明日の予定 is for the next business day, the day counts of
//! 日限注意 read 「あとN営業日」「N営業日超過」, and a due date moved off a
//! holiday gets 「休日のため前倒し」. In 暦日 the text is the MVP's.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use super::date::{format_date, month_day_slash};
use super::events::{EventKind, TodoEvent};
use super::model::{Project, Status, Todo, WorkLog};
use super::urgency::{Assessment, Rules, Urgency, assess, attention_order, waiting_day};

/// Report kinds (`reports.kind`). The MVP makes daily reports only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Daily,
    Weekly,
    Monthly,
}

impl ReportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ReportKind::Daily => "daily",
            ReportKind::Weekly => "weekly",
            ReportKind::Monthly => "monthly",
        }
    }

    pub fn parse(s: &str) -> Option<ReportKind> {
        [ReportKind::Daily, ReportKind::Weekly, ReportKind::Monthly]
            .into_iter()
            .find(|k| k.as_str() == s)
    }
}

/// A saved report (`reports` row). `body_md` is the full text as saved;
/// `note_md` is its 所感・連絡事項 part, kept apart so regenerating keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Report {
    pub kind: ReportKind,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub body_md: String,
    pub note_md: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Heading of the hand-written part; everything after it is kept on regenerate.
pub const NOTE_HEADING: &str = "## 所感・連絡事項";

const NONE_LINE: &str = "- なし";

/// Added to a due date that falls on a holiday, in 営業日.
const SHIFTED: &str = "休日のため前倒し";

const WEEKDAYS: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];

/// 「# 日報 2026-11-19(木)」
pub fn title(date: NaiveDate) -> String {
    let wd = WEEKDAYS[date.weekday().num_days_from_monday() as usize];
    format!("# 日報 {}({wd})", format_date(date))
}

/// 「1.5h」 (minutes to hours, one decimal).
pub fn hours(minutes: i64) -> String {
    format!("{:.1}h", minutes as f64 / 60.0)
}

/// What a report is made from.
pub struct ReportInput<'a> {
    pub date: NaiveDate,
    pub rules: &'a Rules,
    /// All projects (archived included), in display order.
    pub projects: &'a [Project],
    /// All TODOs, in display order.
    pub todos: &'a [Todo],
    /// Work logs written for `date`.
    pub logs: &'a [WorkLog],
    /// Changes recorded on `date`.
    pub events: &'a [TodoEvent],
}

struct Lookup<'a> {
    projects: HashMap<i64, &'a Project>,
    todos: HashMap<i64, &'a Todo>,
    project_order: HashMap<i64, usize>,
}

impl<'a> Lookup<'a> {
    fn new(input: &ReportInput<'a>) -> Self {
        Lookup {
            projects: input.projects.iter().map(|p| (p.id, p)).collect(),
            todos: input.todos.iter().map(|t| (t.id, t)).collect(),
            project_order: input
                .projects
                .iter()
                .enumerate()
                .map(|(i, p)| (p.id, i))
                .collect(),
        }
    }

    fn project_name(&self, t: &Todo) -> &str {
        self.projects
            .get(&t.project_id)
            .map(|p| p.name.as_str())
            .unwrap_or("")
    }

    fn is_active(&self, t: &Todo) -> bool {
        self.projects
            .get(&t.project_id)
            .is_some_and(|p| p.archived_at.is_none())
    }
}

fn section(out: &mut String, heading: &str, lines: Vec<String>) {
    out.push('\n');
    out.push_str(heading);
    out.push('\n');
    if lines.is_empty() {
        out.push_str(NONE_LINE);
        out.push('\n');
    }
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
}

/// 本日の作業: logs grouped by project, then by TODO (priority, then order).
fn work_section(input: &ReportInput, lk: &Lookup, out: &mut String) {
    out.push_str("\n## 本日の作業\n");
    // TODO id -> (minutes, bodies)
    let mut per_todo: HashMap<i64, (i64, Vec<&str>)> = HashMap::new();
    let mut logs: Vec<&WorkLog> = input
        .logs
        .iter()
        .filter(|l| l.log_date == input.date)
        .collect();
    logs.sort_by_key(|l| (l.created_at, l.id));
    for l in logs {
        let e = per_todo.entry(l.todo_id).or_default();
        e.0 += l.minutes.unwrap_or(0);
        e.1.push(l.body_md.as_str());
    }
    let mut todos: Vec<&Todo> = per_todo
        .keys()
        .filter_map(|id| lk.todos.get(id).copied())
        .collect();
    if todos.is_empty() {
        out.push('\n');
        out.push_str(NONE_LINE);
        out.push('\n');
        return;
    }
    todos.sort_by_key(|t| {
        (
            lk.project_order
                .get(&t.project_id)
                .copied()
                .unwrap_or(usize::MAX),
            t.priority,
            t.sort_order,
            t.id,
        )
    });
    let mut current_project: Option<i64> = None;
    for t in todos {
        if current_project != Some(t.project_id) {
            current_project = Some(t.project_id);
            out.push_str(&format!("\n### {}\n", lk.project_name(t)));
        }
        let (minutes, bodies) = &per_todo[&t.id];
        let mut meta = t.status.label().to_owned();
        if t.status.is_open()
            && let Some(due) = t.due_date
        {
            meta.push_str(&format!(" / 日限 {}", month_day_slash(due)));
        }
        let time = if *minutes > 0 {
            hours(*minutes)
        } else {
            String::new()
        };
        out.push_str(&format!("- **{}**({meta}){time}\n", t.title));
        for body in bodies {
            for line in body.trim().lines() {
                if line.trim().is_empty() {
                    out.push('\n');
                } else {
                    out.push_str("  ");
                    out.push_str(line.trim_end());
                    out.push('\n');
                }
            }
        }
    }
}

/// 本日完了・中止: TODOs that became 完了/中止 on the day and still are; 完了 first.
fn closed_section(input: &ReportInput, lk: &Lookup) -> Vec<String> {
    let mut found: Vec<(u8, &TodoEvent, &Todo)> = Vec::new();
    for e in input
        .events
        .iter()
        .filter(|e| e.kind == EventKind::Status && e.event_date == input.date)
    {
        let Some(t) = lk.todos.get(&e.todo_id) else {
            continue;
        };
        let to = e.new_value.as_deref().and_then(Status::parse);
        let rank = match (to, t.status) {
            (Some(Status::Done), Status::Done) => 0,
            (Some(Status::Cancelled), Status::Cancelled) => 1,
            _ => continue,
        };
        if found.iter().any(|(_, _, f)| f.id == t.id) {
            continue;
        }
        found.push((rank, e, t));
    }
    found.sort_by_key(|(rank, e, _)| (*rank, e.occurred_at, e.id));
    found
        .into_iter()
        .map(|(_, _, t)| {
            let reason = match (&t.status, &t.close_reason) {
                (Status::Cancelled, Some(r)) => format!("(理由:{r})"),
                _ => String::new(),
            };
            format!(
                "- 【{}】{} / {}{reason}",
                t.status.label(),
                lk.project_name(t),
                t.title
            )
        })
        .collect()
}

/// 待ち: waiting TODOs of active projects, next follow-up first.
fn waiting_section(input: &ReportInput, lk: &Lookup) -> Vec<String> {
    let mut waiting: Vec<&Todo> = input
        .todos
        .iter()
        .filter(|t| t.status == Status::Waiting && lk.is_active(t))
        // On a past day, a TODO that started waiting later was not waiting yet.
        .filter(|t| t.waiting_since.is_none_or(|since| since <= input.date))
        .collect();
    waiting.sort_by_key(|t| {
        (
            t.follow_up_date.is_none(),
            t.follow_up_date,
            t.waiting_since,
            t.id,
        )
    });
    waiting
        .into_iter()
        .map(|t| {
            let mut parts = vec![format!("{}待ち", t.waiting_on.as_deref().unwrap_or(""))];
            if let Some(since) = t.waiting_since {
                parts.push(format!("{}日目", waiting_day(since, input.date)));
            }
            if let Some(f) = t.follow_up_date {
                parts.push(format!("確認日 {}", month_day_slash(f)));
            }
            format!(
                "- {} / {}({})",
                lk.project_name(t),
                t.title,
                parts.join("、")
            )
        })
        .collect()
}

/// 明日の予定: 作業中, and 未着手 starting by tomorrow (the next business day in
/// 営業日). Active projects, display order.
fn tomorrow_section(input: &ReportInput, lk: &Lookup) -> Vec<String> {
    let tomorrow = input.rules.next_day(input.date);
    input
        .todos
        .iter()
        .filter(|t| lk.is_active(t))
        .filter_map(|t| match t.status {
            Status::Doing => Some(format!("- {} / {}", lk.project_name(t), t.title)),
            Status::Todo if t.planned_start.is_some_and(|s| s <= tomorrow) => {
                let start = t.planned_start.map(month_day_slash).unwrap_or_default();
                let end = t.planned_end.map(month_day_slash).unwrap_or_default();
                Some(format!(
                    "- {} / {}(予定 {start}〜{end})",
                    lk.project_name(t),
                    t.title
                ))
            }
            _ => None,
        })
        .collect()
}

/// 日限注意: 超過・本日・間近 with the target date as today.
fn attention_section(input: &ReportInput, lk: &Lookup) -> Vec<String> {
    let mut items: Vec<(&Todo, Urgency, Assessment)> = input
        .todos
        .iter()
        .filter(|t| lk.is_active(t))
        .filter_map(|t| {
            let a = assess(t, input.date, input.rules);
            match a.urgency {
                Some(u @ (Urgency::Overdue | Urgency::DueToday | Urgency::DueSoon)) => {
                    Some((t, u, a))
                }
                _ => None,
            }
        })
        .collect();
    items.sort_by(|a, b| attention_order((a.0, a.1), (b.0, b.1)));
    items
        .into_iter()
        .map(|(t, u, a)| {
            let mut parts = vec![format!(
                "日限 {}",
                t.due_date.map(month_day_slash).unwrap_or_default()
            )];
            if a.due_shifted {
                parts.push(SHIFTED.to_owned());
            }
            let days = a.days_until_due.unwrap_or(0);
            let unit = if a.business_days { "営業日" } else { "日" };
            match u {
                Urgency::Overdue => parts.push(format!("{}{unit}超過", -days)),
                Urgency::DueSoon => parts.push(format!("あと{days}{unit}")),
                _ => {}
            }
            if t.status == Status::Waiting {
                parts.push("待ち".to_owned());
            }
            format!(
                "- 【{}】{} / {}({})",
                u.label(),
                lk.project_name(t),
                t.title,
                parts.join("、")
            )
        })
        .collect()
}

/// The generated part: title and the five sections, without 所感・連絡事項.
pub fn generate_body(input: &ReportInput) -> String {
    let lk = Lookup::new(input);
    let mut out = title(input.date);
    out.push('\n');
    work_section(input, &lk, &mut out);
    section(&mut out, "## 本日完了・中止", closed_section(input, &lk));
    section(&mut out, "## 待ち", waiting_section(input, &lk));
    section(&mut out, "## 明日の予定", tomorrow_section(input, &lk));
    section(&mut out, "## 日限注意", attention_section(input, &lk));
    out
}

/// Generated part + the hand-written 所感・連絡事項.
pub fn compose(body: &str, note: &str) -> String {
    let mut out = body.trim_end().to_owned();
    out.push_str("\n\n");
    out.push_str(NOTE_HEADING);
    out.push('\n');
    let note = note.trim();
    if !note.is_empty() {
        out.push_str(note);
        out.push('\n');
    }
    out
}

/// The hand-written part of a full report: what follows the 所感・連絡事項
/// heading (empty when the heading is missing).
pub fn split_note(full: &str) -> String {
    let mut lines = full.lines();
    for line in lines.by_ref() {
        if line.trim_end() == NOTE_HEADING {
            let rest: Vec<&str> = lines.collect();
            return rest.join("\n").trim().to_owned();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{business_rules, d, rules, todo, ts};

    #[test]
    fn title_has_the_weekday() {
        assert_eq!(title(d("2026-11-19")), "# 日報 2026-11-19(木)");
        assert_eq!(title(d("2026-11-22")), "# 日報 2026-11-22(日)");
    }

    #[test]
    fn hours_have_one_decimal() {
        assert_eq!(hours(90), "1.5h");
        assert_eq!(hours(120), "2.0h");
        assert_eq!(hours(30), "0.5h");
        assert_eq!(hours(45), "0.8h");
    }

    #[test]
    fn note_round_trips_through_compose_and_split() {
        let body = "# 日報 2026-11-19(木)\n\n## 本日の作業\n\n- なし\n";
        let full = compose(body, "  - 明日の午前に確定する。\n");
        assert_eq!(
            full,
            "# 日報 2026-11-19(木)\n\n## 本日の作業\n\n- なし\n\n## 所感・連絡事項\n- 明日の午前に確定する。\n"
        );
        assert_eq!(split_note(&full), "- 明日の午前に確定する。");
        assert_eq!(split_note("# 日報\n本文だけ"), "");
        assert_eq!(compose(body, ""), format!("{}\n{NOTE_HEADING}\n", body));
    }

    #[test]
    fn edits_above_the_note_are_not_part_of_it() {
        let full = "# 日報\n\n## 待ち\n- 手で直した行\n\n## 所感・連絡事項\n- 所感1\n\n- 所感2\n";
        assert_eq!(split_note(full), "- 所感1\n\n- 所感2");
    }

    #[test]
    fn empty_day_writes_none_in_every_section() {
        let p = Project {
            id: 1,
            name: "P".into(),
            color: "#3B6FD4".into(),
            description: String::new(),
            sort_order: 1,
            archived_at: None,
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        };
        let input = ReportInput {
            date: d("2026-11-19"),
            rules: &rules(3),
            projects: &[p],
            todos: &[],
            logs: &[],
            events: &[],
        };
        assert_eq!(
            generate_body(&input),
            "# 日報 2026-11-19(木)\n\n## 本日の作業\n\n- なし\n\n## 本日完了・中止\n- なし\n\n## 待ち\n- なし\n\n## 明日の予定\n- なし\n\n## 日限注意\n- なし\n"
        );
    }

    #[test]
    fn closed_todos_show_no_due_date_and_logs_keep_their_indentation() {
        let p = Project {
            id: 1,
            name: "P".into(),
            color: "#3B6FD4".into(),
            description: String::new(),
            sort_order: 1,
            archived_at: None,
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        };
        let t = Todo {
            id: 7,
            project_id: 1,
            title: "T".into(),
            due_date: Some(d("2026-11-25")),
            ..todo(Status::Done)
        };
        let log = WorkLog {
            id: 1,
            todo_id: 7,
            log_date: d("2026-11-19"),
            body_md: "まとめ\n\n- 箇条書き\n  - 入れ子".into(),
            minutes: None,
            created_at: ts(2026, 11, 19, 1, 0),
            updated_at: ts(2026, 11, 19, 1, 0),
        };
        let input = ReportInput {
            date: d("2026-11-19"),
            rules: &rules(3),
            projects: std::slice::from_ref(&p),
            todos: std::slice::from_ref(&t),
            logs: std::slice::from_ref(&log),
            events: &[],
        };
        let body = generate_body(&input);
        assert!(
            body.contains("### P\n- **T**(完了)\n  まとめ\n\n  - 箇条書き\n    - 入れ子\n"),
            "{body}"
        );
    }

    #[test]
    fn waiting_lists_only_todos_already_waiting_on_that_day() {
        let p = Project {
            id: 1,
            name: "P".into(),
            color: "#3B6FD4".into(),
            description: String::new(),
            sort_order: 1,
            archived_at: None,
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        };
        let t = Todo {
            id: 7,
            project_id: 1,
            title: "T".into(),
            waiting_on: Some("相手の回答".into()),
            waiting_since: Some(d("2026-11-19")),
            follow_up_date: Some(d("2026-11-20")),
            ..todo(Status::Waiting)
        };
        let on = |date: &str| {
            generate_body(&ReportInput {
                date: d(date),
                rules: &rules(3),
                projects: std::slice::from_ref(&p),
                todos: std::slice::from_ref(&t),
                logs: &[],
                events: &[],
            })
        };
        assert!(on("2026-11-18").contains("## 待ち\n- なし\n"));
        assert!(
            on("2026-11-19").contains("## 待ち\n- P / T(相手の回答待ち、1日目、確認日 11/20)\n")
        );
    }

    #[test]
    fn business_days_change_tomorrow_and_the_day_counts() {
        let p = Project {
            id: 1,
            name: "P".into(),
            color: "#3B6FD4".into(),
            description: String::new(),
            sort_order: 1,
            archived_at: None,
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        };
        let mk = |id, title: &str, status, due: &str, planned: Option<(&str, &str)>| Todo {
            id,
            project_id: 1,
            title: title.into(),
            due_date: Some(d(due)),
            planned_start: planned.map(|p| d(p.0)),
            planned_end: planned.map(|p| d(p.1)),
            ..todo(status)
        };
        let todos = [
            mk(1, "A", Status::Doing, "2026-11-22", None), // Sunday
            mk(
                2,
                "B",
                Status::Todo,
                "2026-11-24",
                Some(("2026-11-24", "2026-11-25")),
            ),
            mk(3, "C", Status::Todo, "2026-11-18", None),
            mk(
                4,
                "D",
                Status::Todo,
                "2026-12-10",
                Some(("2026-11-23", "2026-11-23")),
            ),
        ];
        // Friday 11/20; 11/21–11/23 are off, so the next business day is 11/24.
        let body = |r: &Rules| {
            generate_body(&ReportInput {
                date: d("2026-11-20"),
                rules: r,
                projects: std::slice::from_ref(&p),
                todos: &todos,
                logs: &[],
                events: &[],
            })
        };
        let business = body(&business_rules(3, vec![]));
        assert!(
            business.contains(
                "## 明日の予定\n- P / A\n- P / B(予定 11/24〜11/25)\n- P / D(予定 11/23〜11/23)\n"
            ),
            "{business}"
        );
        assert!(
            business.contains(concat!(
                "## 日限注意\n",
                "- 【超過】P / C(日限 11/18、2営業日超過)\n",
                "- 【本日】P / A(日限 11/22、休日のため前倒し)\n",
                "- 【間近】P / B(日限 11/24、あと1営業日)\n",
            )),
            "{business}"
        );

        let calendar = body(&rules(3));
        assert!(
            calendar.contains("## 明日の予定\n- P / A\n\n"),
            "{calendar}"
        );
        assert!(
            calendar.contains(concat!(
                "## 日限注意\n",
                "- 【超過】P / C(日限 11/18、2日超過)\n",
                "- 【間近】P / A(日限 11/22、あと2日)\n",
            )),
            "{calendar}"
        );
    }

    #[test]
    fn kind_round_trips() {
        for k in [ReportKind::Daily, ReportKind::Weekly, ReportKind::Monthly] {
            assert_eq!(ReportKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(ReportKind::parse("yearly"), None);
    }
}
