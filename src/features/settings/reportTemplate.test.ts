import { describe, expect, it } from "vitest";
import type { ReportTemplate } from "../../lib/bindings";
import {
  OPTION_ITEMS,
  defaultReportTemplate,
  moveSection,
  optionValue,
  sectionName,
  setEnabled,
  setHeading,
  setOption,
  setTitle,
  type SectionKind,
} from "./reportTemplate";

const kinds = (t: ReportTemplate): SectionKind[] => t.sections.map((s) => s.kind);

describe("defaultReportTemplate", () => {
  it("is the MVP's report, with every section shown", () => {
    const t = defaultReportTemplate();
    expect(t.title).toBe("日報 {日付}({曜日})");
    expect(kinds(t)).toEqual(["work", "closed", "waiting", "tomorrow", "attention", "note"]);
    expect(t.sections.every((s) => s.enabled)).toBe(true);
  });

  it("is a copy each time", () => {
    const a = defaultReportTemplate();
    a.sections[0].heading = "変えた";
    expect(defaultReportTemplate().sections[0].heading).toBe("本日の作業");
  });

  it("has an item on the card for every option", () => {
    for (const s of defaultReportTemplate().sections) {
      expect(OPTION_ITEMS[s.kind].map((o) => o.key).sort()).toEqual(Object.keys(s.options).sort());
    }
  });
});

describe("sectionName", () => {
  it("is the default heading", () => {
    expect(sectionName("work")).toBe("本日の作業");
    expect(sectionName("note")).toBe("所感・連絡事項");
  });
});

describe("moveSection", () => {
  it("swaps with the next or previous section", () => {
    const t = defaultReportTemplate();
    expect(kinds(moveSection(t, "waiting", -1))).toEqual(["work", "waiting", "closed", "tomorrow", "attention", "note"]);
    expect(kinds(moveSection(t, "work", 1))).toEqual(["closed", "work", "waiting", "tomorrow", "attention", "note"]);
  });

  it("stops at the ends and keeps 所感・連絡事項 last", () => {
    const t = defaultReportTemplate();
    expect(moveSection(t, "work", -1)).toBe(t);
    expect(moveSection(t, "attention", 1)).toBe(t);
    expect(moveSection(t, "note", -1)).toBe(t);
  });
});

describe("edits", () => {
  it("change only the section asked for", () => {
    let t = defaultReportTemplate();
    t = setTitle(t, "{日付} 業務報告");
    t = setHeading(t, "waiting", "待ち状況");
    t = setEnabled(t, "closed", false);
    t = setOption(t, "attention", "follow_up_and_late_start", true);
    expect(t.title).toBe("{日付} 業務報告");
    expect(t.sections.map((s) => s.heading)).toEqual([
      "本日の作業",
      "本日完了・中止",
      "待ち状況",
      "明日の予定",
      "日限注意",
      "所感・連絡事項",
    ]);
    expect(t.sections.map((s) => s.enabled)).toEqual([true, false, true, true, true, true]);
    const attention = t.sections[4];
    expect(optionValue(attention, "follow_up_and_late_start")).toBe(true);
    expect(optionValue(attention, "days")).toBe(true);
    expect(optionValue(t.sections[0], "follow_up_and_late_start")).toBe(false);
  });
});
