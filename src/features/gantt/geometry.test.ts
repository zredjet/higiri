import { describe, expect, it } from "vitest";
import {
  addDays,
  arrowBetween,
  barGrips,
  canDrag,
  DAY_WIDTH,
  dayCenter,
  dayDelta,
  dayIndex,
  dependencyArrows,
  dragLabelX,
  dragSpan,
  headerRows,
  holidayLabels,
  holidayNames,
  isDrag,
  layoutRows,
  rangeEnd,
  spanRect,
  startAround,
  step,
  visibleDays,
  visibleRows,
  weekday,
  weekendRuns,
  type ArrowRow,
  type Range,
} from "./geometry";

// The mock's view: 2026/11/09 – 2026/12/04 (26 days), today 11/19, 32px a day.
const MOCK: Range = { start: "2026-11-09", days: 26 };
const W = DAY_WIDTH.day;

describe("dates", () => {
  it("counts days across month and year ends without time zones", () => {
    expect(dayIndex("2026-11-09", "2026-11-19")).toBe(10);
    expect(dayIndex("2026-11-30", "2026-12-01")).toBe(1);
    expect(dayIndex("2026-12-31", "2027-01-01")).toBe(1);
    expect(dayIndex("2027-03-27", "2027-03-29")).toBe(2); // DST weekend elsewhere
    expect(addDays("2026-11-29", 3)).toBe("2026-12-02");
    expect(addDays("2027-01-02", -3)).toBe("2026-12-30");
    expect(weekday("2026-11-19")).toBe(4); // Thursday
    expect(rangeEnd(MOCK)).toBe("2026-12-04");
  });

  it("puts today about a third from the left", () => {
    expect(startAround("2026-11-19", 26)).toBe("2026-11-11");
    expect(visibleDays(832, "day")).toBe(26);
    expect(visibleDays(1080, "month")).toBe(270);
    expect(visibleDays(10, "day")).toBe(7);
    expect(step(26)).toBe(13);
  });
});

describe("bars (positions from the mock)", () => {
  it("places planned and actual bars", () => {
    // 移行手順書のドラフト: planned 11/11–11/18, actual 11/12–11/19.
    expect(spanRect(MOCK, { start: "2026-11-11", end: "2026-11-18" }, W)).toEqual({ x: 64, width: 256 });
    expect(spanRect(MOCK, { start: "2026-11-12", end: "2026-11-19" }, W)).toEqual({ x: 96, width: 256 });
    // 募集要項: waiting 11/17–11/19.
    expect(spanRect(MOCK, { start: "2026-11-17", end: "2026-11-19" }, W)).toEqual({ x: 256, width: 96 });
  });

  it("clips spans to the range", () => {
    expect(spanRect(MOCK, { start: "2026-11-01", end: "2026-11-10" }, W)).toEqual({ x: 0, width: 64 });
    expect(spanRect(MOCK, { start: "2026-12-03", end: "2026-12-31" }, W)).toEqual({ x: 768, width: 64 });
    expect(spanRect(MOCK, { start: "2026-10-01", end: "2026-10-31" }, W)).toBeNull();
  });

  it("centers the due-date diamond and the today line on the day", () => {
    // Mock: diamond box left 330px for 11/19 (center 336), today line at 336.
    expect(dayCenter(MOCK, "2026-11-19", W)).toBe(336);
    expect(dayCenter(MOCK, "2026-11-17", W)! - 6).toBe(266); // ライセンス棚卸し
    expect(dayCenter(MOCK, "2026-12-05", W)).toBeNull();
  });

  it("shades weekends as in the mock", () => {
    // Mock: left 160 / 384 / 608, 64px wide each.
    expect(weekendRuns(MOCK).map((r) => [r.index * W, r.length * W])).toEqual([
      [160, 64],
      [384, 64],
      [608, 64],
    ]);
  });

  it("shades holidays with the weekends", () => {
    // 11/23 (Monday) is 勤労感謝の日: the weekend run grows to three days.
    expect(weekendRuns(MOCK, ["2026-11-23"]).map((r) => [r.index * W, r.length * W])).toEqual([
      [160, 64],
      [384, 96],
      [608, 64],
    ]);
    // A holiday on a weekday by itself, and dates outside the range.
    expect(weekendRuns(MOCK, ["2026-11-03", "2026-11-26"]).map((r) => [r.index, r.length])).toEqual([
      [5, 2],
      [12, 2],
      [17, 1],
      [19, 2],
    ]);
  });
});

