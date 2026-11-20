// Gantt geometry: dates -> x, rows -> y, header labels, visible range.
// Pure functions (tested in geometry.test.ts). Sizes are the mock's
// (docs/ui-mock/README.md「ガントの寸法」) and the design's day widths.
import type { DaySpan, Holiday, Status } from "../../lib/bindings";

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

// ---- dragging a planned bar (G-6) ------------------------------------------------

/** What a drag changes: the whole span, or only its start or end. */
export type DragMode = "move" | "start" | "end";

const OPEN_STATUSES: ReadonlySet<Status> = new Set<Status>(["todo", "doing", "waiting", "hold"]);

/** Only the planned bars of open TODOs (both planned dates set) can be dragged. */
export function canDrag(status: Status, planned: DaySpan | null): planned is DaySpan {
  return planned !== null && OPEN_STATUSES.has(status);
}

/** A pointer that moved less than this (px, on both axes) clicked. */
export const DRAG_THRESHOLD = 3;

/** True once the pointer moved far enough to drag instead of click. */
export function isDrag(dx: number, dy: number): boolean {
  return Math.abs(dx) >= DRAG_THRESHOLD || Math.abs(dy) >= DRAG_THRESHOLD;
}

/**
 * Whole days a drag of `dx` px moves, snapped to the nearest day (the same
 * both ways). Works for every scale: pass its day width.
 */
export function dayDelta(dx: number, dayWidth: number): number {
  const days = Math.round(Math.abs(dx) / dayWidth);
  return days === 0 ? 0 : Math.sign(dx) * days;
}

/**
 * The span after a drag of `delta` calendar days (holidays are not skipped,
 * whatever the day count setting). `move` keeps the length; `start` and
 * `end` stop where the span is one day long, so the start never passes the end.
 */
export function dragSpan(span: DaySpan, mode: DragMode, delta: number): DaySpan {
  if (mode === "move") return { start: addDays(span.start, delta), end: addDays(span.end, delta) };
  if (mode === "start") {
    const start = addDays(span.start, delta);
    return { start: start > span.end ? span.end : start, end: span.end };
  }
  const end = addDays(span.end, delta);
  return { start: span.start, end: end < span.start ? span.start : end };
}

/**
 * Grab areas of a planned bar, in px. An end grip reaches `outside` px
 * beyond the bar and up to `inside` px into it, leaving at least `moveMin` px
 * of the bar (all of it when shorter) to move it. `padY` widens every grip
 * above and below the thin bar.
 */
export const GRIP = { outside: 4, inside: 6, moveMin: 8, padY: 3 } as const;

export interface Grips {
  move: { x: number; width: number };
  /** null when the start is outside the range (not on screen). */
  start: { x: number; width: number } | null;
  end: { x: number; width: number } | null;
}

/**
 * Where to grab a planned bar: its ends resize it, the rest moves it. Even a
 * one-day bar on the month scale (4px) can do both: grabbing the bar moves
 * it, grabbing just outside an end resizes it. null when the bar is outside
 * the range.
 */
export function barGrips(r: Range, span: DaySpan, dayWidth: number): Grips | null {
  const rect = spanRect(r, span, dayWidth);
  if (!rect) return null;
  const startShown = dayIndex(r.start, span.start) >= 0;
  const endShown = dayIndex(r.start, span.end) < r.days;
  const ends = Number(startShown) + Number(endShown);
  const inside = ends === 0 ? 0 : Math.min(GRIP.inside, Math.max(0, (rect.width - GRIP.moveMin) / ends));
  const moveFrom = rect.x + (startShown ? inside : 0);
  const moveTo = rect.x + rect.width - (endShown ? inside : 0);
  return {
    move: { x: moveFrom, width: moveTo - moveFrom },
    start: startShown ? { x: rect.x - GRIP.outside, width: GRIP.outside + inside } : null,
    end: endShown ? { x: moveTo, width: inside + GRIP.outside } : null,
  };
}

