//! 全文検索 (T-7): splitting the words, the text around each hit and the
//! order of the results. Pure.
//!
//! A word matches the way SQLite's `LIKE` matches (`repo::search`): ASCII
//! letters ignore case, everything else (全角・半角, かな) is compared as is.

use std::cmp::Reverse;
use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

use super::model::WorkLog;
use super::views::TodoItem;

/// Most TODOs `search_todos` returns.
pub const MAX_HITS: usize = 100;
/// Matching work logs shown per TODO.
pub const MAX_LOGS: usize = 3;
/// Characters kept before and after the first hit of a field.
pub const CONTEXT_CHARS: usize = 30;

const ELLIPSIS: &str = "…";

/// Splits `text` into words on any whitespace (the 全角 space included). A
/// word typed twice (ASCII case ignored) is kept once.
pub fn split_terms(text: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for word in text.split(char::is_whitespace).filter(|w| !w.is_empty()) {
        if !terms.iter().any(|t| t.eq_ignore_ascii_case(word)) {
            terms.push(word.to_owned());
        }
    }
    terms
}

/// A piece of text: one of the words (`hit`) or the text between them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Segment {
    pub text: String,
    pub hit: bool,
}

/// Where a TODO matched, besides its title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MatchField {
    /// メモ
    Memo,
    /// 待っている相手
    WaitingOn,
    /// 中止の理由
    CloseReason,
    /// 作業ログの本文
    WorkLog,
}

/// One place a word was found, with the text around it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct SearchMatch {
    pub field: MatchField,
    /// The work log's date (`work_log` only).
    pub log_date: Option<NaiveDate>,
    /// About 30 characters before and after the first hit, line breaks as
    /// spaces, with 「…」 where the text was cut.
    pub segments: Vec<Segment>,
}

/// A TODO that contains every word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct SearchHit {
    pub item: TodoItem,
    /// The whole title, with the words marked (no hit when the title has
    /// none of them).
    pub title: Vec<Segment>,
    /// Memo, 待っている相手, 中止の理由, then up to 3 work logs (newest
    /// first). Only the fields that contain a word.
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct SearchResult {
    /// At most 100, title hits first, then the most recently updated.
    pub hits: Vec<SearchHit>,
    /// Every TODO that contains every word (may exceed `hits`).
    pub total: i64,
}

impl SearchResult {
    pub fn empty() -> SearchResult {
        SearchResult {
            hits: Vec::new(),
            total: 0,
        }
    }
}

/// A matching TODO before ordering (what `repo::search` returns).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub id: i64,
    pub title: String,
    pub updated_at: DateTime<Utc>,
}

/// The ids of the first `limit` results: TODOs whose title contains any of
/// the words first, then the most recently updated, then the larger id.
pub fn rank(candidates: Vec<Candidate>, terms: &[String], limit: usize) -> Vec<i64> {
    let terms = term_chars(terms);
    let mut keyed: Vec<(bool, Candidate)> = candidates
        .into_iter()
        .map(|c| (!ranges(&chars(&c.title), &terms).is_empty(), c))
        .collect();
    keyed.sort_by_key(|(title_hit, c)| (Reverse(*title_hit), Reverse(c.updated_at), Reverse(c.id)));
    keyed.into_iter().take(limit).map(|(_, c)| c.id).collect()
}

/// Builds the hit for `item` from its fields and its work logs (`logs` may
/// hold other TODOs' logs; they are ignored).
pub fn search_hit(item: TodoItem, logs: &[WorkLog], terms: &[String]) -> SearchHit {
    let terms = term_chars(terms);
    let todo = &item.todo;
    let mut matches = Vec::new();
    let fields = [
        (MatchField::Memo, Some(todo.memo_md.as_str())),
        (MatchField::WaitingOn, todo.waiting_on.as_deref()),
        (MatchField::CloseReason, todo.close_reason.as_deref()),
    ];
    for (field, text) in fields {
        if let Some(segments) = text.and_then(|t| snippet(t, &terms)) {
            matches.push(SearchMatch {
                field,
                log_date: None,
                segments,
            });
        }
    }

    let mut own: Vec<&WorkLog> = logs.iter().filter(|l| l.todo_id == todo.id).collect();
    own.sort_by_key(|l| (Reverse(l.log_date), Reverse(l.id)));
    matches.extend(
        own.into_iter()
            .filter_map(|l| {
                snippet(&l.body_md, &terms).map(|segments| SearchMatch {
                    field: MatchField::WorkLog,
                    log_date: Some(l.log_date),
                    segments,
                })
            })
            .take(MAX_LOGS),
    );

    let title_chars = chars(&todo.title);
    let title_ranges = ranges(&title_chars, &terms);
    SearchHit {
        title: segments(&title_chars, &title_ranges, 0, title_chars.len()),
        matches,
        item,
    }
}

