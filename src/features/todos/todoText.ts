// Labels and display text for TODOs. The numbers (days) come from Rust's
// Assessment; this file only words them as the mocks do. Whether they are
// business days (「営業日」) and whether the due date was moved off a holiday
// (「休日のため前倒し」) also come from the Assessment.
import type { BadgeTone, IconName } from "../../components/aizome";
import type { Assessment, ChildCounts, Priority, Status, Todo, Urgency } from "../../lib/bindings";
import { monthDay } from "../../lib/date";

export const STATUS_LABEL: Record<Status, string> = {
  todo: "未着手",
  doing: "作業中",
  waiting: "待ち",
  hold: "保留",
  done: "完了",
  cancelled: "中止",
};

/** Badge tones from docs/ui-mock/README.md. */
export const STATUS_TONE: Record<Status, BadgeTone> = {
  todo: "neutral",
  doing: "primary",
  waiting: "warning",
  hold: "neutral",
  done: "success",
  cancelled: "neutral",
};

export const STATUSES: Status[] = ["todo", "doing", "waiting", "hold", "done", "cancelled"];

export const PRIORITY_LABEL: Record<Priority, string> = {
  high: "高",
  medium: "中",
  low: "低",
};

export const PRIORITIES: Priority[] = ["high", "medium", "low"];

export const URGENCY_LABEL: Record<Urgency, string> = {
  overdue: "超過",
  due_today: "本日",
  due_soon: "間近",
  follow_up: "要確認",
  late_start: "着手遅れ",
};

export const URGENCY_TONE: Record<Urgency, BadgeTone> = {
  overdue: "danger",
  due_today: "warning",
  due_soon: "primary",
  follow_up: "warning",
  late_start: "neutral",
};

/** Text colour token for a tone (the line under the due date). */
const TONE_COLOR: Record<BadgeTone, string> = {
  danger: "var(--danger)",
  warning: "var(--warning)",
  primary: "var(--primary-text)",
  success: "var(--success)",
  neutral: "var(--text-muted)",
};

const URGENCY_ICON: Record<Urgency, IconName> = {
  overdue: "alert-circle",
  due_today: "alert-triangle",
  due_soon: "bell",
  follow_up: "bell",
  late_start: "alert-circle",
};

export interface Note {
  text: string;
  icon: IconName;
  color: string;
}

/** Added when a due date on a holiday was judged as the business day before it. */
export const SHIFTED_TEXT = "休日のため前倒し";

/** 「日」, or 「営業日」 when Rust counted business days. */
export function dayUnit(a: Assessment): string {
  return a.business_days ? "営業日" : "日";
}

/** 「2日超過」「2営業日超過」 (days_until_due is negative when overdue). */
export function overdueText(a: Assessment): string {
  return `${-(a.days_until_due ?? 0)}${dayUnit(a)}超過`;
}

/** 「あと1日」「あと1営業日」. */
export function daysLeftText(a: Assessment): string {
  return `あと${a.days_until_due ?? 0}${dayUnit(a)}`;
}

/** The 日限 categories carry the 前倒し note; the others are not about the due date. */
function dueCategory(a: Assessment): boolean {
  return a.urgency === "overdue" || a.urgency === "due_today" || a.urgency === "due_soon";
}

/** The line under the due date in the list (「今日が日限」「あと1日」「着手遅れ」). */
export function dueNote(a: Assessment): Note | null {
  if (!a.urgency) return null;
  const text =
    a.urgency === "overdue"
      ? overdueText(a)
      : a.urgency === "due_today"
        ? "今日が日限"
        : a.urgency === "due_soon"
          ? daysLeftText(a)
          : URGENCY_LABEL[a.urgency];
  const shifted = a.due_shifted && dueCategory(a) ? `(${SHIFTED_TEXT})` : "";
  return { text: text + shifted, icon: URGENCY_ICON[a.urgency], color: TONE_COLOR[URGENCY_TONE[a.urgency]] };
}

/** A child due after its parent (S-1, like 計画が間に合わない). */
export const DUE_AFTER_PARENT_TEXT = "親の日限より後";

/** A plan that starts before a TODO it waits for is planned to end (T-11). */
export const STARTS_BEFORE_DEPENDENCY_TEXT = "先行TODOより前に始まる予定";

/**
 * The small warnings of the list (計画が間に合わない, 予定超過, for a child
 * 親の日限より後, and 先行TODOより前に始まる予定).
 */
export function warningTexts(a: Assessment, dueAfterParent = false, startsBeforeDependency = false): string[] {
  const out: string[] = [];
  if (a.warnings.plan_misses_due) out.push("計画が間に合わない");
  if (a.warnings.plan_overrun) out.push("予定超過");
  if (dueAfterParent) out.push(DUE_AFTER_PARENT_TEXT);
  if (startsBeforeDependency) out.push(STARTS_BEFORE_DEPENDENCY_TEXT);
  return out;
}

// ---- サブTODO (S-1) ----------------------------------------------------------

/** 「親 / 子」 for a child (dashboard, reminder, search), the title otherwise. */
export function displayTitle(item: { todo: Todo; parent_title: string | null }): string {
  return item.parent_title ? `${item.parent_title} / ${item.todo.title}` : item.todo.title;
}

/** 「子 2/3」 next to a parent's title (中止 not counted). null when no child counts. */
export function childProgressText(c: ChildCounts | null): string | null {
  return c && c.total > 0 ? `子 ${c.done}/${c.total}` : null;
}

