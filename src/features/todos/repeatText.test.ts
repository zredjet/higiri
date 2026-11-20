import { describe, expect, it } from "vitest";
import type { RepeatInfo } from "../../lib/bindings";
import {
  everyDaysOf,
  leapDayNote,
  monthEndNote,
  nextDueText,
  closedRepeatText,
  repeatMarkTitle,
  repeatRuleText,
  ruleFromDue,
  toggledWeekday,
  untilText,
} from "./repeatText";

function info(next_due: string | null, until_date: string | null = null): RepeatInfo {
  return { rule: { kind: "weekly", weekdays: ["mon"] }, until_date, next_due };
}

describe("repeatRuleText", () => {
  it("words every kind as the design does", () => {
    expect(repeatRuleText({ kind: "daily" })).toBe("毎日");
    expect(repeatRuleText({ kind: "weekly", weekdays: ["mon", "thu"] })).toBe("毎週 月・木");
    expect(repeatRuleText({ kind: "monthly_day", day: 15 })).toBe("毎月 15日");
    expect(repeatRuleText({ kind: "monthly_weekday", week: 2, weekday: "tue" })).toBe("毎月 第2火曜");
    expect(repeatRuleText({ kind: "monthly_weekday", week: -1, weekday: "fri" })).toBe("毎月 最終金曜");
    expect(repeatRuleText({ kind: "monthly_last" })).toBe("毎月 月末");
    expect(repeatRuleText({ kind: "yearly", month: 3, day: 31 })).toBe("毎年 3/31");
    expect(repeatRuleText({ kind: "every_days", days: 10 })).toBe("10日ごと");
  });

  it("lists the days of the week from Monday", () => {
    expect(repeatRuleText({ kind: "weekly", weekdays: ["sun", "wed", "mon"] })).toBe("毎週 月・水・日");
  });
});

describe("the next round and the end", () => {
  it("shows the next round's due date from Rust", () => {
    expect(nextDueText(info("2026-12-09"))).toBe("次の回の日限 12/9");
    expect(nextDueText(info(null, "2026-11-29"))).toBe("次の回はありません(終わりの日を過ぎます)");
  });

  it("shows the end, if any", () => {
    expect(untilText(info("2026-11-30", "2027-03-31"))).toBe("2027/03/31 まで");
    expect(untilText(info("2026-11-30"))).toBeNull();
  });

  it("names the mark and tells that the next round was made", () => {
    expect(repeatMarkTitle(info("2026-11-30"))).toBe("繰り返し:毎週 月");
    expect(closedRepeatText("2026-12-07")).toBe("次の回(日限 12/7)を作りました。");
    expect(closedRepeatText(null)).toBe("繰り返しの終わりの日を過ぎたため、次の回は作りませんでした。");
  });
});

describe("ruleFromDue", () => {
  // 2026-11-19 is a Thursday, the third of the month.
  it("takes the day of the week, the day and the month of the due date", () => {
    expect(ruleFromDue("daily", "2026-11-19")).toEqual({ kind: "daily" });
    expect(ruleFromDue("weekly", "2026-11-19")).toEqual({ kind: "weekly", weekdays: ["thu"] });
    expect(ruleFromDue("weekly", "2026-11-22")).toEqual({ kind: "weekly", weekdays: ["sun"] });
    expect(ruleFromDue("monthly_day", "2026-11-19")).toEqual({ kind: "monthly_day", day: 19 });
    expect(ruleFromDue("monthly_weekday", "2026-11-19")).toEqual({
      kind: "monthly_weekday",
      week: 3,
      weekday: "thu",
    });
    expect(ruleFromDue("monthly_last", "2026-11-19")).toEqual({ kind: "monthly_last" });
    expect(ruleFromDue("yearly", "2028-02-29")).toEqual({ kind: "yearly", month: 2, day: 29 });
    expect(ruleFromDue("every_days", "2026-11-19")).toEqual({ kind: "every_days", days: 14 });
  });

  it("takes the week of the month, the last one from the 29th", () => {
    const week = (due: string) => {
      const r = ruleFromDue("monthly_weekday", due);
      return r.kind === "monthly_weekday" ? r.week : null;
    };
    expect(week("2026-11-01")).toBe(1);
    expect(week("2026-11-07")).toBe(1);
    expect(week("2026-11-08")).toBe(2);
    expect(week("2026-11-28")).toBe(4);
    expect(week("2026-11-29")).toBe(-1);
    expect(week("2026-12-31")).toBe(-1);
  });
});

describe("the dialog's inputs", () => {
  it("toggles a day and keeps the week's order", () => {
    expect(toggledWeekday(["thu"], "mon")).toEqual(["mon", "thu"]);
    expect(toggledWeekday(["mon", "thu"], "mon")).toEqual(["thu"]);
    expect(toggledWeekday([], "sun")).toEqual(["sun"]);
  });

  it("sends 0 for days that are not a whole number, for Rust to refuse", () => {
    expect(everyDaysOf("10")).toBe(10);
    expect(everyDaysOf(" 365 ")).toBe(365);
    expect(everyDaysOf("1")).toBe(1);
    expect(everyDaysOf("")).toBe(0);
    expect(everyDaysOf("2.5")).toBe(0);
    expect(everyDaysOf("-3")).toBe(0);
    expect(everyDaysOf("十")).toBe(0);
    expect(everyDaysOf("99999999999")).toBe(0);
  });

  it("notes the days some months or years do not have", () => {
    expect(monthEndNote(28)).toBeNull();
    expect(monthEndNote(31)).toBe("その日がない月は、月末にします。");
    expect(leapDayNote(2, 29)).toBe("うるう年でない年は、2/28にします。");
    expect(leapDayNote(2, 28)).toBeNull();
    expect(leapDayNote(3, 29)).toBeNull();
  });
});