describe("dragging a planned bar (G-6)", () => {
  const span = { start: "2026-11-20", end: "2026-11-27" };

  it("drags the planned bars of open TODOs only", () => {
    for (const status of ["todo", "doing", "waiting", "hold"] as const) expect(canDrag(status, span)).toBe(true);
    expect(canDrag("done", span)).toBe(false);
    expect(canDrag("cancelled", span)).toBe(false);
    // No bar without both planned dates (get_gantt sends planned: null).
    expect(canDrag("doing", null)).toBe(false);
  });

  it("treats moves under 3px as clicks", () => {
    expect(isDrag(0, 0)).toBe(false);
    expect(isDrag(2, -2)).toBe(false);
    expect(isDrag(2.9, 2.9)).toBe(false);
    expect(isDrag(3, 0)).toBe(true);
    expect(isDrag(0, -3)).toBe(true);
  });

  it("snaps to whole days on every scale, the same both ways", () => {
    // 日 (32px a day)
    expect(dayDelta(0, DAY_WIDTH.day)).toBe(0);
    expect(dayDelta(15, DAY_WIDTH.day)).toBe(0);
    expect(dayDelta(-15, DAY_WIDTH.day)).toBe(0); // not -0
    expect(dayDelta(16, DAY_WIDTH.day)).toBe(1);
    expect(dayDelta(-16, DAY_WIDTH.day)).toBe(-1);
    expect(dayDelta(100, DAY_WIDTH.day)).toBe(3);
    // 週 (12px)
    expect(dayDelta(30, DAY_WIDTH.week)).toBe(3);
    expect(dayDelta(-30, DAY_WIDTH.week)).toBe(-3);
    // 月 (4px): the 3px that start a drag already move a day.
    expect(dayDelta(1, DAY_WIDTH.month)).toBe(0);
    expect(dayDelta(3, DAY_WIDTH.month)).toBe(1);
    expect(dayDelta(-3, DAY_WIDTH.month)).toBe(-1);
    expect(dayDelta(-61, DAY_WIDTH.month)).toBe(-15);
  });

  it("moves the span without changing its length", () => {
    expect(dragSpan(span, "move", 0)).toEqual(span);
    expect(dragSpan(span, "move", 3)).toEqual({ start: "2026-11-23", end: "2026-11-30" });
    expect(dragSpan(span, "move", -21)).toEqual({ start: "2026-10-30", end: "2026-11-06" });
    expect(dragSpan({ start: "2026-12-28", end: "2026-12-31" }, "move", 3)).toEqual({
      start: "2026-12-31",
      end: "2027-01-03",
    });
  });

  it("counts calendar days: weekends and holidays are not skipped", () => {
    // Fri 11/20 + 1 is Sat 11/21; + 3 is 11/23 (勤労感謝の日).
    const day = { start: "2026-11-20", end: "2026-11-20" };
    expect(dragSpan(day, "move", 1)).toEqual({ start: "2026-11-21", end: "2026-11-21" });
    expect(dragSpan(day, "end", 3)).toEqual({ start: "2026-11-20", end: "2026-11-23" });
  });

  it("resizes one end and stops at a one-day span", () => {
    expect(dragSpan(span, "start", -2)).toEqual({ start: "2026-11-18", end: "2026-11-27" });
    expect(dragSpan(span, "start", 7)).toEqual({ start: "2026-11-27", end: "2026-11-27" });
    expect(dragSpan(span, "start", 10)).toEqual({ start: "2026-11-27", end: "2026-11-27" });
    expect(dragSpan(span, "end", 3)).toEqual({ start: "2026-11-20", end: "2026-11-30" });
    expect(dragSpan(span, "end", -7)).toEqual({ start: "2026-11-20", end: "2026-11-20" });
    expect(dragSpan(span, "end", -30)).toEqual({ start: "2026-11-20", end: "2026-11-20" });
  });

  it("puts grips on both ends and moves with the rest", () => {
    // 移行手順書のドラフト (planned 11/11–11/18): x 64, 256px wide.
    expect(barGrips(MOCK, { start: "2026-11-11", end: "2026-11-18" }, W)).toEqual({
      move: { x: 70, width: 244 },
      start: { x: 60, width: 10 },
      end: { x: 314, width: 10 },
    });
    // One day on the day scale (32px): 10px grips, 20px to move.
    expect(barGrips(MOCK, { start: "2026-11-19", end: "2026-11-19" }, W)).toEqual({
      move: { x: 326, width: 20 },
      start: { x: 316, width: 10 },
      end: { x: 346, width: 10 },
    });
  });

  it("keeps short bars on the month scale movable and resizable", () => {
    const year: Range = { start: "2026-11-01", days: 270 };
    const m = DAY_WIDTH.month;
    // One day (4px): the bar moves; 4px just outside each end resize it.
    expect(barGrips(year, { start: "2026-11-19", end: "2026-11-19" }, m)).toEqual({
      move: { x: 72, width: 4 },
      start: { x: 68, width: 4 },
      end: { x: 76, width: 4 },
    });
    // Three days (12px): 8px in the middle move it, the ends reach 2px in.
    expect(barGrips(year, { start: "2026-11-19", end: "2026-11-21" }, m)).toEqual({
      move: { x: 74, width: 8 },
      start: { x: 68, width: 6 },
      end: { x: 82, width: 6 },
    });
  });

  it("leaves out the grip of an end outside the range", () => {
    expect(barGrips(MOCK, { start: "2026-11-01", end: "2026-11-10" }, W)).toEqual({
      move: { x: 0, width: 58 },
      start: null,
      end: { x: 58, width: 10 },
    });
    expect(barGrips(MOCK, { start: "2026-12-03", end: "2026-12-31" }, W)).toEqual({
      move: { x: 774, width: 58 },
      start: { x: 764, width: 10 },
      end: null,
    });
    expect(barGrips(MOCK, { start: "2026-11-01", end: "2026-12-31" }, W)).toEqual({
      move: { x: 0, width: 832 },
      start: null,
      end: null,
    });
    expect(barGrips(MOCK, { start: "2026-10-01", end: "2026-10-31" }, W)).toBeNull();
  });

  it("keeps the dates shown while dragging inside the chart", () => {
    expect(dragLabelX(MOCK, span, W, 90, 832)).toBe(352);
    expect(dragLabelX(MOCK, { start: "2026-11-01", end: "2026-11-12" }, W, 90, 832)).toBe(0);
    expect(dragLabelX(MOCK, { start: "2026-12-03", end: "2026-12-05" }, W, 90, 832)).toBe(742);
    expect(dragLabelX(MOCK, { start: "2027-01-10", end: "2027-01-12" }, W, 90, 832)).toBe(742);
  });
});

