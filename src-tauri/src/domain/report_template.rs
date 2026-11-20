//! 日報テンプレート (R-5, v1.1) and 週報テンプレート (R-6, v0.3.0). Pure.
//!
//! The order of the sections, their headings, whether each is shown and what
//! its lines carry. The line format itself stays as in the MVP; the default
//! template gives exactly the design's example (「日報の生成」).
//!
//! Stored as the `report_template` setting:
//!
//! ```json
//! { "title": "日報 {日付}({曜日})",
//!   "sections": [
//!     { "kind": "work", "heading": "本日の作業", "enabled": true,
//!       "options": { "project_headings": true, "status": true, "due_date": true, "work_time": true } },
//!     …,
//!     { "kind": "note", "heading": "所感・連絡事項", "enabled": true, "options": {} } ] }
//! ```
//!
//! The weekly report (`weekly_report_template`) has the same shape with its own
//! kinds of section (`work`, `closed`, `waiting`, `next_week`, `attention`,
//! `note`) and items, and the title takes `{開始日}` and `{終了日}`. Its own
//! types ([`WeeklyReportTemplate`]) keep the two apart in JSON and in the
//! bindings; the rules (normalizing, checking, the note's heading) are written
//! once, for both, over [`TemplateSection`].
//!
//! The title and headings are text only: the app adds `# ` and `## `.

use serde::{Deserialize, Serialize};

/// Longest title, in characters (placeholders counted as written).
pub const TITLE_MAX: usize = 60;
/// Longest section heading, in characters.
pub const HEADING_MAX: usize = 40;

pub const DATE_PLACEHOLDER: &str = "{日付}";
pub const WEEKDAY_PLACEHOLDER: &str = "{曜日}";
pub const DEFAULT_TITLE: &str = "日報 {日付}({曜日})";

/// The Monday of the week (weekly report).
pub const START_PLACEHOLDER: &str = "{開始日}";
/// The Sunday of the week (weekly report).
pub const END_PLACEHOLDER: &str = "{終了日}";
pub const DEFAULT_WEEKLY_TITLE: &str = "週報 {開始日}〜{終了日}";

/// A kind of section, of the daily or of the weekly report.
pub trait Kind: Copy + Eq + std::fmt::Debug + 'static {
    /// Every kind, in the default order (所感・連絡事項 last).
    const ALL: &'static [Self];
    /// 所感・連絡事項: always shown, always last.
    const NOTE: Self;
    /// Also the fixed name of the section on the settings screen and in errors.
    fn default_heading(self) -> &'static str;
}

/// A section of a template, daily or weekly.
pub trait TemplateSection: Clone {
    type Kind: Kind;
    fn kind(&self) -> Self::Kind;
    /// Shown, with the default heading and items.
    fn default_for(kind: Self::Kind) -> Self;
    fn heading(&self) -> &str;
    fn set_heading(&mut self, heading: String);
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
}

/// A template, daily or weekly: what the note is looked for with
/// (`report::split_note`) and put under (`report::compose`).
pub trait Template {
    type Section: TemplateSection;
    fn sections(&self) -> &[Self::Section];

    /// The heading of 所感・連絡事項.
    fn note_heading(&self) -> &str {
        let note = <<Self::Section as TemplateSection>::Kind as Kind>::NOTE;
        self.sections()
            .iter()
            .find(|s| s.kind() == note)
            .map(|s| s.heading())
            .unwrap_or(note.default_heading())
    }
}

/// The six sections of the daily report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    /// 本日の作業
    Work,
    /// 本日完了・中止
    Closed,
    /// 待ち
    Waiting,
    /// 明日の予定
    Tomorrow,
    /// 日限注意
    Attention,
    /// 所感・連絡事項 (always shown, always last)
    Note,
}

impl SectionKind {
    /// In the default order.
    pub const ALL: [SectionKind; 6] = [
        SectionKind::Work,
        SectionKind::Closed,
        SectionKind::Waiting,
        SectionKind::Tomorrow,
        SectionKind::Attention,
        SectionKind::Note,
    ];