/**
 * Left edge (chart x) of the dates shown while dragging: above the bar's
 * start, kept inside the chart even when the bar is not.
 */
export function dragLabelX(r: Range, span: DaySpan, dayWidth: number, labelWidth: number, chartWidth: number): number {
  const x = dayIndex(r.start, span.start) * dayWidth;
  return Math.max(0, Math.min(x, chartWidth - labelWidth));
}

/** Center x of a day, or null when the day is outside the range. */
export function dayCenter(r: Range, date: string, dayWidth: number): number | null {
  const i = dayIndex(r.start, date);
  if (i < 0 || i >= r.days) return null;
  return i * dayWidth + dayWidth / 2;
}

/**
 * Day indexes falling on Saturday, Sunday or one of `holidays` (national and
 * custom holidays, "YYYY-MM-DD"), merged into runs.
 */
export function weekendRuns(r: Range, holidays: Iterable<string> = []): Array<{ index: number; length: number }> {
  const off = new Set(holidays);
  const runs: Array<{ index: number; length: number }> = [];
  for (let i = 0; i < r.days; i++) {
    const date = addDays(r.start, i);
    const wd = weekday(date);
    if (wd !== 0 && wd !== 6 && !off.has(date)) continue;
    const last = runs[runs.length - 1];
    if (last && last.index + last.length === i) last.length += 1;
    else runs.push({ index: i, length: 1 });
  }
  return runs;
}

/**
 * One name per date: the national holiday's when there is one, otherwise the
 * custom holidays' names joined with 「・」.
 */
export function holidayNames(holidays: Holiday[]): Map<string, string> {
  const national = new Map<string, string>();
  const custom = new Map<string, string[]>();
  for (const h of holidays) {
    if (!h.custom) national.set(h.date, h.name);
    else custom.set(h.date, [...(custom.get(h.date) ?? []), h.name]);
  }
  const out = new Map<string, string>();
  for (const [date, names] of custom) out.set(date, names.join("・"));
  for (const [date, name] of national) out.set(date, name);
  return out;
}

/** Rough width of header text: full-width characters are square, others about 0.6 em. */
export function textWidth(text: string, fontSize: number): number {
  let w = 0;
  for (const ch of text) w += (ch.codePointAt(0) ?? 0) > 0xff ? fontSize : fontSize * 0.6;
  return w;
}

/** Font size of the holiday names in the header's top row. */
export const HOLIDAY_LABEL_SIZE = 11;
const MONTH_LABEL_SIZE = 12;
const LABEL_GAP = 8;

/**
 * Holiday names placed in the header's top row, next to the month labels:
 * one label per run of days with the same name, at the run's first day,
 * left out where it would overlap a month label or the previous name.
 * Every holiday still has its name in `names` (tooltips).
 */
