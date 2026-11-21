import { describe, expect, it } from "vitest";
import type { DependentShift } from "../../lib/bindings";
import { shiftDatesText, shiftQuestion, shiftTitle } from "./shiftText";

function shift(more: Partial<DependentShift> = {}): DependentShift {
  return {
    id: 2,
    title: "結合テスト計画の作成",
    parent_title: null,
    project_id: 1,
    project_name: "基幹システム更改",
    planned_start: "2026-11-22",
    planned_end: "2026-11-24",
    shifted_start: "2026-11-25",
    shifted_end: "2026-11-27",
    days: 3,
    ...more,
  };
}

describe("shiftQuestion", () => {
  it("counts the TODOs, as in the design", () => {
    expect(shiftQuestion(2)).toBe(
      "予定を後ろへ動かしたため、次の2件の予定開始が、先行TODOの予定終了より前になります。後続TODOもずらすと、足りない日数だけ後ろへ動かします。",
    );
  });
});

describe("shiftTitle", () => {
  it("names a child 「親 / 子」", () => {
    expect(shiftTitle(shift())).toBe("結合テスト計画の作成");
    expect(shiftTitle(shift({ title: "手順書", parent_title: "開発環境の標準化" }))).toBe("開発環境の標準化 / 手順書");
  });
});

describe("shiftDatesText", () => {
  it("shows the plan now and moved, with the days", () => {
    expect(shiftDatesText(shift())).toBe("11/22〜11/24 → 11/25〜11/27(3日後ろへ)");
  });
  it("shows a start only", () => {
    expect(shiftDatesText(shift({ planned_end: null, shifted_end: null, days: 1, shifted_start: "2026-11-23" }))).toBe(
      "11/22〜 → 11/23〜(1日後ろへ)",
    );
  });
  it("crosses months and years", () => {
    expect(
      shiftDatesText(
        shift({
          planned_start: "2026-12-30",
          planned_end: "2027-01-02",
          shifted_start: "2027-01-03",
          shifted_end: "2027-01-06",
          days: 4,
        }),
      ),
    ).toBe("12/30〜1/2 → 1/3〜1/6(4日後ろへ)");
  });
});
