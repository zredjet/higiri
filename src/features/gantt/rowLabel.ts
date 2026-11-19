import type { GanttRow } from "../../lib/bindings";
import { monthDay } from "../../lib/date";

/**
 * Accessible name of a chart row, as in the mock:
 * 「API設計書のレビュー対応:予定 11/16〜11/19、実績 11/17〜(11/19から待ち)、日限 11/20」.
 */
export function rowLabel(row: GanttRow): string {
  const { todo, assessment } = row.item;
  const planned =
    todo.planned_start || todo.planned_end
      ? `予定 ${todo.planned_start ? monthDay(todo.planned_start) : ""}〜${todo.planned_end ? monthDay(todo.planned_end) : ""}`
      : "予定なし";
  const waitingFrom = todo.status === "waiting" && todo.waiting_since ? `(${monthDay(todo.waiting_since)}から待ち)` : "";
  const actual = todo.actual_start
    ? `実績 ${monthDay(todo.actual_start)}〜${todo.actual_end ? monthDay(todo.actual_end) : ""}${waitingFrom}`
    : "実績なし";
  let due = "日限なし";
  if (todo.due_date) {
    const days = assessment.days_until_due;
    const note =
      assessment.urgency === "due_today" ? "(今日)" : assessment.urgency === "overdue" && days !== null ? `(${-days}日超過)` : "";
    due = `日限 ${monthDay(todo.due_date)}${note}`;
  }
  return `${todo.title}:${planned}、${actual}、${due}`;
}
