// Wording of 一括変更 (T-14). What changes, what is left out and why, and
// what else a status change does come from Rust (preview_bulk_change and
// apply_bulk_change); this file only words them and reads the typed days.
import type { BulkChange, BulkResult, BulkTodo, Status } from "../../lib/bindings";
import { STATUS_LABEL } from "./todoText";

/** The four changes, one dialog each. */
export type BulkKind = "due" | "planned" | "priority" | "status";

export const BULK_KINDS: BulkKind[] = ["due", "planned", "priority", "status"];

/** The band's buttons and the dialogs' titles. */
export const BULK_TITLE: Record<BulkKind, string> = {
  due: "日限を変える",
  planned: "予定をずらす",
  priority: "優先度を変える",
  status: "ステータスを変える",
};

export const CLEAR_SELECTION = "選択を外す";

/** The band: 「3件を選択中」. */
export function selectingText(count: number): string {
  return `${count}件を選択中`;
}

/** Accessible names of the checkboxes. */
export const SELECT_ALL_LABEL = "すべて選ぶ";
export function selectLabel(title: string): string {
  return `「${title}」を選ぶ`;
}

// ---- 入力 ------------------------------------------------------------------------

export type DueMode = "set" | "shift";

export const DUE_MODES: { value: DueMode; label: string }[] = [
  { value: "set", label: "日付で決める" },
  { value: "shift", label: "N日ずらす" },
];

export const SHIFT_DAYS_NOTE = "マイナスの数で前へずらします。";

/**
 * The typed days: `null` while nothing is typed yet (empty, or only the
 * minus sign), the number when it is a whole number, and 0 otherwise (Rust
 * refuses 0 with its message). Full-width digits and minus signs are read
 * as half-width.
 */
export function daysOf(text: string): number | null {
  const t = text
    .trim()
    .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0))
    .replace(/^[−－ー]/, "-")
    .replace(/^＋/, "+");
  if (t === "" || t === "-" || t === "+") return null;
  if (!/^[-+]?\d+$/.test(t)) return 0;
  const n = Number(t);
  // Far out of range: Rust says the range.
  return Number.isSafeInteger(n) ? n : 9999;
}

// ---- 確認 ------------------------------------------------------------------------

/** 「親 / 子」 for a child, the title otherwise. */
export function bulkTodoTitle(t: Pick<BulkTodo, "title" | "parent_title">): string {
  return t.parent_title ? `${t.parent_title} / ${t.title}` : t.title;
}

/** The first line of the confirmation. */
export function targetsText(count: number): string {
  return count > 0 ? `${count}件を変えます。` : "変えられるTODOはありません。";
}

/** Above the list of what is left out. */
export function skippedText(count: number): string {
  return `次の${count}件は変えられないため除きます。`;
}

/** Above the TODOs going 作業中 that wait for unfinished ones. */
export const BLOCKED_TEXT = "先行TODOが終わっていないTODOがあります。このまま作業中にします。";

/** 「子TODO 4件も一緒に完了にします。」 */
export function closingChildrenText(count: number, to: Status): string {
  return `子TODO ${count}件も一緒に${STATUS_LABEL[to]}にします。`;
}

/** 「繰り返しのあるTODO 2件は、次の回を作ります。」 */
export function nextRoundsText(count: number): string {
  return `繰り返しのあるTODO ${count}件は、次の回を作ります。`;
}

/** The primary button: 「3件を変える」. */
export function applyLabel(count: number): string {
  return `${count}件を変える`;
}

// ---- 保存のあと ------------------------------------------------------------------

const DONE_WHAT: Record<BulkChange["kind"], string> = {
  set_due: "日限を変えました",
  shift_due: "日限を変えました",
  shift_planned: "予定をずらしました",
  priority: "優先度を変えました",
  status: "ステータスを変えました",
};

/** Above the list: 「3件の日限を変えました。変えられない2件は除きました。」 */
export function bulkDoneText(change: BulkChange, result: BulkResult): string {
  const done = `${result.changed.length}件の${DONE_WHAT[change.kind]}。`;
  return result.skipped.length > 0 ? `${done}変えられない${result.skipped.length}件は除きました。` : done;
}
