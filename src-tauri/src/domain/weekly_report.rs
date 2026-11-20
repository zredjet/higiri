//! 週報 (R-6, v0.3.0). Pure.
//!
//! The week runs from Monday to Sunday: any day picked opens the week it
//! falls in. 今週の作業 and 今週完了・中止 come from the week's records; 待ち,
//! 来週の予定 and 日限注意 judge the TODOs' current state with the base day as
//! "today": the week's Sunday, or today when the week is not over (as the
//! daily report does for a past day).
//!
//! The layout follows the daily report: the lines of 待ち and 日限注意 are the
//! daily report's (`report::waiting_lines`, `report::attention_lines`),
//! children read 「親 / 子」, and in 営業日 the day counts and
//! 「休日のため前倒し」 are written the same way. 所感・連絡事項 is put last and
//! found again with `report::compose` and `report::split_note`.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Days, NaiveDate};

use super::date::{format_date, month_day_slash};
use super::events::{EventKind, TodoEvent};
use super::model::{Project, Status, Todo, WorkLog};
use super::report::{
    Lookup, NONE_LINE, attention_lines, hours, parenthesized, section, waiting_lines,
};
use super::report_template::{
    NextWeekOptions, WeeklyClosedOptions, WeeklyReportTemplate, WeeklySectionBody,
    WeeklyWorkOptions,
};
use super::urgency::Rules;

/// The Monday of the week `date` falls in.
pub fn week_start(date: NaiveDate) -> NaiveDate {
    let back = u64::from(date.weekday().num_days_from_monday());
    date.checked_sub_days(Days::new(back)).unwrap_or(date)
}

/// The Sunday of the week that starts on `start` (a Monday).
pub fn week_end(start: NaiveDate) -> NaiveDate {
    start
        .checked_add_days(Days::new(6))
        .unwrap_or(NaiveDate::MAX)
}

/// The week of a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Week {
    /// Monday.
    pub start: NaiveDate,
    /// Sunday.
    pub end: NaiveDate,
    /// The day 待ち and 日限注意 are judged on: the earlier of Sunday and today.
    pub base: NaiveDate,
}

impl Week {
    /// The week `date` falls in, seen on `today`.
    pub fn of(date: NaiveDate, today: NaiveDate) -> Week {
        let start = week_start(date);
        let end = week_end(start);
        Week {
            start,
            end,
            base: end.min(today),
        }
    }

    fn contains(self, date: NaiveDate) -> bool {
        self.start <= date && date <= self.end
    }

    /// The Sunday of the week after (来週の予定 looks up to it).
    fn next_end(self) -> NaiveDate {
        self.end
            .checked_add_days(Days::new(7))
            .unwrap_or(NaiveDate::MAX)
    }
}

/// 「# 週報 2026-11-16〜2026-11-22」 (with the default template).
pub fn title(template: &WeeklyReportTemplate, week: Week) -> String {
    format!(
        "# {}",
        template.fill_title(&format_date(week.start), &format_date(week.end))
    )
}

/// What a weekly report is made from.
pub struct WeeklyReportInput<'a> {
    /// Any day of the week (its Monday is taken).
    pub date: NaiveDate,
    /// The PC's today (see [`Week::base`]).
    pub today: NaiveDate,
    pub rules: &'a Rules,
    /// All projects (archived included), in display order.
    pub projects: &'a [Project],
    /// All TODOs, in display order.
    pub todos: &'a [Todo],
    /// Work logs of the week (logs of other days are left out).
    pub logs: &'a [WorkLog],
    /// Changes recorded in the week (changes on other days are left out).
    pub events: &'a [TodoEvent],
}

/// One TODO's work in the week.
#[derive(Default)]
struct TodoWork<'a> {
    /// Minutes of the logs with a time (shown when more than 0, as in the
    /// daily report).
    minutes: i64,
    /// The days worked, each with its logs' text in the order written.
    days: BTreeMap<NaiveDate, Vec<&'a str>>,
}

