import { describe, expect, it } from "vitest";
import { copiedText, copyButtonText, KIND_TEXT, reportKey, savedFileText, weekRangeText } from "./reportText";

describe("reportKey", () => {
  it("keeps the day of a daily report", () => {
    expect(reportKey("daily", "2026-11-19")).toEqual({ kind: "daily", date: "2026-11-19" });
  });

  it("keys a weekly report by its Monday", () => {
    for (const day of ["2026-11-16", "2026-11-19", "2026-11-22"]) {
      expect(reportKey("weekly", day)).toEqual({ kind: "weekly", date: "2026-11-16" });
    }
    expect(reportKey("weekly", "2027-01-02")).toEqual({ kind: "weekly", date: "2026-12-28" });
  });
});

describe("weekRangeText", () => {
  it("is Monday to Sunday", () => {
    expect(weekRangeText("2026-11-19")).toBe("11/16(月)〜11/22(日)");
    expect(weekRangeText("2026-11-22")).toBe("11/16(月)〜11/22(日)");
  });

  it("crosses the year end", () => {
    expect(weekRangeText("2027-01-01")).toBe("12/28(月)〜1/3(日)");
  });
});

describe("words", () => {
  it("name the kind", () => {
    expect(copyButtonText("daily")).toBe("日報をコピー");
    expect(copyButtonText("weekly")).toBe("週報をコピー");
    expect(copiedText("weekly")).toBe("週報をクリップボードにコピーしました。");
    expect(savedFileText("weekly", "/tmp/週報_2026-11-16.md")).toBe(
      "週報を保存しました。保存先: /tmp/週報_2026-11-16.md",
    );
  });

  it("move a day or a week", () => {
    expect(KIND_TEXT.daily.step).toBe(1);
    expect(KIND_TEXT.weekly.step).toBe(7);
    expect(KIND_TEXT.weekly.current).toBe("今週");
  });
});