/// Groups work logs by TODO (for `search_hit`).
pub fn logs_by_todo(logs: Vec<WorkLog>) -> HashMap<i64, Vec<WorkLog>> {
    let mut map: HashMap<i64, Vec<WorkLog>> = HashMap::new();
    for l in logs {
        map.entry(l.todo_id).or_default().push(l);
    }
    map
}

// ---- matching ------------------------------------------------------------------

fn term_chars(terms: &[String]) -> Vec<Vec<char>> {
    terms
        .iter()
        .map(|t| t.chars().collect::<Vec<char>>())
        .filter(|t| !t.is_empty())
        .collect()
}

/// The text as characters, each run of line breaks turned into one space.
fn chars(text: &str) -> Vec<char> {
    let mut out = Vec::with_capacity(text.len());
    let mut in_break = false;
    for c in text.chars() {
        if c == '\n' || c == '\r' {
            if !in_break {
                out.push(' ');
            }
            in_break = true;
        } else {
            out.push(c);
            in_break = false;
        }
    }
    out
}

fn same(a: char, b: char) -> bool {
    a.eq_ignore_ascii_case(&b)
}

/// Every place any word occurs, as `[start, end)` character ranges, sorted
/// and with overlapping or touching ranges merged.
fn ranges(text: &[char], terms: &[Vec<char>]) -> Vec<(usize, usize)> {
    let mut found: Vec<(usize, usize)> = Vec::new();
    for term in terms {
        if term.len() > text.len() {
            continue;
        }
        for start in 0..=text.len() - term.len() {
            if text[start..start + term.len()]
                .iter()
                .zip(term)
                .all(|(a, b)| same(*a, *b))
            {
                found.push((start, start + term.len()));
            }
        }
    }
    found.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(found.len());
    for (s, e) in found {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    merged
}

/// About `CONTEXT_CHARS` characters on each side of the first hit, or `None`
/// when no word occurs. A hit that the window would cut is kept whole.
fn snippet(text: &str, terms: &[Vec<char>]) -> Option<Vec<Segment>> {
    let text = chars(text);
    let found = ranges(&text, terms);
    let first = *found.first()?;
    let mut start = first.0.saturating_sub(CONTEXT_CHARS);
    let mut end = (first.1 + CONTEXT_CHARS).min(text.len());
    if let Some(cut) = found.iter().find(|r| r.0 < end && r.1 > end) {
        end = cut.1;
    }
    while start < first.0 && text[start].is_whitespace() {
        start += 1;
    }
    while end > first.1 && text[end - 1].is_whitespace() {
        end -= 1;
    }
    let mut out = Vec::new();
    if start > 0 {
        push(&mut out, ELLIPSIS, false);
    }
    out.extend(segments(&text, &found, start, end));
    if end < text.len() {
        push(&mut out, ELLIPSIS, false);
    }
    Some(out)
}

/// `text[start..end]` split into hits and the text between them.
fn segments(text: &[char], found: &[(usize, usize)], start: usize, end: usize) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut at = start;
    for &(s, e) in found {
        let (s, e) = (s.max(start), e.min(end));
        if s >= e {
            continue;
        }
        if at < s {
            push(&mut out, &text[at..s].iter().collect::<String>(), false);
        }
        push(&mut out, &text[s..e].iter().collect::<String>(), true);
        at = e;
    }
    if at < end {
        push(&mut out, &text[at..end].iter().collect::<String>(), false);
    }
    out
}

