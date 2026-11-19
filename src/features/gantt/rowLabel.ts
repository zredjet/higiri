import type { Assessment, GanttRow } from "../../lib/bindings";
import { monthDay } from "../../lib/date";
import { overdueText, SHIFTED_TEXT } from "../todos/todoText";

/**
 * Accessible name of a chart row, as in the mock:
 * 「API設計書のレビュー対応:予定 11/16〜11/19、実績 11/17〜(11/19から待ち)、日限 11/20」.
 * In business days the counts read 「2営業日超過」, and a due date moved off a
 * holiday adds 「休日のため前倒し」.
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
    const notes: string[] = [];
    if (assessment.urgency === "due_today") notes.push("今日");
    else if (assessment.urgency === "overdue" && assessment.days_until_due !== null) notes.push(overdueText(assessment));
    if (assessment.due_shifted) notes.push(SHIFTED_TEXT);
    due = `日限 ${monthDay(todo.due_date)}${notes.length > 0 ? `(${notes.join("、")})` : ""}`;
  }
  return `${todo.title}:${planned}、${actual}、${due}`;
}

/**
 * Text drawn right of the due diamond: 「2日超過」 for an overdue TODO, and
 * 「休日のため前倒し」 when the due date was moved off a holiday (overdue or today).
 */
export function dueMarkText(a: Assessment): string | null {
  if (a.urgency === "overdue" && a.days_until_due !== null) {
    return a.due_shifted ? `${overdueText(a)}(${SHIFTED_TEXT})` : overdueText(a);
  }
  if (a.urgency === "due_today" && a.due_shifted) return SHIFTED_TEXT;
  return null;
}
