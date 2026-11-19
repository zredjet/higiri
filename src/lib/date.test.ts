import { describe, expect, it } from "vitest";
import { formatDate, formatTimestampDate } from "./date";

describe("date formats", () => {
  it("formats a stored date for tables", () => {
    expect(formatDate("2026-11-19")).toBe("2026/11/19");
  });

  it("shows a UTC timestamp as the local date (tests run in Asia/Tokyo)", () => {
    expect(formatTimestampDate("2026-11-19T01:30:00.000Z")).toBe("2026/11/19");
    // 16:00 UTC is already the next day in Japan.
    expect(formatTimestampDate("2026-11-19T16:00:00.000Z")).toBe("2026/11/20");
  });
});
