//! 日報 (R-1, R-2, R-5). Pure.
//!
//! With the default template the text follows the design's example
//! (docs/requirements-design.md 「日報の生成」) to the character. 本日の作業 and
//! 本日完了・中止 come from that day's records; 待ち, 明日の予定 and 日限注意
//! judge the TODOs' current state with the target date as "today" (decided
//! for past dates).
//!
//! The template (v1.1, `report_template`) sets the title, the order of the
//! sections, their headings, which are shown and what their lines carry. The
//! line format stays the MVP's. 所感・連絡事項 is always last.
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
use super::report_template::{
    AttentionOptions, ClosedOptions, ReportTemplate, SectionBody, SectionKind, TomorrowOptions,
    WaitingOptions, WorkOptions,
};
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

const NONE_LINE: &str = "- なし";

/// Added to a due date that falls on a holiday, in 営業日.
const SHIFTED: &str = "休日のため前倒し";

const WEEKDAYS: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];

/// 「# 日報 2026-11-19(木)」 (with the default template).
pub fn title(template: &ReportTemplate, date: NaiveDate) -> String {
    let wd = WEEKDAYS[date.weekday().num_days_from_monday() as usize];
    format!("# {}", template.fill_title(&format_date(date), wd))
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

/// 「(a、b)」, or nothing when there are no parts.
fn parenthesized(parts: &[String]) -> String {
    if parts.is_empty() {
        String::new()
    } else {
        format!("({})", parts.join("、"))
    }
}

fn section(out: &mut String, heading: &str, lines: Vec<String>) {
    out.push_str("\n## ");
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

/// 「- **API設計書のレビュー対応**(待ち / 日限 11/20)1.5h」, with
/// 「プロジェクト名 / 」 in front when there are no project headings.
fn work_line(t: &Todo, minutes: i64, o: &WorkOptions, lk: &Lookup) -> String {
    let mut meta: Vec<String> = Vec::new();
    if o.status {
        meta.push(t.status.label().to_owned());
    }
    if o.due_date
        && t.status.is_open()
        && let Some(due) = t.due_date
    {
        meta.push(format!("日限 {}", month_day_slash(due)));
    }
    let meta = if meta.is_empty() {
        String::new()
    } else {
        format!("({})", meta.join(" / "))
    };
    let time = if o.work_time && minutes > 0 {
        hours(minutes)
    } else {
        String::new()
    };
    // Without the parentheses, a space keeps the time off the bold title.
    let gap = if meta.is_empty() && !time.is_empty() {
        " "
    } else {
        ""
    };
    let project = if o.project_headings {
        String::new()
    } else {
        format!("{} / ", lk.project_name(t))
    };
    format!("- {project}**{}**{meta}{gap}{time}", t.title)
}

/// 本日の作業: logs grouped by project (or not, without project headings),
/// then by TODO (priority, then order).
fn work_section(
    input: &ReportInput,
    lk: &Lookup,
    heading: &str,
    o: &WorkOptions,
    out: &mut String,
) {
    out.push_str("\n## ");
    out.push_str(heading);
    out.push('\n');
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
        // The MVP's text has a blank line here, as before a project heading.
        if o.project_headings {
            out.push('\n');
        }
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
        if o.project_headings && current_project != Some(t.project_id) {
            current_project = Some(t.project_id);
            out.push_str(&format!("\n### {}\n", lk.project_name(t)));
        }
        let (minutes, bodies) = &per_todo[&t.id];
        out.push_str(&work_line(t, *minutes, o, lk));
        out.push('\n');
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
fn closed_section(input: &ReportInput, lk: &Lookup, o: &ClosedOptions) -> Vec<String> {
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
                (Status::Cancelled, Some(r)) if o.close_reason => format!("(理由:{r})"),
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
fn waiting_section(input: &ReportInput, lk: &Lookup, o: &WaitingOptions) -> Vec<String> {
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
            let mut parts: Vec<String> = Vec::new();
            if o.waiting_on {
                parts.push(format!("{}待ち", t.waiting_on.as_deref().unwrap_or("")));
            }
            if o.waiting_days
                && let Some(since) = t.waiting_since
            {
                parts.push(format!("{}日目", waiting_day(since, input.date)));
            }
            if o.follow_up_date
                && let Some(f) = t.follow_up_date
            {
                parts.push(format!("確認日 {}", month_day_slash(f)));
            }
            format!(
                "- {} / {}{}",
                lk.project_name(t),
                t.title,
                parenthesized(&parts)
            )
        })
        .collect()
}

/// 明日の予定: 作業中, and 未着手 starting by tomorrow (the next business day in
/// 営業日). Active projects, display order.
fn tomorrow_section(input: &ReportInput, lk: &Lookup, o: &TomorrowOptions) -> Vec<String> {
    let tomorrow = input.rules.next_day(input.date);
    input
        .todos
        .iter()
        .filter(|t| lk.is_active(t))
        .filter_map(|t| match t.status {
            Status::Doing => Some(format!("- {} / {}", lk.project_name(t), t.title)),
            Status::Todo if t.planned_start.is_some_and(|s| s <= tomorrow) => {
                let period = if o.planned_period {
                    let start = t.planned_start.map(month_day_slash).unwrap_or_default();
                    let end = t.planned_end.map(month_day_slash).unwrap_or_default();
                    format!("(予定 {start}〜{end})")
                } else {
                    String::new()
                };
                Some(format!("- {} / {}{period}", lk.project_name(t), t.title))
            }
            _ => None,
        })
        .collect()
}

/// The parentheses of a 日限注意 line.
fn attention_parts(t: &Todo, u: Urgency, a: &Assessment, o: &AttentionOptions) -> Vec<String> {
    let unit = if a.business_days { "営業日" } else { "日" };
    let mut parts = Vec::new();
    match u {
        Urgency::Overdue | Urgency::DueToday | Urgency::DueSoon => {
            parts.push(format!(
                "日限 {}",
                t.due_date.map(month_day_slash).unwrap_or_default()
            ));
            if a.due_shifted {
                parts.push(SHIFTED.to_owned());
            }
            let days = a.days_until_due.unwrap_or(0);
            match u {
                Urgency::Overdue if o.days => parts.push(format!("{}{unit}超過", -days)),
                Urgency::DueSoon if o.days => parts.push(format!("あと{days}{unit}")),
                _ => {}
            }
            if o.waiting_mark && t.status == Status::Waiting {
                parts.push("待ち".to_owned());
            }
        }
        // As in the startup reminder: 「確認日 11/18、待ち3日目」.
        Urgency::FollowUp => {
            parts.push(format!(
                "確認日 {}",
                t.follow_up_date.map(month_day_slash).unwrap_or_default()
            ));
            if o.days
                && let Some(day) = a.waiting_day
            {
                parts.push(format!("待ち{day}日目"));
            }
        }
        // 「予定開始 11/17、2日経過」.
        Urgency::LateStart => {
            parts.push(format!(
                "予定開始 {}",
                t.planned_start.map(month_day_slash).unwrap_or_default()
            ));
            if o.days
                && let Some(days) = a.days_since_planned_start
            {
                parts.push(format!("{days}{unit}経過"));
            }
        }
    }
    parts
}

/// 日限注意: 超過・本日・間近 (and 要確認・着手遅れ when chosen) with the target
/// date as today.
fn attention_section(input: &ReportInput, lk: &Lookup, o: &AttentionOptions) -> Vec<String> {
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
                Some(u @ (Urgency::FollowUp | Urgency::LateStart))
                    if o.follow_up_and_late_start =>
                {
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
            format!(
                "- 【{}】{} / {}{}",
                u.label(),
                lk.project_name(t),
                t.title,
                parenthesized(&attention_parts(t, u, &a, o))
            )
        })
        .collect()
}

/// The generated part: the title and the shown sections in the template's
/// order, without 所感・連絡事項 (added last by [`compose`]).
pub fn generate_body(input: &ReportInput, template: &ReportTemplate) -> String {
    let lk = Lookup::new(input);
    let mut out = title(template, input.date);
    out.push('\n');
    for s in template.sections.iter().filter(|s| s.enabled) {
        let heading = s.heading.as_str();
        match &s.body {
            SectionBody::Work(o) => work_section(input, &lk, heading, o, &mut out),
            SectionBody::Closed(o) => section(&mut out, heading, closed_section(input, &lk, o)),
            SectionBody::Waiting(o) => section(&mut out, heading, waiting_section(input, &lk, o)),
            SectionBody::Tomorrow(o) => section(&mut out, heading, tomorrow_section(input, &lk, o)),
            SectionBody::Attention(o) => {
                section(&mut out, heading, attention_section(input, &lk, o))
            }
            SectionBody::Note(_) => {}
        }
    }
    out
}

/// Generated part + the hand-written 所感・連絡事項 under the template's heading.
pub fn compose(body: &str, note: &str, template: &ReportTemplate) -> String {
    let mut out = body.trim_end().to_owned();
    out.push_str("\n\n## ");
    out.push_str(template.note_heading());
    out.push('\n');
    let note = note.trim();
    if !note.is_empty() {
        out.push_str(note);
        out.push('\n');
    }
    out
}

/// The text of a `## ` heading line (level 2, from the first column).
fn h2(line: &str) -> Option<&str> {
    line.trim_end().strip_prefix("## ").map(str::trim)
}

/// The line of the 所感・連絡事項 heading in a full report.
///
/// 1. The first line with the template's heading or the default one
///    (「所感・連絡事項」), whichever comes first. A heading that a shown
///    section now uses is not looked for, so that the generated part never
///    ends up in the note.
/// 2. Otherwise (the heading was changed more than once since the report was
///    made), the first `## ` line that is not a section's heading, in the
///    template or by default.
///
/// When unsure, more is kept: an older report whose section headings were
/// changed may keep part of its generated text in the note, but no
/// hand-written text is lost.
fn note_line(lines: &[&str], template: &ReportTemplate) -> Option<usize> {
    let sections = template
        .sections
        .iter()
        .filter(|s| s.kind() != SectionKind::Note);
    let shown: Vec<&str> = sections
        .clone()
        .filter(|s| s.enabled)
        .map(|s| s.heading.as_str())
        .collect();
    let notes: Vec<&str> = [template.note_heading(), SectionKind::Note.default_heading()]
        .into_iter()
        .filter(|h| !shown.contains(h))
        .collect();
    let mut known: Vec<&str> = sections.map(|s| s.heading.as_str()).collect();
    for kind in SectionKind::ALL {
        if kind != SectionKind::Note {
            known.push(kind.default_heading());
        }
    }
    let headings = || {
        lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| h2(l).map(|h| (i, h)))
    };
    headings()
        .find(|(_, h)| notes.contains(h))
        .or_else(|| headings().find(|(_, h)| !known.contains(h)))
        .map(|(i, _)| i)
}