    pub fn default_heading(self) -> &'static str {
        match self {
            SectionKind::Work => "本日の作業",
            SectionKind::Closed => "本日完了・中止",
            SectionKind::Waiting => "待ち",
            SectionKind::Tomorrow => "明日の予定",
            SectionKind::Attention => "日限注意",
            SectionKind::Note => "所感・連絡事項",
        }
    }
}

impl Kind for SectionKind {
    const ALL: &'static [SectionKind] = &SectionKind::ALL;
    const NOTE: SectionKind = SectionKind::Note;

    fn default_heading(self) -> &'static str {
        SectionKind::default_heading(self)
    }
}

/// 本日の作業.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct WorkOptions {
    /// 「### プロジェクト名」 above each project's TODOs. Off: each line
    /// starts with 「プロジェクト名 / 」.
    pub project_headings: bool,
    /// 「(作業中 …)」
    pub status: bool,
    /// 「… / 日限 11/20」 (open TODOs only).
    pub due_date: bool,
    /// 「1.5h」
    pub work_time: bool,
}

/// 本日完了・中止.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ClosedOptions {
    /// 「(理由:…)」 of 中止.
    pub close_reason: bool,
}

/// 待ち (of the daily and the weekly report).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct WaitingOptions {
    /// 「人事部の原稿確認待ち」
    pub waiting_on: bool,
    /// 「3日目」
    pub waiting_days: bool,
    /// 「確認日 11/19」
    pub follow_up_date: bool,
}

/// 明日の予定.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TomorrowOptions {
    /// 「(予定 11/20〜11/27)」 of 未着手.
    pub planned_period: bool,
}

/// 日限注意 (of the daily and the weekly report).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct AttentionOptions {
    /// 「2日超過」「あと1日」 (and 「待ち3日目」「2日経過」 of 要確認 and 着手遅れ).
    pub days: bool,
    /// 「待ち」 on waiting TODOs.
    pub waiting_mark: bool,
    /// Also list 要確認 and 着手遅れ (off by default: 超過・本日・間近 only).
    pub follow_up_and_late_start: bool,
}

/// 所感・連絡事項 has no options (heading only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct NoteOptions {}

/// The kind of a section and its options (`"kind"` and `"options"` in JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", content = "options", rename_all = "snake_case")]
pub enum SectionBody {
    Work(WorkOptions),
    Closed(ClosedOptions),
    Waiting(WaitingOptions),
    Tomorrow(TomorrowOptions),
    Attention(AttentionOptions),
    Note(NoteOptions),
}

fn default_waiting() -> WaitingOptions {
    WaitingOptions {
        waiting_on: true,
        waiting_days: true,
        follow_up_date: true,
    }
}

fn default_attention() -> AttentionOptions {
    AttentionOptions {
        days: true,
        waiting_mark: true,
        follow_up_and_late_start: false,
    }
}

impl SectionBody {
    pub fn kind(&self) -> SectionKind {
        match self {
            SectionBody::Work(_) => SectionKind::Work,
            SectionBody::Closed(_) => SectionKind::Closed,
            SectionBody::Waiting(_) => SectionKind::Waiting,
            SectionBody::Tomorrow(_) => SectionKind::Tomorrow,
            SectionBody::Attention(_) => SectionKind::Attention,
            SectionBody::Note(_) => SectionKind::Note,
        }
    }

    /// The options of the MVP's report.
    pub fn default_for(kind: SectionKind) -> SectionBody {
        match kind {
            SectionKind::Work => SectionBody::Work(WorkOptions {
                project_headings: true,
                status: true,
                due_date: true,
                work_time: true,
            }),
            SectionKind::Closed => SectionBody::Closed(ClosedOptions { close_reason: true }),
            SectionKind::Waiting => SectionBody::Waiting(default_waiting()),
            SectionKind::Tomorrow => SectionBody::Tomorrow(TomorrowOptions {
                planned_period: true,
            }),
            SectionKind::Attention => SectionBody::Attention(default_attention()),
            SectionKind::Note => SectionBody::Note(NoteOptions {}),
        }
    }
}

/// One section: `{ kind, heading, enabled, options }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Section {
    /// Text only (`## ` is added by the app).
    pub heading: String,
    /// Shown in the report. Always true for 所感・連絡事項.
    pub enabled: bool,
    #[serde(flatten)]
    pub body: SectionBody,
}

