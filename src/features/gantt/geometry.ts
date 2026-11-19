// Gantt geometry: dates -> x, rows -> y, header labels, visible range.
// Pure functions (tested in geometry.test.ts). Sizes are the mock's
// (docs/ui-mock/README.md「ガントの寸法」) and the design's day widths.
import type { DaySpan } from "../../lib/bindings";

export type Scale = "day" | "week" | "month";

/** Width of one day per scale: 日 32px, 週 12px, 月 4px. */
export const DAY_WIDTH: Record<Scale, number> = { day: 32, week: 12, month: 4 };

export const LAYOUT = {
  nameWidth: 302,
  headerHeight: 53,
  headerTopRow: 24,
  projectRow: 32,
  todoRow: 44,
  plannedTop: 9,
  plannedHeight: 10,
  actualTop: 23,
  actualHeight: 12,
  diamond: 12,
  /** Rows drawn above and below the viewport while scrolling. */
  overscan: 6,
} as const;

const MS_PER_DAY = 86_400_000;

function utcDay(ymd: string): number {
  const [y, m, d] = ymd.split("-").map(Number);
  return Date.UTC(y, m - 1, d) / MS_PER_DAY;
}

function fromUtcDay(n: number): string {
  const d = new Date(n * MS_PER_DAY);
  const p = (v: number) => String(v).padStart(2, "0");
  return `${d.getUTCFullYear()}-${p(d.getUTCMonth() + 1)}-${p(d.getUTCDate())}`;
}

/** Calendar days from `start` to `date` (no time zone or DST involved). */
export function dayIndex(start: string, date: string): number {
  return utcDay(date) - utcDay(start);
}

export function addDays(ymd: string, days: number): string {
  return fromUtcDay(utcDay(ymd) + days);
}

/** 0 = Sunday ... 6 = Saturday. */
export function weekday(ymd: string): number {
  return new Date(utcDay(ymd) * MS_PER_DAY).getUTCDay();
}

export interface Range {
  start: string;
  /** Number of days shown. */
  days: number;
}

export function rangeEnd(r: Range): string {
  return addDays(r.start, r.days - 1);
}

/** Days that fit in the chart width (at least one week). */
export function visibleDays(chartWidth: number, scale: Scale): number {
  return Math.max(7, Math.floor(chartWidth / DAY_WIDTH[scale]));
}

/** Initial range: today about a third from the left (design). */
export function startAround(today: string, days: number): string {
  return addDays(today, -Math.floor(days / 3));
}

/** How far 「前の期間へ」/「次の期間へ」 move: half of what is shown. */
export function step(days: number): number {
  return Math.max(1, Math.floor(days / 2));
}

/** x and width of a span, clipped to the range; null when outside it. */
export function spanRect(r: Range, span: DaySpan, dayWidth: number): { x: number; width: number } | null {
  const from = Math.max(0, dayIndex(r.start, span.start));
  const to = Math.min(r.days - 1, dayIndex(r.start, span.end));
  if (to < from) return null;
  return { x: from * dayWidth, width: (to - from + 1) * dayWidth };
}

/** Center x of a day, or null when the day is outside the range. */
export function dayCenter(r: Range, date: string, dayWidth: number): number | null {
  const i = dayIndex(r.start, date);
  if (i < 0 || i >= r.days) return null;
  return i * dayWidth + dayWidth / 2;
}

/** Day indexes falling on Saturday or Sunday, merged into runs. */
export function weekendRuns(r: Range): Array<{ index: number; length: number }> {
  const runs: Array<{ index: number; length: number }> = [];
  for (let i = 0; i < r.days; i++) {
    const wd = weekday(addDays(r.start, i));
    if (wd !== 0 && wd !== 6) continue;
    const last = runs[runs.length - 1];
    if (last && last.index + last.length === i) last.length += 1;
    else runs.push({ index: i, length: 1 });
  }
  return runs;
}

export interface Segment {
  index: number;
  length: number;
  label: string;
  /** Highlighted (today's day number). */
  current?: boolean;
}