/// 「- **API設計書のレビュー対応**(待ち / 日限 11/20 / 作業日 11/17・11/19)3.5h」,
/// with 「プロジェクト名 / 」 in front when there are no project headings.
fn work_line(t: &Todo, w: &TodoWork, o: &WeeklyWorkOptions, lk: &Lookup) -> String {
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
    if o.work_days {
        let days: Vec<String> = w.days.keys().map(|d| month_day_slash(*d)).collect();
        meta.push(format!("作業日 {}", days.join("・")));
    }
    let meta = if meta.is_empty() {
        String::new()
    } else {
        format!("({})", meta.join(" / "))
    };
    let time = if o.work_time && w.minutes > 0 {
        hours(w.minutes)
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
    format!("- {project}**{}**{meta}{gap}{time}", lk.title(t))
}

/// 今週の作業: the week's logs grouped by project (or not, without project
/// headings), then by TODO (priority, then order), with the week's total on top.
fn work_section(
    input: &WeeklyReportInput,
    week: Week,
    lk: &Lookup,
    heading: &str,
    o: &WeeklyWorkOptions,
    out: &mut String,
) {
    out.push_str("\n## ");
    out.push_str(heading);
    out.push('\n');
    let mut logs: Vec<&WorkLog> = input
        .logs
        .iter()
        .filter(|l| week.contains(l.log_date))
        .collect();
    logs.sort_by_key(|l| (l.log_date, l.created_at, l.id));
    let mut per_todo: HashMap<i64, TodoWork> = HashMap::new();
    for l in logs {
        let w = per_todo.entry(l.todo_id).or_default();
        w.minutes += l.minutes.unwrap_or(0);
        w.days
            .entry(l.log_date)
            .or_default()
            .push(l.body_md.as_str());
    }
    let mut todos: Vec<&Todo> = per_todo
        .keys()
        .filter_map(|id| lk.todos.get(id).copied())
        .collect();
    if todos.is_empty() {
        // As in the daily report: a blank line, as before a project heading.
        if o.project_headings {
            out.push('\n');
        }
        out.push_str(NONE_LINE);
        out.push('\n');
        return;
    }
    todos.sort_by_key(|t| lk.work_order(t));
    let total: i64 = todos.iter().map(|t| per_todo[&t.id].minutes).sum();
    if o.total_time && total > 0 {
        out.push_str(&format!("\n作業時間の合計 {}\n", hours(total)));
        // A project heading starts with a blank line; a list needs one here.
        if !o.project_headings {
            out.push('\n');
        }
    }
    let mut current_project: Option<i64> = None;
    for t in todos {
        if o.project_headings && current_project != Some(t.project_id) {
            current_project = Some(t.project_id);
            out.push_str(&format!("\n### {}\n", lk.project_name(t)));
        }
        let w = &per_todo[&t.id];
        out.push_str(&work_line(t, w, o, lk));
        out.push('\n');
        if !o.log_body {
            continue;
        }
        for (day, bodies) in &w.days {
            out.push_str(&format!("  - {}\n", month_day_slash(*day)));
            for body in bodies {
                for line in body.trim().lines() {
                    if line.trim().is_empty() {
                        out.push('\n');
                    } else {
                        out.push_str("    ");
                        out.push_str(line.trim_end());
                        out.push('\n');
                    }
                }
            }
        }
    }
}

/// 今週完了・中止: TODOs that became 完了/中止 in the week and still are, with
/// the day of the last such change; 完了 first.
fn closed_lines(
    input: &WeeklyReportInput,
    week: Week,
    lk: &Lookup,
    o: &WeeklyClosedOptions,
) -> Vec<String> {
    // TODO id -> (完了 0 / 中止 1, the last change that closed it in the week)
    let mut found: HashMap<i64, (u8, &TodoEvent)> = HashMap::new();
    for e in input
        .events
        .iter()
        .filter(|e| e.kind == EventKind::Status && week.contains(e.event_date))
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
        let later = found
            .get(&t.id)
            .is_none_or(|(_, f)| (f.occurred_at, f.id) < (e.occurred_at, e.id));
        if later {
            found.insert(t.id, (rank, e));
        }
    }
    let mut found: Vec<(u8, &TodoEvent)> = found.into_values().collect();
    found.sort_by_key(|(rank, e)| (*rank, e.occurred_at, e.id));
    found
        .into_iter()
        .filter_map(|(_, e)| {
            let t = lk.todos.get(&e.todo_id)?;
            let mut parts: Vec<String> = Vec::new();
            if o.closed_date {
                parts.push(month_day_slash(e.event_date));
            }
            if o.close_reason
                && t.status == Status::Cancelled
                && let Some(r) = &t.close_reason
            {
                parts.push(format!("理由:{r}"));
            }
            Some(format!(
                "- 【{}】{} / {}{}",
                t.status.label(),
                lk.project_name(t),
                lk.title(t),
                parenthesized(&parts)
            ))
        })
        .collect()
}