/** 「2/3 完了」 in the detail panel's 子TODO section. */
export function childDoneText(c: ChildCounts): string {
  return `${c.done}/${c.total} 完了`;
}

/** Shown when every child is 完了 or 中止 and the parent is still open. */
export const CHILDREN_ALL_CLOSED = "子TODOはすべて終わりました。";

/** Asked before 完了 when the parent has open children. */
export function closeChildrenQuestion(open: number): string {
  return `開いている子TODOが${open}件あります。一緒に完了にしますか。`;
}

/** Added to the 中止 dialog when the parent has open children. */
export function cancelChildrenNote(open: number): string {
  return `開いている子TODO ${open}件も中止になります。`;
}

/** The delete confirmation: children go with their parent, and a repetition stops (T-12). */
export function deleteTodoText(title: string, children: number, repeats = false): string {
  const kids = children > 0 ? `子TODO ${children}件も削除されます。` : "";
  const repeat = repeats ? "繰り返しも止まります。" : "";
  return `「${title}」と、その作業ログ・変更履歴をすべて削除します。${kids}${repeat}削除したものは元に戻せません。不要になっただけなら、ステータスを「中止」にすると記録が残ります。`;
}

/** 「予定 11/11〜11/18」「実績 11/12〜」「実績 なし」. */
export function periodText(label: string, start: string | null, end: string | null): string {
  if (!start && !end) return `${label} なし`;
  return `${label} ${start ? monthDay(start) : ""}〜${end ? monthDay(end) : ""}`;
}

/** The muted line under the title (waiting partner, cancel reason). */
export function subLine(t: Todo): string | null {
  if (t.status === "waiting" && t.waiting_on) {
    const follow = t.follow_up_date ? ` ・ 確認日 ${monthDay(t.follow_up_date)}` : "";
    return `${t.waiting_on}待ち${follow}`;
  }
  if (t.status === "cancelled" && t.close_reason) return `中止の理由:${t.close_reason}`;
  return null;
}

/** The info alert of a waiting TODO in the detail panel. */
export function waitingMessage(t: Todo, a: Assessment): string | null {
  if (t.status !== "waiting" || !t.waiting_on) return null;
  const day = a.waiting_day ? `待ち${a.waiting_day}日目` : "待っています";
  const days = a.days_until_due;
  const unit = dayUnit(a);
  const shifted = a.due_shifted ? `(${SHIFTED_TEXT})` : "";
  const due =
    days === null
      ? "です。"
      : days > 0
        ? `で、日限${shifted}まであと${days}${unit}です。`
        : days === 0
          ? `で、今日が日限${shifted}です。`
          : `で、日限${shifted}を${-days}${unit}過ぎています。`;
  if (!a.waiting_day) return `${t.waiting_on}を待っています。`;
  return `${t.waiting_on}を待っています。${day}${due}`;
}

/**
 * 状況 on the dashboard (「2日超過」「あと1日 ・ 待ち」「確認日が今日」「予定開始から2日経過」).
 * The follow-up counts stay calendar days.
 */
export function situationText(t: Todo, a: Assessment): string {
  const waiting = t.status === "waiting" ? " ・ 待ち" : "";
  const shifted = a.due_shifted ? ` ・ ${SHIFTED_TEXT}` : "";
  switch (a.urgency) {
    case "overdue":
      return `${overdueText(a)}${shifted}${waiting}`;
    case "due_today":
      return `今日が日限${shifted}${waiting}`;
    case "due_soon":
      return `${daysLeftText(a)}${shifted}${waiting}`;
    case "follow_up":
      return (a.days_since_follow_up ?? 0) > 0 ? `確認日から${a.days_since_follow_up}日経過` : "確認日が今日";
    case "late_start":
      return `予定開始から${a.days_since_planned_start ?? 0}${dayUnit(a)}経過`;
    default:
      return "";
  }
}

/** Right side of a reminder row: the date it is about, and how it stands. */
export function reminderSide(t: Todo, a: Assessment): { label: string; detail: string; color: string; strong: boolean } {
  const waiting = t.status === "waiting" ? " ・ 待ち" : "";
  const shifted = a.due_shifted ? ` ・ ${SHIFTED_TEXT}` : "";
  const due = t.due_date ? `日限 ${monthDay(t.due_date)}` : "日限 なし";
  switch (a.urgency) {
    case "overdue":
      return { label: due, detail: `${overdueText(a)}${shifted}${waiting}`, color: "var(--danger)", strong: true };
    case "due_today":
      return { label: due, detail: `今日が日限${shifted}${waiting}`, color: "var(--warning)", strong: true };
    case "due_soon":
      return { label: due, detail: `${daysLeftText(a)}${shifted}${waiting}`, color: "var(--text-muted)", strong: false };
    case "follow_up":
      return {
        label: t.follow_up_date ? `確認日 ${monthDay(t.follow_up_date)}` : "確認日",
        detail: a.waiting_day ? `待ち${a.waiting_day}日目` : "待ち",
        color: "var(--warning)",
        strong: true,
      };
    case "late_start":
      return {
        label: t.planned_start ? `予定開始 ${monthDay(t.planned_start)}` : "予定開始",
        detail: `${a.days_since_planned_start ?? 0}${dayUnit(a)}経過`,
        color: "var(--text-muted)",
        strong: false,
      };
    default:
      return { label: due, detail: "", color: "var(--text-muted)", strong: false };
  }
}
