import { describe, expect, it } from "vitest";
import type { Assessment, Todo } from "../../lib/bindings";
import { dueNote, periodText, subLine, waitingMessage, warningTexts } from "./todoText";

function assessment(over: Partial<Assessment> = {}): Assessment {
  return {
    urgency: null,
    warnings: { plan_misses_due: false, plan_overrun: false },
    days_until_due: null,
    days_since_planned_start: null,
    waiting_day: null,
    ...over,
  };
}

function todo(over: Partial<Todo> = {}): Todo {
  return {
    id: 1,
    project_id: 1,
    parent_id: null,
    title: "t",
    status: "todo",
    priority: "medium",
    due_date: null,
    planned_start: null,
    planned_end: null,
    actual_start: null,
    actual_end: null,
    waiting_on: null,
    waiting_since: null,
    follow_up_date: null,
    closed_at: null,
    close_reason: null,
    memo_md: "",
    sort_order: 1,
    created_at: "2026-11-01T00:00:00.000Z",
    updated_at: "2026-11-01T00:00:00.000Z",
    ...over,
  };
}

describe("dueNote", () => {
  it("words each category as the mocks do", () => {
    expect(dueNote(assessment({ urgency: "overdue", days_until_due: -2 }))?.text).toBe("2日超過");
    expect(dueNote(assessment({ urgency: "due_today", days_until_due: 0 }))).toEqual({
      text: "今日が日限",
      icon: "alert-triangle",
      color: "var(--warning)",
    });
    expect(dueNote(assessment({ urgency: "due_soon", days_until_due: 1 }))).toEqual({
      text: "あと1日",
      icon: "bell",
      color: "var(--primary-text)",
    });
    expect(dueNote(assessment({ urgency: "late_start" }))).toEqual({
      text: "着手遅れ",
      icon: "alert-circle",
      color: "var(--text-muted)",
    });
    expect(dueNote(assessment({ urgency: "follow_up" }))?.text).toBe("要確認");
    expect(dueNote(assessment())).toBeNull();
  });
});

describe("list texts", () => {
  it("formats periods", () => {
    expect(periodText("予定", "2026-11-11", "2026-11-18")).toBe("予定 11/11〜11/18");
    expect(periodText("実績", "2026-11-12", null)).toBe("実績 11/12〜");
    expect(periodText("実績", null, null)).toBe("実績 なし");
  });

  it("lists warnings", () => {
    expect(warningTexts(assessment({ warnings: { plan_misses_due: true, plan_overrun: true } }))).toEqual([
      "計画が間に合わない",
      "予定超過",
    ]);
    expect(warningTexts(assessment())).toEqual([]);
  });

  it("shows who we wait for, or why it was cancelled", () => {
    expect(
      subLine(todo({ status: "waiting", waiting_on: "認証担当の回答", follow_up_date: "2026-11-20" })),
    ).toBe("認証担当の回答待ち ・ 確認日 11/20");
    expect(subLine(todo({ status: "cancelled", close_reason: "旧帳票の廃止が決まったため" }))).toBe(
      "中止の理由:旧帳票の廃止が決まったため",
    );
    expect(subLine(todo())).toBeNull();
  });
});

describe("waitingMessage", () => {
  const waiting = todo({ status: "waiting", waiting_on: "認証担当の回答" });

  it("matches the mock's alert", () => {
    expect(waitingMessage(waiting, assessment({ waiting_day: 1, days_until_due: 1 }))).toBe(
      "認証担当の回答を待っています。待ち1日目で、日限まであと1日です。",
    );
  });

  it("covers no due date, due today and overdue", () => {
    expect(waitingMessage(waiting, assessment({ waiting_day: 3 }))).toBe(
      "認証担当の回答を待っています。待ち3日目です。",
    );
    expect(waitingMessage(waiting, assessment({ waiting_day: 2, days_until_due: 0 }))).toBe(
      "認証担当の回答を待っています。待ち2日目で、今日が日限です。",
    );
    expect(waitingMessage(waiting, assessment({ waiting_day: 2, days_until_due: -3 }))).toBe(
      "認証担当の回答を待っています。待ち2日目で、日限を3日過ぎています。",
    );
    expect(waitingMessage(todo(), assessment())).toBeNull();
  });
});
