// The report template cards' edits (日報のテンプレート R-5, 週報のテンプレート
// R-6). Rust checks and normalizes what is saved; these only make the change
// the user asked for. The daily and the weekly template have the same shape
// (a title and sections) with their own kinds of section and items.
import {
  DEFAULT_REPORT_TEMPLATE,
  DEFAULT_WEEKLY_REPORT_TEMPLATE,
  type ReportTemplate,
  type Section,
  type Settings,
  type SettingsInput,
  type WeeklyReportTemplate,
  type WeeklySection,
} from "../../lib/bindings";

export type SectionKind = Section["kind"];
export type WeeklySectionKind = WeeklySection["kind"];

/** The options of one kind of section. */
export type OptionsOf<K extends SectionKind> = Extract<Section, { kind: K }>["options"];
export type WeeklyOptionsOf<K extends WeeklySectionKind> = Extract<WeeklySection, { kind: K }>["options"];

/** The daily or the weekly template. */
export type AnyTemplate = ReportTemplate | WeeklyReportTemplate;
export type AnySection = Section | WeeklySection;

/** Which template a card edits. */
export type TemplateKind = "daily" | "weekly";

/** The template 「初期値に戻す」 saves (made by Rust, exported in the bindings). */
export function defaultReportTemplate(): ReportTemplate {
  return structuredClone(DEFAULT_REPORT_TEMPLATE) as unknown as ReportTemplate;
}

export function defaultWeeklyReportTemplate(): WeeklyReportTemplate {
  return structuredClone(DEFAULT_WEEKLY_REPORT_TEMPLATE) as unknown as WeeklyReportTemplate;
}

/** The fixed name of a daily section: its default heading (「本日の作業」). */
export function sectionName(kind: SectionKind): string {
  return DEFAULT_REPORT_TEMPLATE.sections.find((s) => s.kind === kind)?.heading ?? kind;
}

/** The fixed name of a weekly section (「今週の作業」). */
export function weeklySectionName(kind: WeeklySectionKind): string {
  return DEFAULT_WEEKLY_REPORT_TEMPLATE.sections.find((s) => s.kind === kind)?.heading ?? kind;
}

export interface OptionItem {
  key: string;
  label: string;
  description?: string;
}

const PROJECT_HEADINGS: OptionItem & { key: "project_headings" } = {
  key: "project_headings",
  label: "プロジェクトの見出し",
  description: "出さないときは、行の頭にプロジェクト名を付けます。",
};

const WAITING_ITEMS: Array<OptionItem & { key: keyof OptionsOf<"waiting"> }> = [
  { key: "waiting_on", label: "待っている相手" },
  { key: "waiting_days", label: "待ちの日数" },
  { key: "follow_up_date", label: "確認日" },
];

const ATTENTION_ITEMS: Array<OptionItem & { key: keyof OptionsOf<"attention"> }> = [
  { key: "days", label: "日数", description: "「あと1日」「2日超過」のような日数です。" },
  { key: "waiting_mark", label: "待ちの印" },
  { key: "follow_up_and_late_start", label: "要確認と着手遅れも出す" },
];

/** The items each daily section can carry, in the order of the design's table. */
export const OPTION_ITEMS: { [K in SectionKind]: Array<OptionItem & { key: keyof OptionsOf<K> & string }> } = {
  work: [
    PROJECT_HEADINGS,
    { key: "status", label: "ステータス" },
    { key: "due_date", label: "日限" },
    { key: "work_time", label: "作業時間" },
  ],
  closed: [{ key: "close_reason", label: "中止の理由" }],
  waiting: WAITING_ITEMS,
  tomorrow: [{ key: "planned_period", label: "予定期間" }],
  attention: ATTENTION_ITEMS,
  note: [],
};

