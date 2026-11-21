// Wording of 後続TODOのずらし (G-8). Which TODOs move, to which dates and by
// how many days come from Rust (preview_dependent_shift); this file only
// words them.
import type { DependentShift } from "../../lib/bindings";
import { monthDay } from "../../lib/date";

export const SHIFT_DIALOG_TITLE = "後続TODOの予定もずらしますか";

/** The question above the list of 後続TODO. */
export function shiftQuestion(count: number): string {
  return `予定を後ろへ動かしたため、次の${count}件の予定開始が、先行TODOの予定終了より前になります。後続TODOもずらすと、足りない日数だけ後ろへ動かします。`;
}

/** Saves the dragged TODO only (update_todo). */
export const SHIFT_ONLY_THIS = "このTODOだけ動かす";
/** Saves it with its 後続TODO (shift_with_dependents). The primary button. */
export const SHIFT_WITH_DEPENDENTS = "後続もずらす";

/** 「親 / 子」 for a child, the title otherwise. */
export function shiftTitle(s: Pick<DependentShift, "title" | "parent_title">): string {
  return s.parent_title ? `${s.parent_title} / ${s.title}` : s.title;
}

/** 「11/22〜11/24」, or 「11/22〜」 with no 予定終了. */
function spanText(start: string, end: string | null): string {
  return `${monthDay(start)}〜${end ? monthDay(end) : ""}`;
}

/** 「11/22〜11/24 → 11/25〜11/27(3日後ろへ)」 */
export function shiftDatesText(s: DependentShift): string {
  return `${spanText(s.planned_start, s.planned_end)} → ${spanText(s.shifted_start, s.shifted_end)}(${s.days}日後ろへ)`;
}
