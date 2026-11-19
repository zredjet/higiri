import { describe, expect, it } from "vitest";
import type { SearchMatch } from "../../lib/bindings";
import { countText, matchLabel, moveSelection } from "./searchText";

const match = (field: SearchMatch["field"], log_date: string | null = null): SearchMatch => ({
  field,
  log_date,
  segments: [],
});

describe("matchLabel", () => {
  it("names the field, with the date of a work log", () => {
    expect(matchLabel(match("memo"))).toBe("メモ");
    expect(matchLabel(match("waiting_on"))).toBe("待っている相手");
    expect(matchLabel(match("close_reason"))).toBe("中止の理由");
    expect(matchLabel(match("work_log", "2026-11-09"))).toBe("作業ログ 11/9");
  });
});

describe("countText", () => {
  it("says how many were found and how many are listed", () => {
    expect(countText(3, 3)).toBe("3件");
    expect(countText(100, 100)).toBe("100件");
    expect(countText(100, 523)).toBe("523件のうち100件");
  });
});

describe("moveSelection", () => {
  it("stays inside the list", () => {
    expect(moveSelection(0, 1, 3)).toBe(1);
    expect(moveSelection(2, 1, 3)).toBe(2);
    expect(moveSelection(0, -1, 3)).toBe(0);
    expect(moveSelection(2, -1, 3)).toBe(1);
    expect(moveSelection(5, 0, 3)).toBe(2);
    expect(moveSelection(0, 1, 0)).toBe(0);
  });
});
