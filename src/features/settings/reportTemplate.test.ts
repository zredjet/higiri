import { describe, expect, it } from "vitest";
import type { ReportTemplate, Settings, WeeklyReportTemplate } from "../../lib/bindings";
import {
  OPTION_ITEMS,
  TEMPLATE_SPECS,
  WEEKLY_OPTION_ITEMS,
  defaultReportTemplate,
  defaultWeeklyReportTemplate,
  moveSection,
  optionValue,
  sectionName,
  setEnabled,
  setHeading,
  setOption,
  setTitle,
  templateEdits,
  templateOf,
  weeklySectionName,
  type SectionKind,
  type WeeklySectionKind,
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

const weeklyKinds = (t: WeeklyReportTemplate): WeeklySectionKind[] => t.sections.map((s) => s.kind);

describe("defaultWeeklyReportTemplate", () => {
  it("has the weekly sections, the logs' text and 要確認 left out", () => {
    const t = defaultWeeklyReportTemplate();
    expect(t.title).toBe("週報 {開始日}〜{終了日}");
    expect(weeklyKinds(t)).toEqual(["work", "closed", "waiting", "next_week", "attention", "note"]);
    expect(t.sections.map((s) => s.heading)).toEqual([
      "今週の作業",
      "今週完了・中止",
      "待ち",
      "来週の予定",
      "日限注意",
      "所感・連絡事項",
    ]);
    expect(optionValue(t.sections[0], "log_body")).toBe(false);
    expect(optionValue(t.sections[0], "total_time")).toBe(true);
    expect(optionValue(t.sections[4], "follow_up_and_late_start")).toBe(false);
  });

  it("has an item on the card for every option", () => {
    for (const s of defaultWeeklyReportTemplate().sections) {
      expect(WEEKLY_OPTION_ITEMS[s.kind].map((o) => o.key).sort()).toEqual(Object.keys(s.options).sort());
    }
  });

  it("names its sections by their default headings", () => {
    expect(weeklySectionName("next_week")).toBe("来週の予定");
    expect(TEMPLATE_SPECS.weekly.sectionName("work")).toBe("今週の作業");
    expect(TEMPLATE_SPECS.daily.sectionName("work")).toBe("本日の作業");
    expect(TEMPLATE_SPECS.weekly.optionItems("closed").map((o) => o.key)).toEqual(["closed_date", "close_reason"]);
  });
});

describe("weekly edits", () => {
  it("move and change the weekly sections the same way", () => {
    let t = defaultWeeklyReportTemplate();
    t = moveSection(t, "next_week", -1);
    expect(weeklyKinds(t)).toEqual(["work", "closed", "next_week", "waiting", "attention", "note"]);
    expect(moveSection(t, "note", -1)).toBe(t);
    t = setTitle(setHeading(t, "next_week", "翌週"), "{開始日}の週");
    t = setOption(setEnabled(t, "closed", false), "work", "log_body", true);
    expect(t.title).toBe("{開始日}の週");
    expect(t.sections[2].heading).toBe("翌週");
    expect(t.sections[1].enabled).toBe(false);
    expect(optionValue(t.sections[0], "log_body")).toBe(true);
    expect(optionValue(t.sections[0], "work_days")).toBe(true);
  });

  it("read and write the template of the card's kind", () => {
    const settings = {
      report_template: defaultReportTemplate(),
      weekly_report_template: defaultWeeklyReportTemplate(),
    } as Settings;
    expect(templateOf("weekly", settings)).toBe(settings.weekly_report_template);
    expect(templateOf("daily", settings)).toBe(settings.report_template);
    const weekly = setTitle(defaultWeeklyReportTemplate(), "W");
    expect(templateEdits("weekly", weekly)).toEqual({ weekly_report_template: weekly });
    const daily = setTitle(defaultReportTemplate(), "D");
    expect(templateEdits("daily", daily)).toEqual({ report_template: daily });
  });
});
