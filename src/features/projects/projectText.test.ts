import { describe, expect, it } from "vitest";
import type { ProjectSummary } from "../../lib/bindings";
import { colorOptions, PROJECT_COLORS, projectStatsText } from "./projectText";

describe("projectStatsText", () => {
  it("matches the mock's line for 基幹システム更改", () => {
    const s = {
      counts: { todo: 2, doing: 1, waiting: 1, hold: 0, done: 1, cancelled: 1 },
      todo_count: 6,
      progress_total: 5,
      progress_done: 1,
      attention_count: 3,
    } as ProjectSummary;
    expect(projectStatsText(s)).toBe("5件中1件完了 ・ 中止 1件 ・ 日限注意 3件");
  });
});

describe("colorOptions", () => {
  it("offers the palette, plus the current colour when it is not in it", () => {
    expect(colorOptions("#3b6fd4")).toBe(PROJECT_COLORS);
    const opts = colorOptions("#123456");
    expect(opts).toHaveLength(PROJECT_COLORS.length + 1);
    expect(opts.at(-1)).toEqual({ value: "#123456", label: "いまの色(#123456)" });
  });
});
