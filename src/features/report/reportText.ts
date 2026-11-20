// The report screen's words for the two kinds it shows (日報 and 週報, R-6),
// and the keys it opens them by. 月報 is not made.
import type { ReportKey } from "../../lib/bindings";
import { addDaysYmd, monthDay, weekStartYmd } from "../../lib/date";

export type ShownKind = "daily" | "weekly";

export const KIND_TABS: Array<{ id: ShownKind; label: string }> = [
  { id: "daily", label: "日報" },
  { id: "weekly", label: "週報" },
];

export interface KindText {
  /** 「日報」「週報」 */
  name: string;
  /** Days the previous and next buttons move. */
  step: number;
  prev: string;
  next: string;
  /** The button back to the current day or week. */
  current: string;
  /** The date field's label. */
  dateLabel: string;
}

export const KIND_TEXT: Record<ShownKind, KindText> = {
  daily: {
    name: "日報",
    step: 1,
    prev: "前の日へ",
    next: "次の日へ",
    current: "今日",
    dateLabel: "対象日",
  },
  weekly: {
    name: "週報",
    step: 7,
    prev: "前の週へ",
    next: "次の週へ",
    current: "今週",
    dateLabel: "対象の週",
  },
};

/** The report shown for `date`: a weekly report is keyed by its Monday. */
export function reportKey(kind: ShownKind, date: string): ReportKey {
  return { kind, date: kind === "weekly" ? weekStartYmd(date) : date };
}

/** 「11/16(月)〜11/22(日)」: the week `date` falls in. */
export function weekRangeText(date: string): string {
  const start = weekStartYmd(date);
  return `${monthDay(start)}(月)〜${monthDay(addDaysYmd(start, 6))}(日)`;
}

export function copyButtonText(kind: ShownKind): string {
  return `${KIND_TEXT[kind].name}をコピー`;
}

export function copiedText(kind: ShownKind): string {
  return `${KIND_TEXT[kind].name}をクリップボードにコピーしました。`;
}

export function savedFileText(kind: ShownKind, path: string): string {
  return `${KIND_TEXT[kind].name}を保存しました。保存先: ${path}`;
}