export function holidayLabels(
  r: Range,
  scale: Scale,
  names: Map<string, string>,
  width: number,
): Array<{ index: number; label: string }> {
  const w = DAY_WIDTH[scale];
  const { top } = headerRows(r, scale, r.start);
  const taken: Array<[number, number]> = top
    .filter((s) => s.length * w >= 40)
    .map((s) => [s.index * w + 8, s.index * w + 8 + textWidth(s.label, MONTH_LABEL_SIZE)]);
  const out: Array<{ index: number; label: string }> = [];
  for (let i = 0; i < r.days; i++) {
    const name = names.get(addDays(r.start, i));
    if (!name || (i > 0 && names.get(addDays(r.start, i - 1)) === name)) continue;
    const from = i * w + 4;
    const to = from + textWidth(name, HOLIDAY_LABEL_SIZE);
    if (to > width) continue;
    if (taken.some(([a, b]) => from < b + LABEL_GAP && a < to + LABEL_GAP)) continue;
    taken.push([from, to]);
    out.push({ index: i, label: name });
  }
  return out;
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

// ---- dependency arrows (T-11) -----------------------------------------------------

/**
 * Arrow sizes (px): the line leaves a bar end `gap` px sideways before it
 * turns, and its head is `head` px long and twice `head * 0.75` wide.
 */
export const ARROW = { gap: 8, head: 5 } as const;

/** A row the arrows may join: a laid-out TODO row with its planned span (null: no bar). */
export interface ArrowRow {
  id: number;
  top: number;
  planned: DaySpan | null;
  /** The TODOs this one waits for (GanttRow.depends_on). */
  dependsOn: number[];
}

export interface Arrow {
  key: string;
  firstId: number;
  laterId: number;
  /** SVG path of the line. */
  line: string;
  /** SVG points of the head, at the later bar's start. */
  head: string;
}

/** Vertical middle of a row's planned bar. */
function plannedMiddle(top: number): number {
  return top + LAYOUT.plannedTop + LAYOUT.plannedHeight / 2;
}

/**
 * The arrow from the end of `first`'s planned bar to the start of
 * `later`'s. x is not clipped to the range (the chart clips it). When the
 * later bar starts far enough right, the line turns once down (or up) to
 * it; otherwise it goes round: out of the first bar's end, along the edge
 * of the first row toward the later one, back left past the later bar's
 * start, and into it.
 */
export function arrowBetween(
  r: Range,
  dayWidth: number,
  first: { top: number; planned: DaySpan },
  later: { top: number; planned: DaySpan },
): { line: string; head: string; minX: number; maxX: number } {
  const fx = (dayIndex(r.start, first.planned.end) + 1) * dayWidth;
  const fy = plannedMiddle(first.top);
  const tx = dayIndex(r.start, later.planned.start) * dayWidth;
  const ty = plannedMiddle(later.top);
  const { gap, head } = ARROW;
  const end = tx - head;
  let line: string;
  let minX: number;
  let maxX: number;
  if (tx - fx >= 2 * gap) {
    const turn = fx + gap;
    line = `M${fx} ${fy}H${turn}V${ty}H${end}`;
    minX = fx;
    maxX = tx;
  } else {
    // Along the first row's bottom edge (later below) or top edge (above).
    const edge = later.top > first.top ? first.top + LAYOUT.todoRow : first.top;
    const out = fx + gap;
    const back = tx - gap;
    line = `M${fx} ${fy}H${out}V${edge}H${back}V${ty}H${end}`;
    minX = Math.min(fx, back);
    maxX = Math.max(out, tx);
  }
  const w = head * 0.75;
  return { line, head: `${tx},${ty} ${end},${ty - w} ${end},${ty + w}`, minX, maxX };
}

/**
 * The dependency arrows to draw. Both TODOs must be laid out (not hidden,
 * folded away or in another project) with a planned bar. An arrow is left
 * out when it lies wholly above or below the drawn rows (`view`: the top of
 * the first drawn row to the bottom of the last), or wholly left or right
 * of the chart (`width`).
 */
export function dependencyArrows(
  rows: ArrowRow[],
  r: Range,
  dayWidth: number,
  width: number,
  view: { top: number; bottom: number },
): Arrow[] {
  const byId = new Map<number, ArrowRow>();
  for (const row of rows) byId.set(row.id, row);
  const out: Arrow[] = [];
  for (const later of rows) {
    if (!later.planned) continue;
    for (const firstId of later.dependsOn) {
      const first = byId.get(firstId);
      if (!first?.planned) continue;
      const top = Math.min(first.top, later.top);
      const bottom = Math.max(first.top, later.top) + LAYOUT.todoRow;
      if (bottom <= view.top || top >= view.bottom) continue;
      const a = arrowBetween(r, dayWidth, { top: first.top, planned: first.planned }, { top: later.top, planned: later.planned });
      if (a.maxX < 0 || a.minX > width) continue;
      out.push({ key: `${firstId}-${later.id}`, firstId, laterId: later.id, line: a.line, head: a.head });
    }
  }
  return out;
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
