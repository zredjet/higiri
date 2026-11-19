import { describe, expect, it } from "vitest";
import { rowWindow } from "./virtualRows";

describe("rowWindow", () => {
  const even = Array.from({ length: 1000 }, () => 50);

  it("draws the visible rows plus the overscan", () => {
    // 0–600px: rows 0..11, plus 10 below.
    expect(rowWindow(even, 0, 600, 10)).toEqual({ first: 0, end: 22, before: 0, after: (1000 - 22) * 50 });
    // 5,000–5,600px: rows 100..111, plus 10 on each side.
    expect(rowWindow(even, 5000, 5600, 10)).toEqual({ first: 90, end: 122, before: 90 * 50, after: (1000 - 122) * 50 });
  });

  it("keeps the total height whatever is drawn", () => {
    const mixed = Array.from({ length: 500 }, (_, i) => (i % 3 === 0 ? 85 : 65));
    const total = mixed.reduce((a, b) => a + b, 0);
    for (const top of [0, 1234, 20000, total - 300]) {
      const w = rowWindow(mixed, top, top + 700, 5);
      const drawn = mixed.slice(w.first, w.end).reduce((a, b) => a + b, 0);
      expect(w.before + drawn + w.after).toBe(total);
    }
  });

  it("handles the ends and an empty list", () => {
    expect(rowWindow([], 0, 600)).toEqual({ first: 0, end: 0, before: 0, after: 0 });
    // Scrolled past the end: the last rows are still drawn.
    const w = rowWindow(even, 999999, 1000600, 3);
    expect(w.end).toBe(1000);
    expect(w.first).toBe(996);
    // Above the list (it starts lower on the page).
    expect(rowWindow(even, -400, 200, 0)).toEqual({ first: 0, end: 4, before: 0, after: 996 * 50 });
  });
});
