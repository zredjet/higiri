// Labels and display text for TODOs. The numbers (days) come from Rust's
// Assessment; this file only words them as the mocks do.
import type { BadgeTone, IconName } from "../../components/aizome";
import type { Assessment, Priority, Status, Todo, Urgency } from "../../lib/bindings";
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

/** The line under the due date in the list (「今日が日限」「あと1日」「着手遅れ」). */
export function dueNote(a: Assessment): Note | null {
  if (!a.urgency) return null;
  const days = a.days_until_due ?? 0;
  const text =
    a.urgency === "overdue"
      ? `${-days}日超過`
      : a.urgency === "due_today"
        ? "今日が日限"
        : a.urgency === "due_soon"
          ? `あと${days}日`
          : URGENCY_LABEL[a.urgency];
  return { text, icon: URGENCY_ICON[a.urgency], color: TONE_COLOR[URGENCY_TONE[a.urgency]] };
}

/** The small warnings of the list (計画が間に合わない, 予定超過). */
export function warningTexts(a: Assessment): string[] {
  const out: string[] = [];
  if (a.warnings.plan_misses_due) out.push("計画が間に合わない");
  if (a.warnings.plan_overrun) out.push("予定超過");
  return out;
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
  const due =
    days === null
      ? "です。"
      : days > 0
        ? `で、日限まであと${days}日です。`
        : days === 0
          ? "で、今日が日限です。"
          : `で、日限を${-days}日過ぎています。`;
  if (!a.waiting_day) return `${t.waiting_on}を待っています。`;
  return `${t.waiting_on}を待っています。${day}${due}`;
}
