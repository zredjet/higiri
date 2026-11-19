import { describe, expect, it } from "vitest";
import {
  addDaysYmd,
  formatDate,
  formatDateWithWeekday,
  formatTimestampDate,
  monthDay,
  todayYmd,
} from "./date";

describe("date formats", () => {
  it("formats a stored date for tables", () => {
    expect(formatDate("2026-11-19")).toBe("2026/11/19");
  });

  it("formats short dates without padding", () => {
    expect(monthDay("2026-11-09")).toBe("11/9");
    expect(monthDay("2027-01-20")).toBe("1/20");
  });

  it("adds the weekday", () => {
    expect(formatDateWithWeekday("2026-11-18")).toBe("2026/11/18(水)");
    expect(formatDateWithWeekday("2026-11-19")).toBe("2026/11/19(木)");
  });

  it("shows a UTC timestamp as the local date (tests run in Asia/Tokyo)", () => {
    expect(formatTimestampDate("2026-11-19T01:30:00.000Z")).toBe("2026/11/19");
    // 16:00 UTC is already the next day in Japan.
    expect(formatTimestampDate("2026-11-19T16:00:00.000Z")).toBe("2026/11/20");
  });
});

describe("date defaults", () => {
  it("reads today in local time", () => {
    expect(todayYmd(new Date(2026, 10, 19, 23, 59))).toBe("2026-11-19");
  });

  it("adds days across month and year ends", () => {
    expect(addDaysYmd("2026-11-19", 3)).toBe("2026-11-22");
    expect(addDaysYmd("2026-11-29", 3)).toBe("2026-12-02");
    expect(addDaysYmd("2026-12-30", 3)).toBe("2027-01-02");
  });
});