describe("dependency arrows (T-11)", () => {
  // Rows of 基幹システム更改 under its project row (32px), 44px each.
  const row = (i: number) => 32 + 44 * i;

  it("turns once when the later bar starts far enough right", () => {
    // 移行手順書のドラフト (11/11–11/18) → 結合テスト計画の作成 (11/20–11/27), two rows down.
    const a = arrowBetween(
      MOCK,
      W,
      { top: row(0), planned: { start: "2026-11-11", end: "2026-11-18" } },
      { top: row(2), planned: { start: "2026-11-20", end: "2026-11-27" } },
    );
    // From the end of 11/18 (x 320) at the bar's middle (y 46) to the start of 11/20 (x 352, y 134).
    expect(a.line).toBe("M320 46H328V134H347");
    expect(a.head).toBe("352,134 347,130.25 347,137.75");
    expect([a.minX, a.maxX]).toEqual([320, 352]);
  });

  it("goes round when the later bar starts before the first one ends", () => {
    // API設計書のレビュー対応 (11/16–11/19) → 性能試験の環境準備 (11/17–11/20), two rows down.
    const a = arrowBetween(
      MOCK,
      W,
      { top: row(1), planned: { start: "2026-11-16", end: "2026-11-19" } },
      { top: row(3), planned: { start: "2026-11-17", end: "2026-11-20" } },
    );
    // Out of 11/19's end (352), along the first row's bottom edge (y 120), back past 11/17 (256).
    expect(a.line).toBe("M352 90H360V120H248V178H251");
    expect(a.head).toBe("256,178 251,174.25 251,181.75");
    expect([a.minX, a.maxX]).toEqual([248, 360]);
    // The later TODO above: along the first row's top edge.
    const up = arrowBetween(
      MOCK,
      W,
      { top: row(3), planned: { start: "2026-11-16", end: "2026-11-19" } },
      { top: row(1), planned: { start: "2026-11-19", end: "2026-11-20" } },
    );
    expect(up.line).toBe("M352 178H360V164H312V90H315");
  });

  it("counts a bar just one gap away as too close to turn once", () => {
    // 11/18 ends at x 320; 11/19 starts at 320: no room, so it goes round.
    const a = arrowBetween(
      MOCK,
      W,
      { top: row(0), planned: { start: "2026-11-11", end: "2026-11-18" } },
      { top: row(1), planned: { start: "2026-11-19", end: "2026-11-19" } },
    );
    expect(a.line).toBe("M320 46H328V76H312V90H315");
  });

  const rows: ArrowRow[] = [
    { id: 1, top: row(0), planned: { start: "2026-11-11", end: "2026-11-18" }, dependsOn: [] },
    { id: 2, top: row(1), planned: { start: "2026-11-20", end: "2026-11-27" }, dependsOn: [1, 9] },
    { id: 3, top: row(2), planned: null, dependsOn: [1] },
    { id: 4, top: row(3), planned: { start: "2026-11-24", end: "2026-11-26" }, dependsOn: [2, 3] },
  ];
  const all = { top: 0, bottom: 1000 };

  it("joins only rows that are laid out with a planned bar", () => {
    // 9 is hidden (another project, folded, 完了・中止 not shown); 3 has no planned bar.
    const arrows = dependencyArrows(rows, MOCK, W, 832, all);
    expect(arrows.map((a) => a.key)).toEqual(["1-2", "2-4"]);
    expect(arrows[0]).toMatchObject({ firstId: 1, laterId: 2 });
  });

  it("leaves out arrows wholly outside the drawn rows or the chart", () => {
    // Drawn rows from row(2): 1-2 (rows 0–1) is above them; 2-4 reaches into them.
    expect(dependencyArrows(rows, MOCK, W, 832, { top: row(2), bottom: row(4) }).map((a) => a.key)).toEqual(["2-4"]);
    // Ending exactly where the view starts is outside.
    expect(dependencyArrows(rows, MOCK, W, 832, { top: row(2), bottom: row(3) }).map((a) => a.key)).toEqual(["2-4"]);
    expect(dependencyArrows(rows, MOCK, W, 832, { top: row(4), bottom: row(6) })).toEqual([]);
    // A range far to the right: every bar lies left of the chart.
    const later: Range = { start: "2027-01-01", days: 26 };
    expect(dependencyArrows(rows, later, W, 832, all)).toEqual([]);
    // Partly in the range: kept (the chart clips the rest).
    const partly: Range = { start: "2026-11-25", days: 26 };
    expect(dependencyArrows(rows, partly, W, 832, all).map((a) => a.key)).toEqual(["2-4"]);
  });
});