impl Section {
    pub fn kind(&self) -> SectionKind {
        self.body.kind()
    }

    pub fn default_for(kind: SectionKind) -> Section {
        Section {
            heading: kind.default_heading().to_owned(),
            enabled: true,
            body: SectionBody::default_for(kind),
        }
    }
}

impl TemplateSection for Section {
    type Kind = SectionKind;

    fn kind(&self) -> SectionKind {
        Section::kind(self)
    }

    fn default_for(kind: SectionKind) -> Section {
        Section::default_for(kind)
    }

    fn heading(&self) -> &str {
        &self.heading
    }

    fn set_heading(&mut self, heading: String) {
        self.heading = heading;
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ReportTemplate {
    /// Text only (`# ` is added by the app); `{日付}` and `{曜日}` are filled in.
    pub title: String,
    /// Every kind once, 所感・連絡事項 last (see [`ReportTemplate::normalized`]).
    pub sections: Vec<Section>,
}

impl Default for ReportTemplate {
    fn default() -> Self {
        ReportTemplate {
            title: DEFAULT_TITLE.to_owned(),
            sections: SectionKind::ALL
                .into_iter()
                .map(Section::default_for)
                .collect(),
        }
    }
}

impl Template for ReportTemplate {
    type Section = Section;

    fn sections(&self) -> &[Section] {
        &self.sections
    }
}

/// What is wrong with a template (`K`: the daily or the weekly kinds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError<K> {
    EmptyTitle,
    TitleTooLong,
    EmptyHeading(K),
    HeadingTooLong(K),
    /// A title or heading with a line break.
    LineBreak,
    /// Two sections with the same heading, in template order.
    DuplicateHeading {
        heading: String,
        first: K,
        second: K,
    },
}

/// Trimmed text without the `#`s a user may type in front (「## 待ち」 is
/// 「待ち」; 「#1 進捗」 stays as it is).
fn clean_text(text: &str) -> String {
    let t = text.trim();
    let rest = t.trim_start_matches('#');
    if rest.len() < t.len() && (rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        rest.trim().to_owned()
    } else {
        t.to_owned()
    }
}

fn has_line_break(text: &str) -> bool {
    text.contains(['\n', '\r'])
}

/// Every kind once: repeated kinds are dropped (the first stays), missing
/// ones are added with their defaults before 所感・連絡事項, and
/// 所感・連絡事項 is moved last and always shown.
fn normalize<S: TemplateSection>(given: Vec<S>) -> Vec<S> {
    let note_kind = S::Kind::NOTE;
    let mut sections: Vec<S> = Vec::with_capacity(S::Kind::ALL.len());
    let mut note: Option<S> = None;
    for s in given {
        if s.kind() == note_kind {
            note.get_or_insert(s);
        } else if !sections.iter().any(|x| x.kind() == s.kind()) {
            sections.push(s);
        }
    }
    for &kind in S::Kind::ALL {
        if kind != note_kind && !sections.iter().any(|s| s.kind() == kind) {
            sections.push(S::default_for(kind));
        }
    }
    let mut note = note.unwrap_or_else(|| S::default_for(note_kind));
    note.set_enabled(true);
    sections.push(note);
    sections
}

/// The normalized title and sections with the text cleaned up (trimmed,
/// leading `#`s removed), or what is wrong with them.
fn check<S: TemplateSection>(
    title: String,
    sections: Vec<S>,
) -> Result<(String, Vec<S>), TemplateError<S::Kind>> {
    let mut sections = normalize(sections);
    let title = clean_text(&title);
    if title.is_empty() {
        return Err(TemplateError::EmptyTitle);
    }
    if has_line_break(&title) {
        return Err(TemplateError::LineBreak);
    }
    if title.chars().count() > TITLE_MAX {
        return Err(TemplateError::TitleTooLong);
    }
    for i in 0..sections.len() {
        let heading = clean_text(sections[i].heading());
        let kind = sections[i].kind();
        if heading.is_empty() {
            return Err(TemplateError::EmptyHeading(kind));
        }
        if has_line_break(&heading) {
            return Err(TemplateError::LineBreak);
        }
        if heading.chars().count() > HEADING_MAX {
            return Err(TemplateError::HeadingTooLong(kind));
        }
        if let Some(first) = sections[..i].iter().find(|s| s.heading() == heading) {
            return Err(TemplateError::DuplicateHeading {
                heading,
                first: first.kind(),
                second: kind,
            });
        }
        sections[i].set_heading(heading);
    }
    Ok((title, sections))
}

impl ReportTemplate {
    /// Every kind once: repeated kinds are dropped (the first stays), missing
    /// ones are added with their defaults before 所感・連絡事項, and
    /// 所感・連絡事項 is moved last and always shown.
    pub fn normalized(self) -> ReportTemplate {
        ReportTemplate {
            title: self.title,
            sections: normalize(self.sections),
        }
    }