/** The items each weekly section can carry, in the order of the design's table. */
export const WEEKLY_OPTION_ITEMS: {
  [K in WeeklySectionKind]: Array<OptionItem & { key: keyof WeeklyOptionsOf<K> & string }>;
} = {
  work: [
    PROJECT_HEADINGS,
    { key: "status", label: "ステータス" },
    { key: "due_date", label: "日限" },
    { key: "work_days", label: "作業した日" },
    { key: "work_time", label: "TODOごとの作業時間" },
    { key: "total_time", label: "週の作業時間の合計" },
    {
      key: "log_body",
      label: "作業ログの本文",
      description: "作業した日ごとに、TODOの下に字下げして入れます。",
    },
  ],
  closed: [
    { key: "closed_date", label: "完了・中止にした日" },
    { key: "close_reason", label: "中止の理由" },
  ],
  waiting: WAITING_ITEMS,
  next_week: [
    { key: "planned_period", label: "予定期間" },
    { key: "due_date", label: "日限" },
  ],
  attention: ATTENTION_ITEMS,
  note: [],
};

/** What differs between the daily and the weekly card. */
export interface TemplateSpec {
  /** 「日報」「週報」 */
  name: string;
  /** The title field's help. */
  titleDescription: string;
  sectionName: (kind: string) => string;
  optionItems: (kind: string) => OptionItem[];
  defaultTemplate: () => AnyTemplate;
}

export const TEMPLATE_SPECS: Record<TemplateKind, TemplateSpec> = {
  daily: {
    name: "日報",
    titleDescription: "「{日付}」と「{曜日}」は、その日の日付と曜日に置き換わります。先頭の「# 」はアプリが付けます。",
    sectionName: (kind) => sectionName(kind as SectionKind),
    optionItems: (kind) => OPTION_ITEMS[kind as SectionKind] ?? [],
    defaultTemplate: defaultReportTemplate,
  },
  weekly: {
    name: "週報",
    titleDescription:
      "「{開始日}」と「{終了日}」は、その週の月曜と日曜の日付に置き換わります。先頭の「# 」はアプリが付けます。",
    sectionName: (kind) => weeklySectionName(kind as WeeklySectionKind),
    optionItems: (kind) => WEEKLY_OPTION_ITEMS[kind as WeeklySectionKind] ?? [],
    defaultTemplate: defaultWeeklyReportTemplate,
  },
};

/** The template of that kind as saved. */
export function templateOf(kind: TemplateKind, settings: Settings): AnyTemplate {
  return kind === "weekly" ? settings.weekly_report_template : settings.report_template;
}

/** The settings to send to save `template` as the template of that kind. */
export function templateEdits(kind: TemplateKind, template: AnyTemplate): Partial<SettingsInput> {
  return kind === "weekly"
    ? { weekly_report_template: template as WeeklyReportTemplate }
    : { report_template: template as ReportTemplate };
}

function mapSection<T extends AnyTemplate>(t: T, kind: string, f: (s: AnySection) => AnySection): T {
  const sections = (t.sections as AnySection[]).map((s) => (s.kind === kind ? f(s) : s));
  return { ...t, sections } as T;
}

export function setTitle<T extends AnyTemplate>(t: T, title: string): T {
  return { ...t, title };
}

export function setHeading<T extends AnyTemplate>(t: T, kind: string, heading: string): T {
  return mapSection(t, kind, (s) => ({ ...s, heading }));
}

export function setEnabled<T extends AnyTemplate>(t: T, kind: string, enabled: boolean): T {
  return mapSection(t, kind, (s) => ({ ...s, enabled }));
}

export function setOption<T extends AnyTemplate>(t: T, kind: string, key: string, value: boolean): T {
  return mapSection(t, kind, (s) => ({ ...s, options: { ...s.options, [key]: value } }) as AnySection);
}

/** The value of one item (false when the section has no such item). */
export function optionValue(s: AnySection, key: string): boolean {
  return (s.options as Record<string, boolean | undefined>)[key] === true;
}

/**
 * Moves a section one place up (-1) or down (+1) among the sections that can
 * move. 所感・連絡事項 stays last and never moves.
 */
export function moveSection<T extends AnyTemplate>(t: T, kind: string, delta: -1 | 1): T {
  const all = t.sections as AnySection[];
  const movable = all.filter((s) => s.kind !== "note");
  const from = movable.findIndex((s) => s.kind === kind);
  const to = from + delta;
  if (from < 0 || to < 0 || to >= movable.length) return t;
  const next = [...movable];
  [next[from], next[to]] = [next[to], next[from]];
  return { ...t, sections: [...next, ...all.filter((s) => s.kind === "note")] } as T;
}
