// Which rows of a long TODO list to draw (非機能要件「性能」: 5,000 TODOs).
// Rows differ in height (sub-lines, warnings), so heights are measured once a
// row has been drawn and estimated until then.

/** Lists up to this length are drawn in full. */
export const VIRTUAL_FROM = 200;
/** Height of a row not drawn yet (two-line period, one-line title). */
export const ESTIMATED_ROW = 65;
/** Rows drawn above and below what is on screen. */
export const OVERSCAN = 10;

export interface RowWindow {
  /** First row drawn. */
  first: number;
  /** One past the last row drawn. */
  end: number;
  /** Space for the rows above / below the drawn ones, in px. */
  before: number;
  after: number;
}

/**
 * The rows that meet the visible band `[top, bottom)` (px from the top of
 * the list), plus `overscan` rows on each side.
 */
export function rowWindow(heights: number[], top: number, bottom: number, overscan: number = OVERSCAN): RowWindow {
  const n = heights.length;
  if (n === 0) return { first: 0, end: 0, before: 0, after: 0 };
  let y = 0;
  let first = 0;
  while (first < n - 1 && y + heights[first] <= top) {
    y += heights[first];
    first++;
  }
  let end = first;
  let yEnd = y;
  while (end < n && yEnd < bottom) {
    yEnd += heights[end];
    end++;
  }
  end = Math.max(end, first + 1);
  const from = Math.max(0, first - overscan);
  const to = Math.min(n, end + overscan);
  let before = 0;
  for (let i = 0; i < from; i++) before += heights[i];
  let after = 0;
  for (let i = to; i < n; i++) after += heights[i];
  return { first: from, end: to, before, after };
}