    /// The normalized template with its title and headings cleaned up
    /// (trimmed, leading `#`s removed), or what is wrong with it.
    pub fn checked(self) -> Result<ReportTemplate, TemplateError<SectionKind>> {
        let (title, sections) = check(self.title, self.sections)?;
        Ok(ReportTemplate { title, sections })
    }

    /// The section of that kind (there is always one once normalized).
    pub fn section(&self, kind: SectionKind) -> Option<&Section> {
        self.sections.iter().find(|s| s.kind() == kind)
    }

    /// The title with the date and weekday filled in (without `# `).
    pub fn fill_title(&self, date: &str, weekday: &str) -> String {
        self.title
            .replace(DATE_PLACEHOLDER, date)
            .replace(WEEKDAY_PLACEHOLDER, weekday)
    }
}

// ---------------------------------------------------------------------------
// 週報 (R-6, v0.3.0)

/// The six sections of the weekly report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum WeeklySectionKind {
    /// 今週の作業
    Work,
    /// 今週完了・中止
    Closed,
    /// 待ち
    Waiting,
    /// 来週の予定
    NextWeek,
    /// 日限注意
    Attention,
    /// 所感・連絡事項 (always shown, always last)
    Note,
}

impl WeeklySectionKind {
    /// In the default order.
    pub const ALL: [WeeklySectionKind; 6] = [
        WeeklySectionKind::Work,
        WeeklySectionKind::Closed,
        WeeklySectionKind::Waiting,
        WeeklySectionKind::NextWeek,
        WeeklySectionKind::Attention,
        WeeklySectionKind::Note,
    ];

    pub fn default_heading(self) -> &'static str {
        match self {
            WeeklySectionKind::Work => "今週の作業",
            WeeklySectionKind::Closed => "今週完了・中止",
            WeeklySectionKind::Waiting => "待ち",
            WeeklySectionKind::NextWeek => "来週の予定",
            WeeklySectionKind::Attention => "日限注意",
            WeeklySectionKind::Note => "所感・連絡事項",
        }
    }
}

impl Kind for WeeklySectionKind {
    const ALL: &'static [WeeklySectionKind] = &WeeklySectionKind::ALL;
    const NOTE: WeeklySectionKind = WeeklySectionKind::Note;

    fn default_heading(self) -> &'static str {
        WeeklySectionKind::default_heading(self)
    }
}

/// 今週の作業.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct WeeklyWorkOptions {
    /// 「### プロジェクト名」 above each project's TODOs. Off: each line
    /// starts with 「プロジェクト名 / 」.
    pub project_headings: bool,
    /// 「(作業中 …)」
    pub status: bool,
    /// 「… / 日限 11/20」 (open TODOs only).
    pub due_date: bool,
    /// 「… / 作業日 11/17・11/19」
    pub work_days: bool,
    /// 「3.5h」 after each TODO (TODOs with a timed log only).
    pub work_time: bool,
    /// 「作業時間の合計 8.0h」 at the top (when some log is timed).
    pub total_time: bool,
    /// The logs' text under each TODO, by day (off by default).
    pub log_body: bool,
}

/// 今週完了・中止.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct WeeklyClosedOptions {
    /// 「(11/19)」: the day it was closed.
    pub closed_date: bool,
    /// 「(理由:…)」 of 中止.
    pub close_reason: bool,
}

/// 来週の予定.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct NextWeekOptions {
    /// 「(予定 11/20〜11/27)」 of 未着手.
    pub planned_period: bool,
    /// 「日限 11/19」.
    pub due_date: bool,
}