/**
 * Header rows. 日: months over day numbers. 週: months over the Monday of
 * each week. 月: years over months. The first segment of the top row, and
 * any January, carry the year (「2026年11月」, then 「12月」).
 */
export function headerRows(r: Range, scale: Scale, today: string): { top: Segment[]; bottom: Segment[] } {
  const months: Segment[] = [];
  const monthKeys: string[] = [];
  const years: Segment[] = [];
  for (let i = 0; i < r.days; i++) {
    const [y, m] = addDays(r.start, i).split("-").map(Number);
    const monthKey = `${y}-${m}`;
    if (monthKeys[monthKeys.length - 1] === monthKey) months[months.length - 1].length += 1;
    else {
      const label = months.length === 0 || m === 1 ? `${y}年${m}月` : `${m}月`;
      months.push({ index: i, length: 1, label });
      monthKeys.push(monthKey);
    }
    const lastY = years[years.length - 1];
    if (lastY && lastY.label === `${y}年`) lastY.length += 1;
    else years.push({ index: i, length: 1, label: `${y}年` });
  }
  const strip = (s: Segment[]) => s.map(({ index, length, label }) => ({ index, length, label }));

  if (scale === "day") {
    const days: Segment[] = [];
    for (let i = 0; i < r.days; i++) {
      const date = addDays(r.start, i);
      days.push({ index: i, length: 1, label: String(Number(date.slice(8))), current: date === today });
    }
    return { top: strip(months), bottom: days };
  }
  if (scale === "week") {
    const weeks: Segment[] = [];
    for (let i = 0; i < r.days; i++) {
      const date = addDays(r.start, i);
      if (weekday(date) !== 1 && i !== 0) continue;
      const last = weeks[weeks.length - 1];
      if (last) last.length = i - last.index;
      const [, m, d] = date.split("-").map(Number);
      weeks.push({ index: i, length: r.days - i, label: `${m}/${d}` });
    }
    // A partial first week before the first Monday gets no label.
    if (weeks.length > 1 && weekday(r.start) !== 1) weeks[0].label = "";
    return { top: strip(months), bottom: weeks };
  }
  return {
    top: years,
    bottom: strip(months).map((s) => ({ ...s, label: s.label.replace(/^\d+年/, "") })),
  };
}

// ---- rows ------------------------------------------------------------------------

export interface LaidOutRow<P, T> {
  kind: "project" | "todo";
  key: string;
  top: number;
  height: number;
  project: P;
  todo?: T;
}

/** Stacks project rows and their TODO rows; returns the rows and the total height. */
export function layoutRows<P, T>(
  groups: Array<{ key: string; project: P; todos: Array<{ key: string; todo: T }> }>,
): { rows: Array<LaidOutRow<P, T>>; height: number } {
  const rows: Array<LaidOutRow<P, T>> = [];
  let y = 0;
  for (const g of groups) {
    rows.push({ kind: "project", key: `p${g.key}`, top: y, height: LAYOUT.projectRow, project: g.project });
    y += LAYOUT.projectRow;
    for (const t of g.todos) {
      rows.push({ kind: "todo", key: `t${t.key}`, top: y, height: LAYOUT.todoRow, project: g.project, todo: t.todo });
      y += LAYOUT.todoRow;
    }
  }
  return { rows, height: y };
}

/** Rows that intersect the viewport (plus overscan): only these are drawn. */
export function visibleRows<R extends { top: number; height: number }>(
  rows: R[],
  scrollTop: number,
  viewportHeight: number,
  overscan: number = LAYOUT.overscan,
): R[] {
  if (rows.length === 0) return rows;
  const from = scrollTop;
  const to = scrollTop + viewportHeight;
  let first = rows.findIndex((r) => r.top + r.height > from);
  if (first < 0) first = rows.length - 1;
  let last = first;
  while (last + 1 < rows.length && rows[last + 1].top < to) last++;
  return rows.slice(Math.max(0, first - overscan), Math.min(rows.length, last + 1 + overscan));
}