describe("holidays", () => {
  const h = (date: string, name: string, custom = false) => ({ date, name, custom });

  it("names a date by its national holiday, or its custom holidays", () => {
    const names = holidayNames([
      h("2027-01-01", "元日"),
      h("2027-01-01", "年末年始", true),
      h("2027-01-02", "年末年始", true),
      h("2027-01-02", "冬季休業", true),
    ]);
    expect([...names.entries()].sort()).toEqual([
      ["2027-01-01", "元日"],
      ["2027-01-02", "年末年始・冬季休業"],
    ]);
  });

  it("puts names in the header's top row where they fit", () => {
    const names = holidayNames([h("2026-11-23", "勤労感謝の日")]);
    expect(holidayLabels(MOCK, "day", names, 832)).toEqual([{ index: 14, label: "勤労感謝の日" }]);
    // No room left at the right edge.
    expect(holidayLabels(MOCK, "day", names, 500)).toEqual([]);
  });

  it("leaves out names that would overlap a month label or another name", () => {
    const gw = holidayNames([
      h("2026-04-29", "昭和の日"),
      h("2026-05-03", "憲法記念日"),
      h("2026-05-04", "みどりの日"),
      h("2026-05-05", "こどもの日"),
      h("2026-05-06", "振替休日"),
    ]);
    // 4/29 sits right after 「2026年4月」; 5/4 and 5/6 would touch the name before.
    expect(holidayLabels({ start: "2026-04-27", days: 26 }, "day", gw, 832)).toEqual([
      { index: 6, label: "憲法記念日" },
      { index: 8, label: "こどもの日" },
    ]);
    // A run of days with the same name gets one label.
    const yearEnd = holidayNames(
      ["2026-12-29", "2026-12-30", "2026-12-31"].map((date) => h(date, "年末年始", true)),
    );
    expect(holidayLabels({ start: "2026-12-21", days: 26 }, "day", yearEnd, 832)).toEqual([
      { index: 8, label: "年末年始" },
    ]);
  });
});