/// The kind of a weekly section and its options (`"kind"` and `"options"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", content = "options", rename_all = "snake_case")]
pub enum WeeklySectionBody {
    Work(WeeklyWorkOptions),
    Closed(WeeklyClosedOptions),
    Waiting(WaitingOptions),
    NextWeek(NextWeekOptions),
    Attention(AttentionOptions),
    Note(NoteOptions),
}

impl WeeklySectionBody {
    pub fn kind(&self) -> WeeklySectionKind {
        match self {
            WeeklySectionBody::Work(_) => WeeklySectionKind::Work,
            WeeklySectionBody::Closed(_) => WeeklySectionKind::Closed,
            WeeklySectionBody::Waiting(_) => WeeklySectionKind::Waiting,
            WeeklySectionBody::NextWeek(_) => WeeklySectionKind::NextWeek,
            WeeklySectionBody::Attention(_) => WeeklySectionKind::Attention,
            WeeklySectionBody::Note(_) => WeeklySectionKind::Note,
        }
    }

    /// Every item on but the logs' text (and 要確認・着手遅れ, as in the daily report).
    pub fn default_for(kind: WeeklySectionKind) -> WeeklySectionBody {
        match kind {
            WeeklySectionKind::Work => WeeklySectionBody::Work(WeeklyWorkOptions {
                project_headings: true,
                status: true,
                due_date: true,
                work_days: true,
                work_time: true,
                total_time: true,
                log_body: false,
            }),
            WeeklySectionKind::Closed => WeeklySectionBody::Closed(WeeklyClosedOptions {
                closed_date: true,
                close_reason: true,
            }),
            WeeklySectionKind::Waiting => WeeklySectionBody::Waiting(default_waiting()),
            WeeklySectionKind::NextWeek => WeeklySectionBody::NextWeek(NextWeekOptions {
                planned_period: true,
                due_date: true,
            }),
            WeeklySectionKind::Attention => WeeklySectionBody::Attention(default_attention()),
            WeeklySectionKind::Note => WeeklySectionBody::Note(NoteOptions {}),
        }
    }
}

/// One section of the weekly report: `{ kind, heading, enabled, options }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct WeeklySection {
    /// Text only (`## ` is added by the app).
    pub heading: String,
    /// Shown in the report. Always true for 所感・連絡事項.
    pub enabled: bool,
    #[serde(flatten)]
    pub body: WeeklySectionBody,
}

impl WeeklySection {
    pub fn kind(&self) -> WeeklySectionKind {
        self.body.kind()
    }

    pub fn default_for(kind: WeeklySectionKind) -> WeeklySection {
        WeeklySection {
            heading: kind.default_heading().to_owned(),
            enabled: true,
            body: WeeklySectionBody::default_for(kind),
        }
    }
}

impl TemplateSection for WeeklySection {
    type Kind = WeeklySectionKind;

    fn kind(&self) -> WeeklySectionKind {
        WeeklySection::kind(self)
    }

    fn default_for(kind: WeeklySectionKind) -> WeeklySection {
        WeeklySection::default_for(kind)
    }

    fn heading(&self) -> &str {
        &self.heading
    }

