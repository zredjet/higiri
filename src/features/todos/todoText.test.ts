import { describe, expect, it } from "vitest";
import type { Assessment, Todo } from "../../lib/bindings";
import { dueNote, periodText, reminderSide, situationText, subLine, waitingMessage, warningTexts } from "./todoText";

function assessment(over: Partial<Assessment> = {}): Assessment {
  return {
    urgency: null,
    warnings: { plan_misses_due: false, plan_overrun: false },
    days_until_due: null,
    days_since_planned_start: null,
    waiting_day: null,
    days_since_follow_up: null,
    business_days: false,
    due_shifted: false,
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

describe("dashboard and reminder texts", () => {
  it("words 状況 as the dashboard mock does", () => {
    expect(situationText(todo(), assessment({ urgency: "overdue", days_until_due: -2 }))).toBe("2日超過");
    expect(situationText(todo(), assessment({ urgency: "due_today", days_until_due: 0 }))).toBe("今日が日限");
    expect(
      situationText(todo({ status: "waiting" }), assessment({ urgency: "due_soon", days_until_due: 1 })),
    ).toBe("あと1日 ・ 待ち");
    expect(situationText(todo(), assessment({ urgency: "follow_up", days_since_follow_up: 0 }))).toBe("確認日が今日");
    expect(situationText(todo(), assessment({ urgency: "follow_up", days_since_follow_up: 2 }))).toBe(
      "確認日から2日経過",
    );
    expect(situationText(todo(), assessment({ urgency: "late_start", days_since_planned_start: 2 }))).toBe(
      "予定開始から2日経過",
    );
  });

  it("words the reminder rows as the reminder mock does", () => {
    expect(
      reminderSide(todo({ due_date: "2026-11-17" }), assessment({ urgency: "overdue", days_until_due: -2 })),
    ).toEqual({ label: "日限 11/17", detail: "2日超過", color: "var(--danger)", strong: true });
    expect(
      reminderSide(
        todo({ status: "waiting", due_date: "2026-11-20" }),
        assessment({ urgency: "due_soon", days_until_due: 1 }),
      ),
    ).toEqual({ label: "日限 11/20", detail: "あと1日 ・ 待ち", color: "var(--text-muted)", strong: false });
    expect(
      reminderSide(
        todo({ status: "waiting", follow_up_date: "2026-11-19" }),
        assessment({ urgency: "follow_up", waiting_day: 3 }),
      ),
    ).toEqual({ label: "確認日 11/19", detail: "待ち3日目", color: "var(--warning)", strong: true });
    expect(
      reminderSide(
        todo({ planned_start: "2026-11-17" }),
        assessment({ urgency: "late_start", days_since_planned_start: 2 }),
      ).detail,
    ).toBe("2日経過");
  });
});

describe("business days (営業日)", () => {
  const biz = (over: Partial<Assessment>) => assessment({ business_days: true, ...over });

  it("adds 営業日 to the day counts", () => {
    expect(dueNote(biz({ urgency: "overdue", days_until_due: -2 }))?.text).toBe("2営業日超過");
    expect(dueNote(biz({ urgency: "due_soon", days_until_due: 1 }))?.text).toBe("あと1営業日");
    expect(situationText(todo(), biz({ urgency: "late_start", days_since_planned_start: 1 }))).toBe(
      "予定開始から1営業日経過",
    );
    expect(
      reminderSide(todo({ due_date: "2026-11-17" }), biz({ urgency: "overdue", days_until_due: -2 })).detail,
    ).toBe("2営業日超過");
    expect(reminderSide(todo(), biz({ urgency: "late_start", days_since_planned_start: 3 })).detail).toBe(
      "3営業日経過",
    );
  });

  it("keeps the waiting counts in calendar days", () => {
    expect(situationText(todo(), biz({ urgency: "follow_up", days_since_follow_up: 2 }))).toBe("確認日から2日経過");
    expect(
      reminderSide(todo({ status: "waiting", follow_up_date: "2026-11-19" }), biz({ urgency: "follow_up", waiting_day: 3 }))
        .detail,
    ).toBe("待ち3日目");
  });

  it("adds 休日のため前倒し to a due date moved off a holiday", () => {
    const shifted = (over: Partial<Assessment>) => biz({ due_shifted: true, ...over });
    expect(dueNote(shifted({ urgency: "due_today", days_until_due: 0 }))?.text).toBe("今日が日限(休日のため前倒し)");
    expect(dueNote(shifted({ urgency: "overdue", days_until_due: -1 }))?.text).toBe("1営業日超過(休日のため前倒し)");
    expect(
      situationText(todo({ status: "waiting" }), shifted({ urgency: "due_soon", days_until_due: 1 })),
    ).toBe("あと1営業日 ・ 休日のため前倒し ・ 待ち");
    expect(
      reminderSide(todo({ due_date: "2026-11-22" }), shifted({ urgency: "due_today", days_until_due: 0 })),
    ).toEqual({ label: "日限 11/22", detail: "今日が日限 ・ 休日のため前倒し", color: "var(--warning)", strong: true });
    // 着手遅れ is not about the due date.
    expect(dueNote(shifted({ urgency: "late_start" }))?.text).toBe("着手遅れ");
  });

  it("words the waiting alert", () => {
    const waiting = todo({ status: "waiting", waiting_on: "認証担当の回答" });
    expect(waitingMessage(waiting, biz({ waiting_day: 4, days_until_due: 1 }))).toBe(
      "認証担当の回答を待っています。待ち4日目で、日限まであと1営業日です。",
    );
    expect(waitingMessage(waiting, biz({ waiting_day: 4, days_until_due: 0, due_shifted: true }))).toBe(
      "認証担当の回答を待っています。待ち4日目で、今日が日限(休日のため前倒し)です。",
    );
    expect(waitingMessage(waiting, biz({ waiting_day: 4, days_until_due: -2, due_shifted: true }))).toBe(
      "認証担当の回答を待っています。待ち4日目で、日限(休日のため前倒し)を2営業日過ぎています。",
    );
  });
});
