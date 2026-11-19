// Wording and small helpers of the search dialog. The words, the text around
// each hit and the order come from Rust (search_todos); nothing is counted or
// matched here.
import type { MatchField, SearchMatch } from "../../lib/bindings";
import { monthDay } from "../../lib/date";

/** Wait after the last key before searching (the design: 200ms). */
export const SEARCH_DELAY_MS = 200;

export const EMPTY_QUERY_TEXT = "タイトル、メモ、作業ログなどから探します。語を入れてください。";
export const NO_HITS_TEXT = "当てはまるTODOはありません。語を変えて検索してください。";
export const OVER_LIMIT_TEXT = "100件を超えました。語を足して絞ってください。";
export const AND_HINT = "空白で区切ると、すべての語を含むTODOを探します。";

const FIELD_LABEL: Record<MatchField, string> = {
  memo: "メモ",
  waiting_on: "待っている相手",
  close_reason: "中止の理由",
  work_log: "作業ログ",
};

/** Heading of a match: 「メモ」, 「作業ログ 11/19」. */
export function matchLabel(m: SearchMatch): string {
  const label = FIELD_LABEL[m.field];
  return m.field === "work_log" && m.log_date ? `${label} ${monthDay(m.log_date)}` : label;
}

/** 「3件」, or 「523件のうち100件」 when not all are listed. */
export function countText(shown: number, total: number): string {
  return shown < total ? `${total}件のうち${shown}件` : `${total}件`;
}

/** The selection after ↑ (-1) or ↓ (+1), kept inside the list. */
export function moveSelection(index: number, delta: number, length: number): number {
  if (length === 0) return 0;
  return Math.min(length - 1, Math.max(0, index + delta));
}