/// The hand-written part of a full report: what follows the 所感・連絡事項
/// heading (see [`note_line`]); empty when there is none.
pub fn split_note(full: &str, template: &ReportTemplate) -> String {
    let lines: Vec<&str> = full.lines().collect();
    match note_line(&lines, template) {
        Some(i) => lines[i + 1..].join("\n").trim().to_owned(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{business_rules, d, rules, todo, ts};

    fn default_template() -> ReportTemplate {
        ReportTemplate::default()
    }

    #[test]
    fn title_has_the_weekday() {
        let t = default_template();
        assert_eq!(title(&t, d("2026-11-19")), "# 日報 2026-11-19(木)");
        assert_eq!(title(&t, d("2026-11-22")), "# 日報 2026-11-22(日)");
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
        let t = default_template();
        let body = "# 日報 2026-11-19(木)\n\n## 本日の作業\n\n- なし\n";
        let full = compose(body, "  - 明日の午前に確定する。\n", &t);
        assert_eq!(
            full,
            "# 日報 2026-11-19(木)\n\n## 本日の作業\n\n- なし\n\n## 所感・連絡事項\n- 明日の午前に確定する。\n"
        );
        assert_eq!(split_note(&full, &t), "- 明日の午前に確定する。");
        assert_eq!(split_note("# 日報\n本文だけ", &t), "");
        assert_eq!(
            compose(body, "", &t),
            format!("{}\n## 所感・連絡事項\n", body)
        );
    }

    #[test]
    fn edits_above_the_note_are_not_part_of_it() {
        let full = "# 日報\n\n## 待ち\n- 手で直した行\n\n## 所感・連絡事項\n- 所感1\n\n- 所感2\n";
        assert_eq!(split_note(full, &default_template()), "- 所感1\n\n- 所感2");
    }

    fn template_with(headings: &[(SectionKind, &str)]) -> ReportTemplate {
        let mut t = default_template();
        for (kind, h) in headings {
            t.sections
                .iter_mut()
                .find(|s| s.kind() == *kind)
                .unwrap()
                .heading = (*h).into();
        }
        t
    }

    #[test]
    fn the_note_starts_at_whichever_heading_comes_first() {
        let t = template_with(&[(SectionKind::Note, "連絡")]);
        // Written under the default heading; 「## 連絡」 is part of the note.
        let full = "# 日報\n\n## 待ち\n- なし\n\n## 所感・連絡事項\n- 所感\n\n## 連絡\n- 連絡\n";
        assert_eq!(split_note(full, &t), "- 所感\n\n## 連絡\n- 連絡");
        // And the other way round.
        let full = "# 日報\n\n## 待ち\n- なし\n\n## 連絡\n- 連絡\n\n## 所感・連絡事項\n- 所感\n";
        assert_eq!(split_note(full, &t), "- 連絡\n\n## 所感・連絡事項\n- 所感");
    }

    #[test]
    fn a_section_that_uses_the_default_note_heading_is_not_the_note() {
        let t = template_with(&[
            (SectionKind::Attention, "所感・連絡事項"),
            (SectionKind::Note, "連絡"),
        ]);
        let body = "# 日報\n\n## 所感・連絡事項\n- なし\n";
        let full = compose(body, "- 連絡を書いた。", &t);
        assert_eq!(split_note(&full, &t), "- 連絡を書いた。");
        // Regenerating again and again does not pile up the generated part.
        let again = compose(body, &split_note(&full, &t), &t);
        assert_eq!(again, full);
    }

    #[test]
    fn an_older_note_heading_is_found_among_the_section_headings() {
        // Made when the note was 「連絡」; now it is 「メモ」.
        let t = template_with(&[
            (SectionKind::Note, "メモ"),
            (SectionKind::Waiting, "待ち状況"),
        ]);
        let full = "# 日報\n\n## 本日の作業\n\n- なし\n\n## 待ち\n- なし\n\n## 連絡\n- 連絡\n\n## 補足\n- 補足\n";
        assert_eq!(split_note(full, &t), "- 連絡\n\n## 補足\n- 補足");
        // A section heading changed since: more is kept, nothing is lost.
        let full = "# 日報\n\n## 待ち合わせ\n- なし\n\n## 明日の予定\n- なし\n\n## 連絡\n- 連絡\n";
        assert_eq!(
            split_note(full, &t),
            "- なし\n\n## 明日の予定\n- なし\n\n## 連絡\n- 連絡"
        );
        // Only section headings: no note.
        let full = "# 日報\n\n## 待ち\n- なし\n\n## 待ち状況\n- なし\n";
        assert_eq!(split_note(full, &t), "");
        // A level-3 heading or an indented one is not a section.
        let full = "# 日報\n\n## 待ち\n### P\n  ## ログ\n";
        assert_eq!(split_note(full, &t), "");
    }

    #[test]
    fn a_report_whose_note_heading_was_deleted_has_no_note() {
        // As in the MVP: nothing tells which lines were the note.
        let full = "# 日報\n\n## 待ち\n- なし\n\n## 日限注意\n- なし\n- 手で書いた所感\n";
        assert_eq!(split_note(full, &default_template()), "");
    }

    #[test]
    fn a_note_that_writes_the_default_heading_again_is_kept_whole() {
        let full =
            "# 日報\n\n## 待ち\n- なし\n\n## 所感・連絡事項\n- 前半\n\n## 所感・連絡事項\n- 後半\n";
        assert_eq!(
            split_note(full, &default_template()),
            "- 前半\n\n## 所感・連絡事項\n- 後半"
        );
    }

    #[test]
    fn headings_inside_work_logs_are_not_the_note() {
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
            ..todo(Status::Doing)
        };
        let log = WorkLog {
            id: 1,
            todo_id: 7,
            log_date: d("2026-11-19"),
            body_md: "## 所感・連絡事項\nログの本文\n## 連絡\n## メモ".into(),
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
        // Made when the note was 「連絡」.
        let then = template_with(&[(SectionKind::Note, "連絡")]);
        let full = compose(&generate_body(&input, &then), "- 所感", &then);
        assert!(
            full.contains("  ## 所感・連絡事項\n  ログの本文\n  ## 連絡\n  ## メモ\n"),
            "{full}"
        );
        assert_eq!(split_note(&full, &then), "- 所感");
        // Found again after the heading changed to 「メモ」, and by default.
        let now = template_with(&[(SectionKind::Note, "メモ")]);
        assert_eq!(split_note(&full, &now), "- 所感");
        assert_eq!(split_note(&full, &default_template()), "- 所感");
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
            generate_body(&input, &default_template()),
            "# 日報 2026-11-19(木)\n\n## 本日の作業\n\n- なし\n\n## 本日完了・中止\n- なし\n\n## 待ち\n- なし\n\n## 明日の予定\n- なし\n\n## 日限注意\n- なし\n"
        );
        // Without project headings, 本日の作業 looks like the other sections.
        let mut flat = default_template();
        for s in flat.sections.iter_mut() {
            if let SectionBody::Work(o) = &mut s.body {
                o.project_headings = false;
            }
        }
        assert!(
            generate_body(&input, &flat).starts_with(
                "# 日報 2026-11-19(木)\n\n## 本日の作業\n- なし\n\n## 本日完了・中止\n"
            )
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
        let body = generate_body(&input, &default_template());
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
            generate_body(
                &ReportInput {
                    date: d(date),
                    rules: &rules(3),
                    projects: std::slice::from_ref(&p),
                    todos: std::slice::from_ref(&t),
                    logs: &[],
                    events: &[],
                },
                &default_template(),
            )
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
            generate_body(
                &ReportInput {
                    date: d("2026-11-20"),
                    rules: r,
                    projects: std::slice::from_ref(&p),
                    todos: &todos,
                    logs: &[],
                    events: &[],
                },
                &default_template(),
            )
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

        // Without the day counts, the 前倒し note stays with the due date.
        let mut quiet = default_template();
        for s in quiet.sections.iter_mut() {
            if let SectionBody::Attention(o) = &mut s.body {
                o.days = false;
            }
        }
        let business_quiet = generate_body(
            &ReportInput {
                date: d("2026-11-20"),
                rules: &business_rules(3, vec![]),
                projects: std::slice::from_ref(&p),
                todos: &todos,
                logs: &[],
                events: &[],
            },
            &quiet,
        );
        assert!(
            business_quiet.contains(concat!(
                "## 日限注意\n",
                "- 【超過】P / C(日限 11/18)\n",
                "- 【本日】P / A(日限 11/22、休日のため前倒し)\n",
                "- 【間近】P / B(日限 11/24)\n",
            )),
            "{business_quiet}"
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
