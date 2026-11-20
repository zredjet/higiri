// Wording of TODO間の依存関係 (T-11). Which TODOs are unfinished and whether a
// plan starts too early come from Rust (TodoItem.blocked_by and
// starts_before_dependency); this file only words them.
import type { BlockingTodo, TodoItem } from "../../lib/bindings";

/** The mark next to a title, read aloud (the list, the Gantt chart). */
export const BLOCKED_LABEL = "先のTODOが未完了";

/** 「親 / 子」 for a child, the title otherwise. */
export function blockingTitle(b: BlockingTodo): string {
  return b.parent_title ? `${b.parent_title} / ${b.title}` : b.title;
}

/** The mark's tooltip: 「先のTODOが未完了:要件の確定、API設計」. */
export function blockedMarkTitle(blockedBy: BlockingTodo[]): string {
  return `${BLOCKED_LABEL}:${blockedBy.map(blockingTitle).join("、")}`;
}

/** True when going 作業中 asks first (T-11): it waits for an unfinished TODO. */
export function asksBeforeDoing(item: Pick<TodoItem, "blocked_by">): boolean {
  return item.blocked_by.length > 0;
}

export const BLOCKED_DIALOG_TITLE = "先に終わらせるTODOが終わっていません";

/** The question of the dialog, above the list of unfinished TODOs. */
export function blockedQuestion(count: number): string {
  return `次の${count}件が、まだ完了も中止もしていません。このまま作業中にしますか。`;
}

/** One line of the dialog's list: 「API設計(基幹システム更改)」. */
export function blockingLine(b: BlockingTodo): string {
  return `${blockingTitle(b)}(${b.project_name})`;
}

// ---- 詳細パネル ----------------------------------------------------------------

export const DEPENDS_ON_LABEL = "先に終わらせるTODO";
export const DEPENDENTS_LABEL = "これを待っているTODO";
export const ADD_DEPENDENCY = "先に終わらせるTODOを足す";

export const NO_DEPENDS_ON_TEXT = "ありません。先に終わらせたいTODOがあれば足してください。";
export const NO_DEPENDENTS_TEXT = "このTODOを待っているTODOはありません。";

/** The warning in the detail panel when the plan starts before a TODO it waits for ends. */
export const STARTS_BEFORE_ALERT =
  "予定開始日が、先に終わらせるTODOの予定終了日より前になっています。予定を見直してください。";

/** Accessible name and tooltip of the × of a row (either list). */
export function removeLabel(title: string): string {
  return `「${title}」との依存を外す`;
}

// ---- 選択のダイアログ ----------------------------------------------------------

export const PICKER_DESCRIPTION = "語を入れてTODOを探し、選んだTODOを先に終わらせるTODOに足します。プロジェクトをまたいでも選べます。";

/** Why a search result cannot be picked, or null when it can. */
export function pickBlockedReason(id: number, self: number, joined: ReadonlySet<number>): string | null {
  if (id === self) return "このTODO";
  if (joined.has(id)) return "足してあります";
  return null;
}
