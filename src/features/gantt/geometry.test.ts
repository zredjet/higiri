import { describe, expect, it } from "vitest";
import {
  addDays,
  DAY_WIDTH,
  dayCenter,
  dayIndex,
  headerRows,
  layoutRows,
  rangeEnd,
  spanRect,
  startAround,
  step,
  visibleDays,
  visibleRows,
  weekday,
  weekendRuns,
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
