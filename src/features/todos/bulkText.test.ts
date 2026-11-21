import { describe, expect, it } from "vitest";
import type { BulkResult, BulkSkipped } from "../../lib/bindings";
import {
  applyLabel,
  bulkDoneText,
  bulkTodoTitle,
  closingChildrenText,
  daysOf,
  nextRoundsText,
  selectingText,
  selectLabel,
  skippedText,
  targetsText,
} from "./bulkText";

describe("daysOf", () => {
  it("reads whole numbers, either way", () => {
    expect(daysOf("3")).toBe(3);
    expect(daysOf(" -7 ")).toBe(-7);
    expect(daysOf("+2")).toBe(2);
    expect(daysOf("365")).toBe(365);
  });

  it("reads full-width digits and minus signs", () => {
    expect(daysOf("３")).toBe(3);
    expect(daysOf("−１０")).toBe(-10);
    expect(daysOf("ー2")).toBe(-2);
    expect(daysOf("－5")).toBe(-5);
  });

  it("waits while nothing is typed yet", () => {
    expect(daysOf("")).toBeNull();
    expect(daysOf("  ")).toBeNull();
    expect(daysOf("-")).toBeNull();
    expect(daysOf("ー")).toBeNull();
  });

  it("sends 0 for what is not a whole number (Rust says what to type)", () => {
    expect(daysOf("1.5")).toBe(0);
    expect(daysOf("abc")).toBe(0);
    expect(daysOf("0")).toBe(0);
    expect(daysOf("3日")).toBe(0);
  });

  it("sends a far number out of range as out of range", () => {
    expect(daysOf("99999999999999999999")).toBe(9999);
  });
});

describe("wording", () => {
  it("words the band and the checkboxes", () => {
    expect(selectingText(3)).toBe("3件を選択中");
    expect(selectLabel("API設計")).toBe("「API設計」を選ぶ");
  });

  it("words the confirmation", () => {
    expect(targetsText(3)).toBe("3件を変えます。");
    expect(targetsText(0)).toBe("変えられるTODOはありません。");
    expect(skippedText(2)).toBe("次の2件は変えられないため除きます。");
    expect(closingChildrenText(4, "done")).toBe("子TODO 4件も一緒に完了にします。");
    expect(closingChildrenText(1, "cancelled")).toBe("子TODO 1件も一緒に中止にします。");
    expect(nextRoundsText(2)).toBe("繰り返しのあるTODO 2件は、次の回を作ります。");
    expect(applyLabel(3)).toBe("3件を変える");
    expect(bulkTodoTitle({ title: "手順書", parent_title: "開発環境の標準化" })).toBe("開発環境の標準化 / 手順書");
    expect(bulkTodoTitle({ title: "API設計", parent_title: null })).toBe("API設計");
  });

  it("words what was done", () => {
    const skipped = (n: number) => Array.from({ length: n }, () => ({}) as BulkSkipped);
    const result = (changed: number, left: number): BulkResult => ({
      changed: Array.from({ length: changed }, (_, i) => i),
      skipped: skipped(left),
    });
    expect(bulkDoneText({ kind: "shift_due", days: 2 }, result(3, 2))).toBe(
      "3件の日限を変えました。変えられない2件は除きました。",
    );
    expect(bulkDoneText({ kind: "set_due", date: "2026-11-30" }, result(3, 0))).toBe("3件の日限を変えました。");
    expect(bulkDoneText({ kind: "shift_planned", days: -1 }, result(1, 0))).toBe("1件の予定をずらしました。");
    expect(bulkDoneText({ kind: "priority", priority: "high" }, result(2, 1))).toBe(
      "2件の優先度を変えました。変えられない1件は除きました。",
    );
    expect(bulkDoneText({ kind: "status", status: "done", reason: null }, result(5, 0))).toBe(
      "5件のステータスを変えました。",
    );
  });
});
