import type { Assessment, DaySpan, GanttRow } from "../../lib/bindings";
import { monthDay } from "../../lib/date";
import { BLOCKED_LABEL, blockingTitle } from "../todos/dependencyText";
import { REPEAT_LABEL, repeatRuleText } from "../todos/repeatText";
import { displayTitle, DUE_AFTER_PARENT_TEXT, overdueText, SHIFTED_TEXT, STATUS_LABEL } from "../todos/todoText";

/**
 * Accessible name of a chart row, as in the mock:
 * 「API設計書のレビュー対応:予定 11/16〜11/19、実績 11/17〜(11/19から待ち)、日限 11/20」.
 * In business days the counts read 「2営業日超過」, and a due date moved off a
 * holiday adds 「休日のため前倒し」. A child is named 「親 / 子」 and may add
 * 「親の日限より後」 (S-1); a bridge row (a hidden parent) has no bars to read.
 * A repeating TODO ends with 「繰り返し 毎週 月」 (T-12), and one waiting for
 * unfinished TODOs with 「先のTODOが未完了(要件の確定)」 (T-11: the arrows
 * themselves are not read).
 */
export function rowLabel(row: GanttRow): string {
  const { todo, assessment } = row.item;
  const name = displayTitle(row.item);
  if (row.bridge) return `${name}:${STATUS_LABEL[todo.status]}(子TODOだけを表示しています)`;
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
    if (row.item.due_after_parent) notes.push(DUE_AFTER_PARENT_TEXT);
    due = `日限 ${monthDay(todo.due_date)}${notes.length > 0 ? `(${notes.join("、")})` : ""}`;
  }
  const repeat = row.item.repeat ? `、${REPEAT_LABEL} ${repeatRuleText(row.item.repeat.rule)}` : "";
  const blocked =
    row.item.blocked_by.length > 0 ? `、${BLOCKED_LABEL}(${row.item.blocked_by.map(blockingTitle).join("、")})` : "";
  return `${name}:${planned}、${actual}、${due}${repeat}${blocked}`;
}

/** The dates shown while dragging a planned bar: 「11/20〜11/27」. */
export function dragText(span: DaySpan): string {
  return `${monthDay(span.start)}〜${monthDay(span.end)}`;
}

/**
 * Text drawn right of the due diamond:「2日超過」 for an overdue TODO, and
 * 「休日のため前倒し」 when the due date was moved off a holiday (overdue or today).
 */
export function dueMarkText(a: Assessment): string | null {
  if (a.urgency === "overdue" && a.days_until_due !== null) {
    return a.due_shifted ? `${overdueText(a)}(${SHIFTED_TEXT})` : overdueText(a);
  }
  if (a.urgency === "due_today" && a.due_shifted) return SHIFTED_TEXT;
  return null;
}