/// Appends `text`, joining it to the last segment when both are hits or both
/// are not.
fn push(out: &mut Vec<Segment>, text: &str, hit: bool) {
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(last) if last.hit == hit => last.text.push_str(text),
        _ => out.push(Segment {
            text: text.to_owned(),
            hit,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Status;
    use crate::domain::testing::{d, rules, todo, ts};
    use crate::domain::views::todo_item;

    fn terms(text: &str) -> Vec<String> {
        split_terms(text)
    }

    /// Segments as text with the hits in brackets: 「…の[認証]方式」.
    fn marked(segments: &[Segment]) -> String {
        segments
            .iter()
            .map(|s| {
                if s.hit {
                    format!("[{}]", s.text)
                } else {
                    s.text.clone()
                }
            })
            .collect()
    }

    fn snip(text: &str, words: &str) -> Option<String> {
        snippet(text, &term_chars(&terms(words))).map(|s| marked(&s))
    }

    #[test]
    fn splits_on_ascii_and_zenkaku_spaces() {
        assert_eq!(terms("認証 API"), vec!["認証", "API"]);
        assert_eq!(terms("認証\u{3000}API"), vec!["認証", "API"]);
        assert_eq!(terms("  認証 \t\u{3000} API\n"), vec!["認証", "API"]);
        assert_eq!(terms(""), Vec::<String>::new());
        assert_eq!(terms(" \u{3000} "), Vec::<String>::new());
        // % and _ are words like any other.
        assert_eq!(terms("100% a_b"), vec!["100%", "a_b"]);
    }

    #[test]
    fn a_repeated_word_is_kept_once() {
        assert_eq!(terms("認証 認証"), vec!["認証"]);
        assert_eq!(terms("api API Api"), vec!["api"]);
        // Only ASCII ignores case: 全角 letters are different words.
        assert_eq!(terms("API ＡＰＩ"), vec!["API", "ＡＰＩ"]);
    }

    #[test]
    fn marks_every_word_in_the_window() {
        assert_eq!(
            snip("認証方式の確認と、認証APIの設計", "認証 API").unwrap(),
            "[認証]方式の確認と、[認証API]の設計"
        );
        // ASCII ignores case, the original text is kept.
        assert_eq!(snip("Api の設計", "api").unwrap(), "[Api] の設計");
        // 全角 and 半角 are not the same.
        assert_eq!(snip("ＡＰＩの設計", "API"), None);
        assert_eq!(snip("設計", "認証"), None);
    }

    #[test]
    fn overlapping_words_are_one_hit() {
        assert_eq!(
            snip("認証APIの設計", "認証A API").unwrap(),
            "[認証API]の設計"
        );
        assert_eq!(snip("ああああ", "ああ").unwrap(), "[ああああ]");
    }

    #[test]
    fn keeps_30_characters_on_each_side_of_the_first_hit() {
        let before = "あ".repeat(40);
        let after = "い".repeat(40);
        let text = format!("{before}認証{after}");
        assert_eq!(
            snip(&text, "認証").unwrap(),
            format!("…{}[認証]{}…", "あ".repeat(30), "い".repeat(30))
        );
        // Exactly 30 on each side: nothing is cut.
        let text = format!("{}認証{}", "あ".repeat(30), "い".repeat(30));
        assert_eq!(
            snip(&text, "認証").unwrap(),
            format!("{}[認証]{}", "あ".repeat(30), "い".repeat(30))
        );
        // A hit at the very start.
        let text = format!("認証{after}");
        assert_eq!(
            snip(&text, "認証").unwrap(),
            format!("[認証]{}…", "い".repeat(30))
        );
    }

    #[test]
    fn a_later_hit_cut_by_the_window_is_kept_whole() {
        let text = format!("認証{}レビュー{}", "い".repeat(28), "う".repeat(10));
        assert_eq!(
            snip(&text, "認証 レビュー").unwrap(),
            format!("[認証]{}[レビュー]…", "い".repeat(28))
        );
        // Hits outside the window are not shown.
        let text = format!("認証{}レビュー", "い".repeat(40));
        assert_eq!(
            snip(&text, "認証 レビュー").unwrap(),
            format!("[認証]{}…", "い".repeat(30))
        );
    }

    #[test]
    fn line_breaks_become_one_space() {
        assert_eq!(
            snip("- 進め方\n- 認証の確認\r\n\n- 次", "認証").unwrap(),
            "- 進め方 - [認証]の確認 - 次"
        );
        // The window does not start or end with the space of a line break.
        let text = format!(
            "{}\n{}認証{}\n{}",
            "あ".repeat(10),
            "え".repeat(29),
            "お".repeat(29),
            "か".repeat(10)
        );
        assert_eq!(
            snip(&text, "認証").unwrap(),
            format!("…{}[認証]{}…", "え".repeat(29), "お".repeat(29))
        );
        let text = format!("{}\n認証", "あ".repeat(30));
        assert_eq!(
            snip(&text, "認証").unwrap(),
            format!("…{} [認証]", "あ".repeat(29))
        );
    }

    #[test]
    fn counts_characters_not_bytes() {
        // 30 characters of mixed width before the hit.
        let text = format!("{}認証", "aあ".repeat(20));
        assert_eq!(
            snip(&text, "認証").unwrap(),
            format!("…{}[認証]", "aあ".repeat(15))
        );
    }

    fn candidate(id: i64, title: &str, updated_day: u32) -> Candidate {
        Candidate {
            id,
            title: title.into(),
            updated_at: ts(2026, 11, updated_day, 0, 0),
        }
    }

    #[test]
    fn title_hits_come_first_then_the_newest_update() {
        let list = vec![
            candidate(1, "週次の打ち合わせ", 19),
            candidate(2, "認証方式の検討", 10),
            candidate(3, "API設計", 18),
            candidate(4, "認証のレビュー", 12),
            candidate(5, "資料の準備", 19),
        ];
        // Any of the words in the title counts as a title hit.
        assert_eq!(
            rank(list.clone(), &terms("認証 レビュー"), 100),
            vec![4, 2, 5, 1, 3]
        );
        assert_eq!(rank(list.clone(), &terms("api"), 100), vec![3, 5, 1, 4, 2]);
        assert_eq!(rank(list, &terms("認証"), 2), vec![4, 2]);
    }

    fn log(id: i64, todo_id: i64, day: &str, body: &str) -> WorkLog {
        WorkLog {
            id,
            todo_id,
            log_date: d(day),
            body_md: body.into(),
            minutes: None,
            created_at: ts(2026, 11, 1, 0, 0),
            updated_at: ts(2026, 11, 1, 0, 0),
        }
    }

    fn item(t: crate::domain::model::Todo) -> TodoItem {
        todo_item(
            t,
            "基幹システム更改".into(),
            &crate::domain::subtodo::Family::default(),
            None,
            d("2026-11-19"),
            &rules(3),
        )
    }

    #[test]
    fn lists_the_fields_that_contain_a_word() {
        let mut t = todo(Status::Cancelled);
        t.title = "API設計書のレビュー".into();
        t.memo_md = "# 概要\n認証方式を決める".into();
        t.waiting_on = Some("認証担当の回答".into());
        t.close_reason = Some("別の案件で認証を扱うため".into());
        let hit = search_hit(item(t), &[], &terms("認証"));

        assert_eq!(marked(&hit.title), "API設計書のレビュー");
        assert!(hit.title.iter().all(|s| !s.hit));
        let fields: Vec<(MatchField, String)> = hit
            .matches
            .iter()
            .map(|m| (m.field, marked(&m.segments)))
            .collect();
        assert_eq!(
            fields,
            vec![
                (MatchField::Memo, "# 概要 [認証]方式を決める".to_owned()),
                (MatchField::WaitingOn, "[認証]担当の回答".to_owned()),
                (
                    MatchField::CloseReason,
                    "別の案件で[認証]を扱うため".to_owned()
                ),
            ]
        );
        assert_eq!(hit.item.project_name, "基幹システム更改");
    }

    #[test]
    fn the_title_is_marked_whole() {
        let mut t = todo(Status::Doing);
        t.title = format!("{}認証{}", "あ".repeat(40), "い".repeat(40));
        let hit = search_hit(item(t), &[], &terms("認証"));
        assert_eq!(
            marked(&hit.title),
            format!("{}[認証]{}", "あ".repeat(40), "い".repeat(40))
        );
        assert!(hit.matches.is_empty());
    }

    #[test]
    fn up_to_three_matching_work_logs_newest_first() {
        let t = todo(Status::Doing);
        let logs = vec![
            log(1, 1, "2026-11-10", "認証の調査"),
            log(2, 1, "2026-11-18", "設計の見直し"),
            log(3, 1, "2026-11-17", "認証APIの確認"),
            log(4, 1, "2026-11-19", "認証担当に連絡"),
            log(5, 1, "2026-11-17", "認証の打ち合わせ"),
            log(6, 2, "2026-11-19", "別のTODOの認証"),
        ];
        let hit = search_hit(item(t), &logs, &terms("認証"));
        let found: Vec<(Option<NaiveDate>, String)> = hit
            .matches
            .iter()
            .map(|m| {
                assert_eq!(m.field, MatchField::WorkLog);
                (m.log_date, marked(&m.segments))
            })
            .collect();
        assert_eq!(
            found,
            vec![
                (Some(d("2026-11-19")), "[認証]担当に連絡".to_owned()),
                // Same day: the later log first.
                (Some(d("2026-11-17")), "[認証]の打ち合わせ".to_owned()),
                (Some(d("2026-11-17")), "[認証]APIの確認".to_owned()),
            ]
        );
    }

    #[test]
    fn groups_logs_by_todo() {
        let map = logs_by_todo(vec![
            log(1, 1, "2026-11-10", "a"),
            log(2, 2, "2026-11-10", "b"),
            log(3, 1, "2026-11-11", "c"),
        ]);
        assert_eq!(map[&1].iter().map(|l| l.id).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(map[&2].len(), 1);
    }
}