    fn set_heading(&mut self, heading: String) {
        self.heading = heading;
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct WeeklyReportTemplate {
    /// Text only (`# ` is added by the app); `{開始日}` and `{終了日}` are filled in.
    pub title: String,
    /// Every kind once, 所感・連絡事項 last (see [`WeeklyReportTemplate::normalized`]).
    pub sections: Vec<WeeklySection>,
}

impl Default for WeeklyReportTemplate {
    fn default() -> Self {
        WeeklyReportTemplate {
            title: DEFAULT_WEEKLY_TITLE.to_owned(),
            sections: WeeklySectionKind::ALL
                .into_iter()
                .map(WeeklySection::default_for)
                .collect(),
        }
    }
}

impl Template for WeeklyReportTemplate {
    type Section = WeeklySection;

    fn sections(&self) -> &[WeeklySection] {
        &self.sections
    }
}

impl WeeklyReportTemplate {
    /// As [`ReportTemplate::normalized`].
    pub fn normalized(self) -> WeeklyReportTemplate {
        WeeklyReportTemplate {
            title: self.title,
            sections: normalize(self.sections),
        }
    }

    /// As [`ReportTemplate::checked`] (the same rules).
    pub fn checked(self) -> Result<WeeklyReportTemplate, TemplateError<WeeklySectionKind>> {
        let (title, sections) = check(self.title, self.sections)?;
        Ok(WeeklyReportTemplate { title, sections })
    }

    /// The section of that kind (there is always one once normalized).
    pub fn section(&self, kind: WeeklySectionKind) -> Option<&WeeklySection> {
        self.sections.iter().find(|s| s.kind() == kind)
    }

    /// The title with the week's Monday and Sunday filled in (without `# `).
    pub fn fill_title(&self, start: &str, end: &str) -> String {
        self.title
            .replace(START_PLACEHOLDER, start)
            .replace(END_PLACEHOLDER, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(t: &ReportTemplate) -> Vec<SectionKind> {
        t.sections.iter().map(Section::kind).collect()
    }

    fn heading(t: &ReportTemplate, kind: SectionKind) -> &str {
        &t.section(kind).unwrap().heading
    }

    #[test]
    fn default_is_the_mvp_report() {
        let t = ReportTemplate::default();
        assert_eq!(t.title, "日報 {日付}({曜日})");
        assert_eq!(kinds(&t), SectionKind::ALL);
        assert!(t.sections.iter().all(|s| s.enabled));
        assert_eq!(t.note_heading(), "所感・連絡事項");
        assert_eq!(t.clone().checked(), Ok(t));
    }

    #[test]
    fn json_shape() {
        let json = serde_json::to_value(ReportTemplate::default()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "title": "日報 {日付}({曜日})",
                "sections": [
                    { "heading": "本日の作業", "enabled": true, "kind": "work",
                      "options": { "project_headings": true, "status": true, "due_date": true, "work_time": true } },
                    { "heading": "本日完了・中止", "enabled": true, "kind": "closed",
                      "options": { "close_reason": true } },
                    { "heading": "待ち", "enabled": true, "kind": "waiting",
                      "options": { "waiting_on": true, "waiting_days": true, "follow_up_date": true } },
                    { "heading": "明日の予定", "enabled": true, "kind": "tomorrow",
                      "options": { "planned_period": true } },
                    { "heading": "日限注意", "enabled": true, "kind": "attention",
                      "options": { "days": true, "waiting_mark": true, "follow_up_and_late_start": false } },
                    { "heading": "所感・連絡事項", "enabled": true, "kind": "note", "options": {} },
                ]
            })
        );
        let back: ReportTemplate = serde_json::from_value(json).unwrap();
        assert_eq!(back, ReportTemplate::default());
    }

    #[test]
    fn normalizing_fills_in_drops_repeats_and_puts_the_note_last() {
        let mut waiting = Section::default_for(SectionKind::Waiting);
        waiting.heading = "待ち状況".into();
        let mut note = Section::default_for(SectionKind::Note);
        note.heading = "連絡".into();
        note.enabled = false;
        let mut attention = Section::default_for(SectionKind::Attention);
        attention.enabled = false;
        let t = ReportTemplate {
            title: "T".into(),
            sections: vec![
                note,
                attention,
                waiting,
                // A repeat: the first one stays.
                Section::default_for(SectionKind::Waiting),
                Section::default_for(SectionKind::Note),
            ],
        }
        .normalized();
        assert_eq!(
            kinds(&t),
            [
                SectionKind::Attention,
                SectionKind::Waiting,
                SectionKind::Work,
                SectionKind::Closed,
                SectionKind::Tomorrow,
                SectionKind::Note,
            ]
        );
        assert_eq!(heading(&t, SectionKind::Waiting), "待ち状況");
        assert!(!t.section(SectionKind::Attention).unwrap().enabled);
        // The note keeps its heading and is always shown.
        assert_eq!(t.note_heading(), "連絡");
        assert!(t.section(SectionKind::Note).unwrap().enabled);

        let empty = ReportTemplate {
            title: "T".into(),
            sections: vec![],
        }
        .normalized();
        assert_eq!(kinds(&empty), SectionKind::ALL);
    }

    fn with_heading(kind: SectionKind, text: &str) -> ReportTemplate {
        let mut t = ReportTemplate::default();
        t.sections
            .iter_mut()
            .find(|s| s.kind() == kind)
            .unwrap()
            .heading = text.into();
        t
    }

    #[test]
    fn checking_cleans_the_text() {
        let mut t = with_heading(SectionKind::Waiting, "  ## 待ち状況 ");
        t.title = "# 日報 {日付}".into();
        let t = t.checked().unwrap();
        assert_eq!(t.title, "日報 {日付}");
        assert_eq!(heading(&t, SectionKind::Waiting), "待ち状況");
        // A # that is part of the text stays.
        let t = with_heading(SectionKind::Work, "#1 作業")
            .checked()
            .unwrap();
        assert_eq!(heading(&t, SectionKind::Work), "#1 作業");
    }

    #[test]
    fn checking_finds_what_is_wrong() {
        let titled = |title: &str| ReportTemplate {
            title: title.into(),
            ..ReportTemplate::default()
        };
        assert_eq!(titled(" ").checked(), Err(TemplateError::EmptyTitle));
        assert_eq!(titled("#").checked(), Err(TemplateError::EmptyTitle));
        assert_eq!(
            titled(&"題".repeat(61)).checked(),
            Err(TemplateError::TitleTooLong)
        );
        assert!(titled(&"題".repeat(60)).checked().is_ok());
        assert_eq!(
            titled("日報\n{日付}").checked(),
            Err(TemplateError::LineBreak)
        );

        assert_eq!(
            with_heading(SectionKind::Tomorrow, "  ").checked(),
            Err(TemplateError::EmptyHeading(SectionKind::Tomorrow))
        );
        assert_eq!(
            with_heading(SectionKind::Note, &"見".repeat(41)).checked(),
            Err(TemplateError::HeadingTooLong(SectionKind::Note))
        );
        assert!(
            with_heading(SectionKind::Note, &"見".repeat(40))
                .checked()
                .is_ok()
        );
        assert_eq!(
            with_heading(SectionKind::Work, "a\rb").checked(),
            Err(TemplateError::LineBreak)
        );
        // Same heading as another section (after cleaning), shown or not.
        let mut t = with_heading(SectionKind::Note, "## 待ち");
        t.sections[2].enabled = false;
        assert_eq!(
            t.checked(),
            Err(TemplateError::DuplicateHeading {
                heading: "待ち".into(),
                first: SectionKind::Waiting,
                second: SectionKind::Note,
            })
        );
    }

    #[test]
    fn title_placeholders_are_filled_in() {
        let t = ReportTemplate {
            title: "{日付}({曜日}) 業務報告 {日付}".into(),
            ..ReportTemplate::default()
        };
        assert_eq!(
            t.fill_title("2026-11-19", "木"),
            "2026-11-19(木) 業務報告 2026-11-19"
        );
        assert_eq!(
            ReportTemplate::default().fill_title("2026-11-19", "木"),
            "日報 2026-11-19(木)"
        );
    }

    #[test]
    fn weekly_default_and_json_shape() {
        let t = WeeklyReportTemplate::default();
        assert_eq!(t.title, "週報 {開始日}〜{終了日}");
        assert_eq!(
            t.sections
                .iter()
                .map(WeeklySection::kind)
                .collect::<Vec<_>>(),
            WeeklySectionKind::ALL
        );
        assert_eq!(t.note_heading(), "所感・連絡事項");
        assert_eq!(t.clone().checked(), Ok(t.clone()));
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "title": "週報 {開始日}〜{終了日}",
                "sections": [
                    { "heading": "今週の作業", "enabled": true, "kind": "work",
                      "options": { "project_headings": true, "status": true, "due_date": true,
                                   "work_days": true, "work_time": true, "total_time": true,
                                   "log_body": false } },
                    { "heading": "今週完了・中止", "enabled": true, "kind": "closed",
                      "options": { "closed_date": true, "close_reason": true } },
                    { "heading": "待ち", "enabled": true, "kind": "waiting",
                      "options": { "waiting_on": true, "waiting_days": true, "follow_up_date": true } },
                    { "heading": "来週の予定", "enabled": true, "kind": "next_week",
                      "options": { "planned_period": true, "due_date": true } },
                    { "heading": "日限注意", "enabled": true, "kind": "attention",
                      "options": { "days": true, "waiting_mark": true, "follow_up_and_late_start": false } },
                    { "heading": "所感・連絡事項", "enabled": true, "kind": "note", "options": {} },
                ]
            })
        );
        let back: WeeklyReportTemplate = serde_json::from_value(json).unwrap();
        assert_eq!(back, t);
        // The daily kinds are not the weekly ones.
        assert!(
            serde_json::from_str::<WeeklySection>(
                r#"{"heading":"明日","enabled":true,"kind":"tomorrow","options":{"planned_period":true}}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Section>(
                r#"{"heading":"来週","enabled":true,"kind":"next_week","options":{"planned_period":true,"due_date":true}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn weekly_templates_follow_the_same_rules() {
        let mut next = WeeklySection::default_for(WeeklySectionKind::NextWeek);
        next.heading = " ## 来週 ".into();
        let mut note = WeeklySection::default_for(WeeklySectionKind::Note);
        note.heading = "連絡".into();
        note.enabled = false;
        let t = WeeklyReportTemplate {
            title: " # 週報 {開始日} ".into(),
            sections: vec![
                note,
                next,
                WeeklySection::default_for(WeeklySectionKind::NextWeek),
            ],
        }
        .checked()
        .unwrap();
        assert_eq!(t.title, "週報 {開始日}");
        assert_eq!(
            t.sections
                .iter()
                .map(WeeklySection::kind)
                .collect::<Vec<_>>(),
            [
                WeeklySectionKind::NextWeek,
                WeeklySectionKind::Work,
                WeeklySectionKind::Closed,
                WeeklySectionKind::Waiting,
                WeeklySectionKind::Attention,
                WeeklySectionKind::Note,
            ]
        );
        assert_eq!(
            t.section(WeeklySectionKind::NextWeek).unwrap().heading,
            "来週"
        );
        assert_eq!(t.note_heading(), "連絡");
        assert!(t.section(WeeklySectionKind::Note).unwrap().enabled);

        let with = |kind: WeeklySectionKind, text: &str| {
            let mut t = WeeklyReportTemplate::default();
            t.sections
                .iter_mut()
                .find(|s| s.kind() == kind)
                .unwrap()
                .heading = text.into();
            t
        };
        assert_eq!(
            with(WeeklySectionKind::NextWeek, " ").checked(),
            Err(TemplateError::EmptyHeading(WeeklySectionKind::NextWeek))
        );
        assert_eq!(
            with(WeeklySectionKind::Work, &"見".repeat(41)).checked(),
            Err(TemplateError::HeadingTooLong(WeeklySectionKind::Work))
        );
        assert_eq!(
            with(WeeklySectionKind::Note, "## 待ち").checked(),
            Err(TemplateError::DuplicateHeading {
                heading: "待ち".into(),
                first: WeeklySectionKind::Waiting,
                second: WeeklySectionKind::Note,
            })
        );
        let titled = |title: &str| WeeklyReportTemplate {
            title: title.into(),
            ..WeeklyReportTemplate::default()
        };
        assert_eq!(titled("#").checked(), Err(TemplateError::EmptyTitle));
        assert_eq!(
            titled(&"題".repeat(61)).checked(),
            Err(TemplateError::TitleTooLong)
        );
        assert_eq!(titled("週\n報").checked(), Err(TemplateError::LineBreak));
    }

    #[test]
    fn weekly_title_placeholders_are_filled_in() {
        assert_eq!(
            WeeklyReportTemplate::default().fill_title("2026-11-16", "2026-11-22"),
            "週報 2026-11-16〜2026-11-22"
        );
        let t = WeeklyReportTemplate {
            title: "{終了日}まで({開始日}から) {日付}".into(),
            ..WeeklyReportTemplate::default()
        };
        // The daily report's placeholders stay as written.
        assert_eq!(
            t.fill_title("2026-11-16", "2026-11-22"),
            "2026-11-22まで(2026-11-16から) {日付}"
        );
    }
}
