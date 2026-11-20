// 繰り返しのTODO (T-12): the words for a repetition, and the first values of
// the 周期 dialog. The next round's 日限 comes from Rust (RepeatInfo.next_due);
// nothing here decides a date.
import type { DayOfWeek, RepeatInfo, RepeatRule } from "../../lib/bindings";
import { formatDate, monthDay } from "../../lib/date";

/** Read out with the repeat mark of the list and the Gantt chart. */
export const REPEAT_LABEL = "繰り返し";

export const WEEKDAYS: DayOfWeek[] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

export const WEEKDAY_LABEL: Record<DayOfWeek, string> = {
  mon: "月",
  tue: "火",
  wed: "水",
  thu: "木",
  fri: "金",
  sat: "土",
  sun: "日",
};

export type RepeatKind = RepeatRule["kind"];

/** The 周期 select, in the design's order. */
export const REPEAT_KINDS: { value: RepeatKind; label: string }[] = [
  { value: "daily", label: "毎日" },
  { value: "weekly", label: "毎週" },
  { value: "monthly_day", label: "毎月(日付)" },
  { value: "monthly_weekday", label: "毎月(第N◯曜日)" },
  { value: "monthly_last", label: "毎月(月末)" },
  { value: "yearly", label: "毎年" },
  { value: "every_days", label: "N日ごと" },
];

/** 第1〜第4 and the last week of a month. */
export const WEEK_OPTIONS: { value: number; label: string }[] = [
  { value: 1, label: "第1" },
  { value: 2, label: "第2" },
  { value: 3, label: "第3" },
  { value: 4, label: "第4" },
  { value: -1, label: "最終" },
];

function weekLabel(week: number): string {
  return WEEK_OPTIONS.find((w) => w.value === week)?.label ?? `第${week}`;
}

/** 「毎週 月・木」「毎月 第2火曜」「毎月 15日」「毎年 3/31」「10日ごと」. */
export function repeatRuleText(rule: RepeatRule): string {
  switch (rule.kind) {
    case "daily":
      return "毎日";
    case "weekly": {
      const days = WEEKDAYS.filter((d) => rule.weekdays.includes(d)).map((d) => WEEKDAY_LABEL[d]);
      return `毎週 ${days.join("・")}`;
    }
    case "monthly_day":
      return `毎月 ${rule.day}日`;
    case "monthly_weekday":
      return `毎月 ${weekLabel(rule.week)}${WEEKDAY_LABEL[rule.weekday]}曜`;
    case "monthly_last":
      return "毎月 月末";
    case "yearly":
      return `毎年 ${rule.month}/${rule.day}`;
    case "every_days":
      return `${rule.days}日ごと`;
  }
}

/** 「次の回の日限 11/30」, or that no round comes after this one. */
export function nextDueText(info: RepeatInfo): string {
  return info.next_due ? `次の回の日限 ${monthDay(info.next_due)}` : "次の回はありません(終わりの日を過ぎます)";
}

/** 「2027/03/31 まで」, or null without an end. */
export function untilText(info: RepeatInfo): string | null {
  return info.until_date ? `${formatDate(info.until_date)} まで` : null;
}

/** The tooltip of the repeat mark: 「繰り返し:毎週 月」. */
export function repeatMarkTitle(info: RepeatInfo): string {
  return `${REPEAT_LABEL}:${repeatRuleText(info.rule)}`;
}

/** Shown once a repeating TODO was closed and its next round made. */
export function nextRoundMadeText(nextDue: string): string {
  return `次の回(日限 ${monthDay(nextDue)})を作りました。`;
}

// ---- the dialog's first values ---------------------------------------------------

/** Days of 「N日ごと」 before one is entered. */
export const DEFAULT_EVERY_DAYS = 14;

function parts(ymd: string): { year: number; month: number; day: number; weekday: DayOfWeek } {
  const [year, month, day] = ymd.split("-").map(Number);
  // getDay(): 0 = Sunday.
  const weekday = WEEKDAYS[(new Date(year, month - 1, day).getDay() + 6) % 7];
  return { year, month, day, weekday };
}

/**
 * The rule of a kind as first shown, from the 日限 (design: 毎週ならその曜日、
 * 毎月ならその日付、毎年ならその月日). 第N◯曜日 takes the 日限's week; from the
 * 29th on it is the last one (week 5 is not offered).
 */
export function ruleFromDue(kind: RepeatKind, due: string): RepeatRule {
  const { month, day, weekday } = parts(due);
  switch (kind) {
    case "daily":
      return { kind };
    case "weekly":
      return { kind, weekdays: [weekday] };
    case "monthly_day":
      return { kind, day };
    case "monthly_weekday":
      return { kind, week: day > 28 ? -1 : Math.ceil(day / 7), weekday };
    case "monthly_last":
      return { kind };
    case "yearly":
      return { kind, month, day };
    case "every_days":
      return { kind, days: DEFAULT_EVERY_DAYS };
  }
}

/** A day toggled in the 毎週 days, kept in Monday-to-Sunday order. */
export function toggledWeekday(days: DayOfWeek[], day: DayOfWeek): DayOfWeek[] {
  const next = days.includes(day) ? days.filter((d) => d !== day) : [...days, day];
  return WEEKDAYS.filter((d) => next.includes(d));
}

/**
 * The days of 「N日ごと」 as typed. Anything that is not a whole number goes
 * as 0, which Rust refuses with its own message (the check stays in Rust).
 */
export function everyDaysOf(text: string): number {
  const n = Number(text.trim());
  return text.trim() !== "" && Number.isInteger(n) && n >= 0 && n <= 0xffffffff ? n : 0;
}

/** Shown under a 毎月(日付) day the shorter months do not have. */
export function monthEndNote(day: number): string | null {
  return day > 28 ? "その日がない月は、月末にします。" : null;
}

/** Shown under 毎年 2/29. */
export function leapDayNote(month: number, day: number): string | null {
  return month === 2 && day === 29 ? "うるう年でない年は、2/28にします。" : null;
}
