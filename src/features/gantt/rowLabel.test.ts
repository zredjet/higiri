import { describe, expect, it } from "vitest";
import type { Assessment, GanttRow, Todo } from "../../lib/bindings";
import { rowLabel } from "./rowLabel";

function row(todo: Partial<Todo>, assessment: Partial<Assessment> = {}): GanttRow {
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
        ...assessment,
      },
      next_statuses: [],
    },
    planned: null,
    actual: null,
    waiting: [],
  };
}

describe("rowLabel", () => {
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
});