describe("header", () => {
  it("day scale: months over day numbers, today highlighted", () => {
    const { top, bottom } = headerRows(MOCK, "day", "2026-11-19");
    expect(top.map((s) => [s.label, s.length * W])).toEqual([
      ["2026年11月", 704],
      ["12月", 128],
    ]);
    expect(bottom).toHaveLength(26);
    expect(bottom[0].label).toBe("9");
    expect(bottom[22].label).toBe("1");
    expect(bottom.filter((s) => s.current).map((s) => s.label)).toEqual(["19"]);
  });

  it("week scale: Mondays", () => {
    const { bottom } = headerRows({ start: "2026-11-16", days: 21 }, "week", "2026-11-19");
    expect(bottom.map((s) => [s.label, s.index, s.length])).toEqual([
      ["11/16", 0, 7],
      ["11/23", 7, 7],
      ["11/30", 14, 7],
    ]);
    const partial = headerRows({ start: "2026-11-19", days: 10 }, "week", "2026-11-19").bottom;
    expect(partial.map((s) => s.label)).toEqual(["", "11/23"]);
  });

  it("month scale: years over months, the year on January", () => {
    const { top, bottom } = headerRows({ start: "2026-12-01", days: 62 }, "month", "2026-12-10");
    expect(top.map((s) => [s.label, s.length])).toEqual([
      ["2026年", 31],
      ["2027年", 31],
    ]);
    expect(bottom.map((s) => s.label)).toEqual(["12月", "1月"]);
  });
});

describe("rows", () => {
  const groups = [
    { key: "1", project: "A", todos: [{ key: "1", todo: "a1" }, { key: "2", todo: "a2" }] },
    { key: "2", project: "B", todos: [{ key: "3", todo: "b1" }] },
  ];

  it("stacks project rows (32px) and TODO rows (44px)", () => {
    const { rows, height } = layoutRows(groups);
    expect(rows.map((r) => [r.kind, r.top])).toEqual([
      ["project", 0],
      ["todo", 32],
      ["todo", 76],
      ["project", 120],
      ["todo", 152],
    ]);
    expect(height).toBe(196);
  });

  it("draws only the rows in view", () => {
    const many = Array.from({ length: 1000 }, (_, i) => ({ top: i * 44, height: 44 }));
    const shown = visibleRows(many, 44 * 500, 440, 2);
    expect(shown[0].top).toBe(44 * 498);
    expect(shown).toHaveLength(14);
    expect(visibleRows(many, 0, 440, 2)[0].top).toBe(0);
    expect(visibleRows([], 0, 100)).toEqual([]);
  });
});
