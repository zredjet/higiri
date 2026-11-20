import { describe, expect, it } from "vitest";
import type { Assessment, GanttRow, Todo } from "../../lib/bindings";
import { dragText, dueMarkText, rowLabel } from "./rowLabel";

function row(todo: Partial<Todo>, assessment: Partial<Assessment> = {}, more: Partial<GanttRow> = {}): GanttRow {
  return {
    item: {
      todo: {
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
        ...todo,
      },
      project_name: "p",
      assessment: {
        urgency: null,
        warnings: { plan_misses_due: false, plan_overrun: false },
        days_until_due: null,
        days_since_planned_start: null,
        waiting_day: null,
        days_since_follow_up: null,
        business_days: false,
        due_shifted: false,
        ...assessment,
      },
      next_statuses: [],
      follow_up_default: "2026-11-22",
      parent_title: null,
      children: null,
      due_after_parent: false,
      repeat: null,
      blocked_by: [],
      starts_before_dependency: false,
    },
    planned: null,
    actual: null,
    waiting: [],
    child: false,
    bridge: false,
    depends_on: [],
    ...more,
  };
}

describe("rowLabel", () => {
  it("names a child with its parent and warns about its due date", () => {
    const child = row({ title: "子", due_date: "2026-11-25" }, {}, { child: true });
    child.item.parent_title = "親";
    child.item.due_after_parent = true;
    expect(rowLabel(child)).toBe("親 / 子:予定なし、実績なし、日限 11/25(親の日限より後)");
  });

  it("ends with the unfinished TODOs it waits for", () => {
    const later = row({ title: "結合テスト計画の作成" });
    later.item.blocked_by = [
      { id: 2, title: "移行手順書のドラフト", parent_title: null, project_name: "基幹システム更改" },
      { id: 3, title: "手順書", parent_title: "開発環境の標準化", project_name: "社内ツール整備" },
    ];
    expect(rowLabel(later)).toBe(
      "結合テスト計画の作成:予定なし、実績なし、日限なし、先行TODOが未完了(移行手順書のドラフト、開発環境の標準化 / 手順書)",
    );
  });

  it("ends with the repetition of a repeating TODO", () => {
    const weekly = row({ title: "定例の資料", due_date: "2026-11-23" });
    weekly.item.repeat = { rule: { kind: "weekly", weekdays: ["mon"] }, until_date: null, next_due: "2026-11-30" };
    expect(rowLabel(weekly)).toBe("定例の資料:予定なし、実績なし、日限 11/23、繰り返し 毎週 月");
  });

  it("reads a bridge row as a hidden parent", () => {
    expect(rowLabel(row({ title: "親", status: "done" }, {}, { bridge: true }))).toBe(
      "親:完了(子TODOだけを表示しています)",
    );
  });

  it("matches the mock's labels", () => {
    expect(
      rowLabel(
        row(
          {
            title: "移行手順書のドラフト",
            status: "doing",
            planned_start: "2026-11-11",
            planned_end: "2026-11-18",
            actual_start: "2026-11-12",
            due_date: "2026-11-19",
          },
          { urgency: "due_today", days_until_due: 0 },
        ),
      ),
    ).toBe("移行手順書のドラフト:予定 11/11〜11/18、実績 11/12〜、日限 11/19(今日)");

    expect(
      rowLabel(
        row({
          title: "API設計書のレビュー対応",
          status: "waiting",
          planned_start: "2026-11-16",
          planned_end: "2026-11-19",
          actual_start: "2026-11-17",
          waiting_since: "2026-11-19",
          due_date: "2026-11-20",
        }),
      ),
    ).toBe("API設計書のレビュー対応:予定 11/16〜11/19、実績 11/17〜(11/19から待ち)、日限 11/20");

    expect(
      rowLabel(
        row(
          { title: "ライセンス棚卸し", planned_start: "2026-11-12", planned_end: "2026-11-13", due_date: "2026-11-17" },
          { urgency: "overdue", days_until_due: -2 },
        ),
      ),
    ).toBe("ライセンス棚卸し:予定 11/12〜11/13、実績なし、日限 11/17(2日超過)");
  });

  it("says when nothing is planned", () => {
    expect(rowLabel(row({ title: "x" }))).toBe("x:予定なし、実績なし、日限なし");
  });

  it("words business days and a due date moved off a holiday", () => {
    expect(
      rowLabel(
        row({ title: "x", due_date: "2026-11-17" }, { urgency: "overdue", days_until_due: -2, business_days: true }),
      ),
    ).toBe("x:予定なし、実績なし、日限 11/17(2営業日超過)");
    expect(
      rowLabel(
        row(
          { title: "x", due_date: "2026-11-22" },
          { urgency: "due_today", days_until_due: 0, business_days: true, due_shifted: true },
        ),
      ),
    ).toBe("x:予定なし、実績なし、日限 11/22(今日、休日のため前倒し)");
    expect(
      rowLabel(
        row(
          { title: "x", due_date: "2026-11-23" },
          { urgency: null, days_until_due: 5, business_days: true, due_shifted: true },
        ),
      ),
    ).toBe("x:予定なし、実績なし、日限 11/23(休日のため前倒し)");
  });
});

describe("dueMarkText", () => {
  const a = (over: Partial<Assessment>) => row({}, over).item.assessment;

  it("shows the overdue days, as in the mock", () => {
    expect(dueMarkText(a({ urgency: "overdue", days_until_due: -2 }))).toBe("2日超過");
    expect(dueMarkText(a({ urgency: "due_today", days_until_due: 0 }))).toBeNull();
    expect(dueMarkText(a({ urgency: "due_soon", days_until_due: 1 }))).toBeNull();
  });

  it("adds 営業日 and 休日のため前倒し", () => {
    expect(dueMarkText(a({ urgency: "overdue", days_until_due: -1, business_days: true }))).toBe("1営業日超過");
    expect(dueMarkText(a({ urgency: "overdue", days_until_due: -1, business_days: true, due_shifted: true }))).toBe(
      "1営業日超過(休日のため前倒し)",
    );
    expect(dueMarkText(a({ urgency: "due_today", days_until_due: 0, business_days: true, due_shifted: true }))).toBe(
      "休日のため前倒し",
    );
    expect(dueMarkText(a({ urgency: "due_soon", days_until_due: 1, business_days: true, due_shifted: true }))).toBeNull();
  });
});

describe("dragText", () => {
  it("shows the span being dragged as in the design", () => {
    expect(dragText({ start: "2026-11-20", end: "2026-11-27" })).toBe("11/20〜11/27");
    expect(dragText({ start: "2026-12-31", end: "2027-01-04" })).toBe("12/31〜1/4");
    expect(dragText({ start: "2026-11-20", end: "2026-11-20" })).toBe("11/20〜11/20");
  });
});