/// 来週の予定: 作業中, and 未着手 planned to start by the next week's Sunday.
/// Active projects, display order.
fn next_week_lines(
    input: &WeeklyReportInput,
    week: Week,
    lk: &Lookup,
    o: &NextWeekOptions,
) -> Vec<String> {
    let until = week.next_end();
    input
        .todos
        .iter()
        .filter(|t| lk.is_active(t))
        .filter_map(|t| {
            let mut parts: Vec<String> = Vec::new();
            match t.status {
                Status::Doing => {}
                Status::Todo if t.planned_start.is_some_and(|s| s <= until) => {
                    if o.planned_period {
                        let start = t.planned_start.map(month_day_slash).unwrap_or_default();
                        let end = t.planned_end.map(month_day_slash).unwrap_or_default();
                        parts.push(format!("予定 {start}〜{end}"));
                    }
                }
                _ => return None,
            }
            if o.due_date
                && let Some(due) = t.due_date
            {
                parts.push(format!("日限 {}", month_day_slash(due)));
            }
            Some(format!(
                "- {} / {}{}",
                lk.project_name(t),
                lk.title(t),
                parenthesized(&parts)
            ))
        })
        .collect()
}

/// The generated part: the title and the shown sections in the template's
/// order, without 所感・連絡事項 (added last by `report::compose`).
pub fn generate_body(input: &WeeklyReportInput, template: &WeeklyReportTemplate) -> String {
    let week = Week::of(input.date, input.today);
    let lk = Lookup::new(input.projects, input.todos);
    let mut out = title(template, week);
    out.push('\n');
    for s in template.sections.iter().filter(|s| s.enabled) {
        let heading = s.heading.as_str();
        match &s.body {
            WeeklySectionBody::Work(o) => work_section(input, week, &lk, heading, o, &mut out),
            WeeklySectionBody::Closed(o) => {
                section(&mut out, heading, closed_lines(input, week, &lk, o))
            }
            WeeklySectionBody::Waiting(o) => section(
                &mut out,
                heading,
                waiting_lines(input.todos, &lk, week.base, o),
            ),
            WeeklySectionBody::NextWeek(o) => {
                section(&mut out, heading, next_week_lines(input, week, &lk, o))
            }
            WeeklySectionBody::Attention(o) => section(
                &mut out,
                heading,
                attention_lines(input.todos, &lk, week.base, input.rules, o),
            ),
            WeeklySectionBody::Note(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::EventNote;
    use crate::domain::model::Priority;
    use crate::domain::report::{compose, split_note};
    use crate::domain::report_template::WeeklySectionKind;
    use crate::domain::testing::{business_rules, d, rules, todo, ts};

    fn project(id: i64, name: &str) -> Project {
        Project {
            id,
            name: name.into(),
            color: "#3B6FD4".into(),
            description: String::new(),
            sort_order: id,
            archived_at: None,
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        }
    }

    fn log(id: i64, todo_id: i64, date: &str, minutes: Option<i64>, body: &str) -> WorkLog {
        WorkLog {
            id,
            todo_id,
            log_date: d(date),
            body_md: body.into(),
            minutes,
            created_at: ts(2026, 11, 1, 0, 0) + chrono::Duration::minutes(id),
            updated_at: ts(2026, 11, 1, 0, 0),
        }
    }

    fn status_event(id: i64, todo_id: i64, date: &str, from: &str, to: &str) -> TodoEvent {
        let at = d(date).and_hms_opt(9, 0, 0).unwrap().and_utc() + chrono::Duration::minutes(id);
        TodoEvent {
            id,
            todo_id,
            event_date: d(date),
            kind: EventKind::Status,
            old_value: Some(from.into()),
            new_value: Some(to.into()),
            note: EventNote::default(),
            occurred_at: at,
        }
    }

    fn mk(id: i64, project_id: i64, title: &str, status: Status) -> Todo {
        Todo {
            id,
            project_id,
            title: title.into(),
            sort_order: id,
            ..todo(status)
        }
    }

    fn template(change: impl FnOnce(&mut WeeklyReportTemplate)) -> WeeklyReportTemplate {
        let mut t = WeeklyReportTemplate::default();
        change(&mut t);
        t
    }

    fn work_options(t: &mut WeeklyReportTemplate) -> &mut WeeklyWorkOptions {
        t.sections
            .iter_mut()
            .find_map(|s| match &mut s.body {
                WeeklySectionBody::Work(o) => Some(o),
                _ => None,
            })
            .unwrap()
    }

    struct Data {
        projects: Vec<Project>,
        todos: Vec<Todo>,
        logs: Vec<WorkLog>,
        events: Vec<TodoEvent>,
    }

    impl Data {
        fn body(&self, date: &str, today: &str, r: &Rules, t: &WeeklyReportTemplate) -> String {
            generate_body(
                &WeeklyReportInput {
                    date: d(date),
                    today: d(today),
                    rules: r,
                    projects: &self.projects,
                    todos: &self.todos,
                    logs: &self.logs,
                    events: &self.events,
                },
                t,
            )
        }
    }

    fn empty() -> Data {
        Data {
            projects: vec![project(1, "P")],
            todos: vec![],
            logs: vec![],
            events: vec![],
        }
    }

    #[test]
    fn weeks_run_from_monday_to_sunday() {
        // 2026-11-16 is a Monday.
        for day in [
            "2026-11-16",
            "2026-11-17",
            "2026-11-19",
            "2026-11-21",
            "2026-11-22",
        ] {
            assert_eq!(week_start(d(day)), d("2026-11-16"), "{day}");
        }
        assert_eq!(week_end(d("2026-11-16")), d("2026-11-22"));
        assert_eq!(week_start(d("2026-11-23")), d("2026-11-23"));
        assert_eq!(week_start(d("2026-11-15")), d("2026-11-09"));
        // Across the new year: 2026-12-28 (Mon) to 2027-01-03 (Sun).
        for day in ["2026-12-28", "2026-12-31", "2027-01-01", "2027-01-03"] {
            assert_eq!(week_start(d(day)), d("2026-12-28"), "{day}");
        }
        assert_eq!(week_end(d("2026-12-28")), d("2027-01-03"));
        assert_eq!(week_start(d("2027-01-04")), d("2027-01-04"));
        // Across a month and a leap day.
        assert_eq!(week_start(d("2028-03-01")), d("2028-02-28"));
        assert_eq!(week_end(d("2028-02-28")), d("2028-03-05"));
    }

    #[test]
    fn the_base_day_is_sunday_or_today_whichever_comes_first() {
        let this_week = Week::of(d("2026-11-17"), d("2026-11-19"));
        assert_eq!(
            this_week,
            Week {
                start: d("2026-11-16"),
                end: d("2026-11-22"),
                base: d("2026-11-19"),
            }
        );
        // On the Sunday itself, and a past week.
        assert_eq!(
            Week::of(d("2026-11-22"), d("2026-11-22")).base,
            d("2026-11-22")
        );
        assert_eq!(
            Week::of(d("2026-11-10"), d("2026-11-19")).base,
            d("2026-11-15")
        );
        // A week to come is judged on today.
        assert_eq!(
            Week::of(d("2026-11-25"), d("2026-11-19")).base,
            d("2026-11-19")
        );
    }

    #[test]
    fn an_empty_week_writes_none_in_every_section() {
        let t = WeeklyReportTemplate::default();
        assert_eq!(
            empty().body("2026-11-19", "2026-11-19", &rules(3), &t),
            "# 週報 2026-11-16〜2026-11-22\n\n## 今週の作業\n\n- なし\n\n## 今週完了・中止\n- なし\n\n## 待ち\n- なし\n\n## 来週の予定\n- なし\n\n## 日限注意\n- なし\n"
        );
        let flat = template(|t| work_options(t).project_headings = false);
        assert!(
            empty()
                .body("2026-11-19", "2026-11-19", &rules(3), &flat)
                .starts_with(
                    "# 週報 2026-11-16〜2026-11-22\n\n## 今週の作業\n- なし\n\n## 今週完了・中止\n"
                )
        );
    }

    /// Two projects; A has a timed log on two days and an untimed one, B
    /// only untimed logs, C (in P) is a child; logs outside the week too.
    fn worked() -> Data {
        let parent = Todo {
            due_date: Some(d("2026-11-30")),
            ..mk(10, 1, "親", Status::Doing)
        };
        Data {
            projects: vec![project(1, "P"), project(2, "Q")],
            todos: vec![
                Todo {
                    due_date: Some(d("2026-11-20")),
                    priority: Priority::High,
                    ..mk(1, 1, "A", Status::Doing)
                },
                Todo {
                    due_date: Some(d("2026-11-18")),
                    ..mk(2, 2, "B", Status::Done)
                },
                parent,
                Todo {
                    parent_id: Some(10),
                    ..mk(11, 1, "子", Status::Todo)
                },
            ],
            logs: vec![
                log(1, 1, "2026-11-19", Some(90), "- 反映した\n\n- 残りは来週"),
                log(2, 1, "2026-11-17", Some(30), "下調べ"),
                log(3, 1, "2026-11-17", None, "メモ"),
                log(4, 2, "2026-11-16", None, "- 片付けた"),
                log(5, 11, "2026-11-22", Some(60), "日曜に作業"),
                // Outside the week.
                log(6, 1, "2026-11-15", Some(600), "先週"),
                log(7, 2, "2026-11-23", Some(600), "来週"),
            ],
            events: vec![],
        }
    }

    #[test]
    fn work_lists_days_and_times_per_todo() {
        let body = worked().body(
            "2026-11-19",
            "2026-11-22",
            &rules(3),
            &WeeklyReportTemplate::default(),
        );
        assert!(
            body.contains(concat!(
                "## 今週の作業\n",
                "\n",
                "作業時間の合計 3.0h\n",
                "\n",
                "### P\n",
                "- **A**(作業中 / 日限 11/20 / 作業日 11/17・11/19)2.0h\n",
                "- **親 / 子**(未着手 / 作業日 11/22)1.0h\n",
                "\n",
                "### Q\n",
                "- **B**(完了 / 作業日 11/16)\n",
                "\n## 今週完了・中止\n",
            )),
            "{body}"
        );
    }

    #[test]
    fn work_items_can_be_left_out_and_bodies_put_in() {
        let t = template(|t| {
            let o = work_options(t);
            o.project_headings = false;
            o.status = false;
            o.due_date = false;
            o.work_days = false;
            o.log_body = true;
        });
        let body = worked().body("2026-11-19", "2026-11-22", &rules(3), &t);
        assert!(
            body.contains(concat!(
                "## 今週の作業\n",
                "\n",
                "作業時間の合計 3.0h\n",
                "\n",
                "- P / **A** 2.0h\n",
                "  - 11/17\n",
                "    下調べ\n",
                "    メモ\n",
                "  - 11/19\n",
                "    - 反映した\n",
                "\n",
                "    - 残りは来週\n",
                "- P / **親 / 子** 1.0h\n",
                "  - 11/22\n",
                "    日曜に作業\n",
                "- Q / **B**\n",
                "  - 11/16\n",
                "    - 片付けた\n",
                "\n## 今週完了・中止\n",
            )),
            "{body}"
        );
        // Without the times: no total, no hours.
        let t = template(|t| {
            let o = work_options(t);
            o.work_time = false;
            o.total_time = false;
        });
        let body = worked().body("2026-11-19", "2026-11-22", &rules(3), &t);
        assert!(
            body.contains(concat!(
                "## 今週の作業\n",
                "\n",
                "### P\n",
                "- **A**(作業中 / 日限 11/20 / 作業日 11/17・11/19)\n",
            )),
            "{body}"
        );
    }

    #[test]
    fn no_time_without_minutes() {
        let mut data = worked();
        for l in data.logs.iter_mut() {
            l.minutes = None;
        }
        let body = data.body(
            "2026-11-19",
            "2026-11-22",
            &rules(3),
            &WeeklyReportTemplate::default(),
        );
        assert!(
            body.contains(
                "## 今週の作業\n\n### P\n- **A**(作業中 / 日限 11/20 / 作業日 11/17・11/19)\n"
            ),
            "{body}"
        );
        assert!(!body.contains("作業時間の合計"), "{body}");
        // Logs of 0 minutes: no time and no total, as in the daily report.
        data.logs[0].minutes = Some(0);
        data.logs[1].minutes = Some(0);
        let body = data.body(
            "2026-11-19",
            "2026-11-22",
            &rules(3),
            &WeeklyReportTemplate::default(),
        );
        assert!(
            body.contains(
                "## 今週の作業\n\n### P\n- **A**(作業中 / 日限 11/20 / 作業日 11/17・11/19)\n"
            ),
            "{body}"
        );
        assert!(!body.contains("作業時間の合計"), "{body}");
        assert!(!body.contains("0.0h"), "{body}");
        // One TODO with 0 minutes beside one with time: only the latter has it.
        data.logs[4].minutes = Some(60);
        let body = data.body(
            "2026-11-19",
            "2026-11-22",
            &rules(3),
            &WeeklyReportTemplate::default(),
        );
        assert!(
            body.contains(concat!(
                "## 今週の作業\n",
                "\n",
                "作業時間の合計 1.0h\n",
                "\n",
                "### P\n",
                "- **A**(作業中 / 日限 11/20 / 作業日 11/17・11/19)\n",
                "- **親 / 子**(未着手 / 作業日 11/22)1.0h\n",
            )),
            "{body}"
        );
    }

    fn closed() -> Data {
        let mut todos = vec![
            Todo {
                close_reason: Some("不要になった".into()),
                ..mk(1, 1, "中止", Status::Cancelled)
            },
            mk(2, 1, "完了", Status::Done),
            // Closed in the week, then reopened.
            mk(3, 1, "再開", Status::Doing),
            // Closed before the week.
            mk(4, 1, "先週", Status::Done),
            // Closed, reopened and closed again in the week.
            mk(5, 1, "二度", Status::Done),
            // Closed on the Sunday.
            mk(6, 1, "日曜", Status::Done),
        ];
        todos.push(Todo {
            parent_id: Some(2),
            ..mk(7, 1, "子", Status::Done)
        });
        Data {
            projects: vec![project(1, "P")],
            todos,
            logs: vec![],
            events: vec![
                status_event(1, 1, "2026-11-16", "todo", "cancelled"),
                status_event(2, 2, "2026-11-18", "doing", "done"),
                status_event(3, 3, "2026-11-17", "doing", "done"),
                status_event(4, 3, "2026-11-18", "done", "doing"),
                status_event(5, 4, "2026-11-13", "doing", "done"),
                status_event(6, 5, "2026-11-16", "doing", "done"),
                status_event(7, 5, "2026-11-17", "done", "doing"),
                status_event(8, 5, "2026-11-20", "doing", "done"),
                status_event(9, 6, "2026-11-22", "doing", "done"),
                status_event(10, 7, "2026-11-18", "todo", "done"),
                // The week after.
                status_event(11, 4, "2026-11-23", "done", "done"),
            ],
        }
    }

    #[test]
    fn closed_lists_what_closed_in_the_week_and_still_is() {
        let body = closed().body(
            "2026-11-22",
            "2026-11-30",
            &rules(3),
            &WeeklyReportTemplate::default(),
        );
        assert!(
            body.contains(concat!(
                "## 今週完了・中止\n",
                "- 【完了】P / 完了(11/18)\n",
                "- 【完了】P / 完了 / 子(11/18)\n",
                "- 【完了】P / 二度(11/20)\n",
                "- 【完了】P / 日曜(11/22)\n",
                "- 【中止】P / 中止(11/16、理由:不要になった)\n",
                "\n",
            )),
            "{body}"
        );
        let t = template(|t| {
            for s in t.sections.iter_mut() {
                if let WeeklySectionBody::Closed(o) = &mut s.body {
                    o.closed_date = false;
                }
            }
        });
        let body = closed().body("2026-11-22", "2026-11-30", &rules(3), &t);
        assert!(
            body.contains("- 【完了】P / 完了\n")
                && body.contains("- 【中止】P / 中止(理由:不要になった)\n"),
            "{body}"
        );
        let t = template(|t| {
            for s in t.sections.iter_mut() {
                if let WeeklySectionBody::Closed(o) = &mut s.body {
                    o.close_reason = false;
                }
            }
        });
        let body = closed().body("2026-11-22", "2026-11-30", &rules(3), &t);
        assert!(body.contains("- 【中止】P / 中止(11/16)\n"), "{body}");
    }

    #[test]
    fn waiting_and_attention_are_seen_on_the_base_day() {
        let data = Data {
            projects: vec![project(1, "P")],
            todos: vec![
                Todo {
                    waiting_on: Some("回答".into()),
                    waiting_since: Some(d("2026-11-17")),
                    follow_up_date: Some(d("2026-11-20")),
                    due_date: Some(d("2026-11-24")),
                    ..mk(1, 1, "前から", Status::Waiting)
                },
                // Started waiting after the week of 11/9.
                Todo {
                    waiting_on: Some("承認".into()),
                    waiting_since: Some(d("2026-11-19")),
                    ..mk(2, 1, "あとから", Status::Waiting)
                },
                Todo {
                    due_date: Some(d("2026-11-14")),
                    ..mk(3, 1, "日限", Status::Doing)
                },
            ],
            logs: vec![],
            events: vec![],
        };
        let t = WeeklyReportTemplate::default();
        // This week, seen on Thursday: 待ち3日目, 日限 11/24 is 5 days on.
        let now = data.body("2026-11-16", "2026-11-19", &rules(3), &t);
        assert!(
            now.contains(concat!(
                "## 待ち\n",
                "- P / 前から(回答待ち、3日目、確認日 11/20)\n",
                "- P / あとから(承認待ち、1日目)\n",
            )),
            "{now}"
        );
        assert!(
            now.ends_with("## 日限注意\n- 【超過】P / 日限(日限 11/14、5日超過)\n"),
            "{now}"
        );
        // The week before, seen on its Sunday (11/15): neither was waiting yet
        // and the due date was not past.
        let before = data.body("2026-11-10", "2026-11-19", &rules(3), &t);
        assert!(before.contains("## 待ち\n- なし\n"), "{before}");
        assert!(
            before.contains("## 日限注意\n- 【超過】P / 日限(日限 11/14、1日超過)\n"),
            "{before}"
        );
        // The week of 11/16 seen after it ended: judged on its Sunday.
        let after = data.body("2026-11-16", "2026-11-26", &rules(3), &t);
        assert!(
            after.contains("- P / 前から(回答待ち、6日目、確認日 11/20)\n"),
            "{after}"
        );
        assert!(
            after.contains("- 【間近】P / 前から(日限 11/24、あと2日、待ち)\n"),
            "{after}"
        );
        // In 営業日, as in the daily report: 11/14 is a Saturday.
        let business = data.body("2026-11-16", "2026-11-19", &business_rules(3, vec![]), &t);
        assert!(
            business.contains("- 【超過】P / 日限(日限 11/14、休日のため前倒し、4営業日超過)\n"),
            "{business}"
        );
    }

    #[test]
    fn next_week_lists_doing_and_what_starts_by_next_sunday() {
        let planned = |id, title: &str, start: &str, end: &str| Todo {
            planned_start: Some(d(start)),
            planned_end: Some(d(end)),
            due_date: Some(d("2026-12-01")),
            ..mk(id, 1, title, Status::Todo)
        };
        let data = Data {
            projects: vec![project(1, "P")],
            todos: vec![
                Todo {
                    due_date: Some(d("2026-11-19")),
                    planned_start: Some(d("2026-11-10")),
                    planned_end: Some(d("2026-11-18")),
                    ..mk(1, 1, "作業中", Status::Doing)
                },
                mk(2, 1, "日限なし", Status::Doing),
                planned(3, "来週の日曜", "2026-11-29", "2026-11-30"),
                planned(4, "その先", "2026-11-30", "2026-12-01"),
                planned(5, "遅れ", "2026-11-12", "2026-11-13"),
                Todo {
                    planned_start: Some(d("2026-11-20")),
                    ..mk(6, 1, "保留", Status::Hold)
                },
                mk(7, 1, "予定なし", Status::Todo),
            ],
            logs: vec![],
            events: vec![],
        };
        let body = data.body(
            "2026-11-16",
            "2026-11-19",
            &rules(3),
            &WeeklyReportTemplate::default(),
        );
        assert!(
            body.contains(concat!(
                "## 来週の予定\n",
                "- P / 作業中(日限 11/19)\n",
                "- P / 日限なし\n",
                "- P / 来週の日曜(予定 11/29〜11/30、日限 12/1)\n",
                "- P / 遅れ(予定 11/12〜11/13、日限 12/1)\n",
                "\n",
            )),
            "{body}"
        );
        let t = template(|t| {
            for s in t.sections.iter_mut() {
                if let WeeklySectionBody::NextWeek(o) = &mut s.body {
                    o.planned_period = false;
                }
            }
        });
        let body = data.body("2026-11-16", "2026-11-19", &rules(3), &t);
        assert!(
            body.contains("- P / 来週の日曜(日限 12/1)\n- P / 遅れ(日限 12/1)\n"),
            "{body}"
        );
        let t = template(|t| {
            for s in t.sections.iter_mut() {
                if let WeeklySectionBody::NextWeek(o) = &mut s.body {
                    o.due_date = false;
                }
            }
        });
        let body = data.body("2026-11-16", "2026-11-19", &rules(3), &t);
        assert!(
            body.contains("- P / 作業中\n- P / 日限なし\n- P / 来週の日曜(予定 11/29〜11/30)\n"),
            "{body}"
        );
    }

    #[test]
    fn the_template_sets_the_title_order_and_shown_sections() {
        let t = template(|t| {
            t.title = "{開始日}の週".into();
            t.sections.swap(0, 4);
            for s in t.sections.iter_mut() {
                match s.kind() {
                    WeeklySectionKind::Closed | WeeklySectionKind::Waiting => s.enabled = false,
                    WeeklySectionKind::NextWeek => s.heading = "翌週".into(),
                    _ => {}
                }
            }
        });
        assert_eq!(
            empty().body("2026-11-19", "2026-11-19", &rules(3), &t),
            "# 2026-11-16の週\n\n## 日限注意\n- なし\n\n## 翌週\n- なし\n\n## 今週の作業\n\n- なし\n"
        );
    }

    #[test]
    fn the_note_is_kept_with_either_heading() {
        let mut t = WeeklyReportTemplate::default();
        let body = empty().body("2026-11-19", "2026-11-19", &rules(3), &t);
        let full = compose(&body, "- 来週は試験。", &t);
        assert!(full.ends_with("## 日限注意\n- なし\n\n## 所感・連絡事項\n- 来週は試験。\n"));
        assert_eq!(split_note(&full, &t), "- 来週は試験。");
        // The heading changed: the note under the old (default) one is found.
        t.sections
            .iter_mut()
            .find(|s| s.kind() == WeeklySectionKind::Note)
            .unwrap()
            .heading = "連絡".into();
        assert_eq!(split_note(&full, &t), "- 来週は試験。");
        let again = compose(&body, &split_note(&full, &t), &t);
        assert!(again.ends_with("\n\n## 連絡\n- 来週は試験。\n"), "{again}");
        // A section that now uses 「所感・連絡事項」 is not the note.
        t.sections
            .iter_mut()
            .find(|s| s.kind() == WeeklySectionKind::Attention)
            .unwrap()
            .heading = "所感・連絡事項".into();
        assert_eq!(split_note(&again, &t), "- 来週は試験。");
    }
}
