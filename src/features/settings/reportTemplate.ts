// The report template card's edits (日報のテンプレート, R-5). Rust checks and
// normalizes what is saved; these only make the change the user asked for.
import { DEFAULT_REPORT_TEMPLATE, type ReportTemplate, type Section } from "../../lib/bindings";

export type SectionKind = Section["kind"];

/** The options of one kind of section. */
export type OptionsOf<K extends SectionKind> = Extract<Section, { kind: K }>["options"];

/** The template 「初期値に戻す」 saves (made by Rust, exported in the bindings). */
export function defaultReportTemplate(): ReportTemplate {
  return structuredClone(DEFAULT_REPORT_TEMPLATE) as unknown as ReportTemplate;
}

/** The fixed name of a section: its default heading (「本日の作業」). */
export function sectionName(kind: SectionKind): string {
  return DEFAULT_REPORT_TEMPLATE.sections.find((s) => s.kind === kind)?.heading ?? kind;
}

export interface OptionItem {
  key: string;
  label: string;
  description?: string;
}

/** The items each section can carry, in the order of the design's table. */
export const OPTION_ITEMS: { [K in SectionKind]: Array<OptionItem & { key: keyof OptionsOf<K> & string }> } = {
  work: [
    {
      key: "project_headings",
      label: "プロジェクトの見出し",
      description: "出さないときは、行の頭にプロジェクト名を付けます。",
    },
    { key: "status", label: "ステータス" },
    { key: "due_date", label: "日限" },
    { key: "work_time", label: "作業時間" },
  ],
  closed: [{ key: "close_reason", label: "中止の理由" }],
  waiting: [
    { key: "waiting_on", label: "待っている相手" },
    { key: "waiting_days", label: "待ちの日数" },
    { key: "follow_up_date", label: "確認日" },
  ],
  tomorrow: [{ key: "planned_period", label: "予定期間" }],
  attention: [
    { key: "days", label: "日数", description: "「あと1日」「2日超過」のような日数です。" },
    { key: "waiting_mark", label: "待ちの印" },
    { key: "follow_up_and_late_start", label: "要確認と着手遅れも出す" },
  ],
  note: [],
};

function mapSection(t: ReportTemplate, kind: SectionKind, f: (s: Section) => Section): ReportTemplate {
  return { ...t, sections: t.sections.map((s) => (s.kind === kind ? f(s) : s)) };
}

export function setTitle(t: ReportTemplate, title: string): ReportTemplate {
  return { ...t, title };
}

export function setHeading(t: ReportTemplate, kind: SectionKind, heading: string): ReportTemplate {
  return mapSection(t, kind, (s) => ({ ...s, heading }));
}

export function setEnabled(t: ReportTemplate, kind: SectionKind, enabled: boolean): ReportTemplate {
  return mapSection(t, kind, (s) => ({ ...s, enabled }));
}

export function setOption(t: ReportTemplate, kind: SectionKind, key: string, value: boolean): ReportTemplate {
  return mapSection(t, kind, (s) => ({ ...s, options: { ...s.options, [key]: value } }) as Section);
}

/** The value of one item (false when the section has no such item). */
export function optionValue(s: Section, key: string): boolean {
  return (s.options as Record<string, boolean | undefined>)[key] === true;
}

/**
 * Moves a section one place up (-1) or down (+1) among the sections that can
 * move. 所感・連絡事項 stays last and never moves.
 */
export function moveSection(t: ReportTemplate, kind: SectionKind, delta: -1 | 1): ReportTemplate {
  const movable = t.sections.filter((s) => s.kind !== "note");
  const from = movable.findIndex((s) => s.kind === kind);
  const to = from + delta;
  if (from < 0 || to < 0 || to >= movable.length) return t;
  const next = [...movable];
  [next[from], next[to]] = [next[to], next[from]];
  return { ...t, sections: [...next, ...t.sections.filter((s) => s.kind === "note")] };
}
