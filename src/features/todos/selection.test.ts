import { describe, expect, it } from "vitest";
import type { TodoListRow } from "../../lib/bindings";
import { VIRTUAL_FROM } from "./virtualRows";
import {
  allToggled,
  clicked,
  headerState,
  NO_SELECTION,
  pruned,
  selectableIds,
  selectedIn,
  type Selection,
} from "./selection";

/** A list row with only what the selection reads. */
function row(id: number, opts: { parent?: number; child?: boolean; bridge?: boolean } = {}): TodoListRow {
  return {
    item: { todo: { id, parent_id: opts.parent ?? null } },
    child: opts.child ?? false,
    bridge: opts.bridge ?? false,
  } as unknown as TodoListRow;
}

// 1 (bridge: a parent not matching the filters) > 2, 3; 4; 5 > 6, 7; 8
const rows = [
  row(1, { bridge: true }),
  row(2, { parent: 1, child: true }),
  row(3, { parent: 1, child: true }),
  row(4),
  row(5),
  row(6, { parent: 5, child: true }),
  row(7, { parent: 5, child: true }),
  row(8),
];

const sel = (ids: number[], anchor: number | null = null): Selection => ({ ids: new Set(ids), anchor });
const sorted = (s: Selection) => [...s.ids].sort((a, b) => a - b);

describe("selectableIds", () => {
  it("leaves out bridge rows", () => {
    expect(selectableIds(rows, new Set())).toEqual([2, 3, 4, 5, 6, 7, 8]);
  });

  it("leaves out the children of a folded parent", () => {
    expect(selectableIds(rows, new Set([5]))).toEqual([2, 3, 4, 5, 8]);
    // The bridge's children fold too.
    expect(selectableIds(rows, new Set([1, 5]))).toEqual([4, 5, 8]);
  });

  it("counts every row of a long list, drawn near the screen or not", () => {
    const long = Array.from({ length: VIRTUAL_FROM + 50 }, (_, i) => row(i + 1));
    expect(selectableIds(long, new Set())).toHaveLength(VIRTUAL_FROM + 50);
  });
});

describe("clicked", () => {
  const all = selectableIds(rows, new Set());

  it("flips one row and remembers it", () => {
    const one = clicked(NO_SELECTION, all, 4, false);
    expect(sorted(one)).toEqual([4]);
    expect(one.anchor).toBe(4);
    const none = clicked(one, all, 4, false);
    expect(sorted(none)).toEqual([]);
    expect(none.anchor).toBe(4);
  });

  it("selects the rows between with Shift, either way", () => {
    const from = clicked(NO_SELECTION, all, 3, false);
    expect(sorted(clicked(from, all, 6, true))).toEqual([3, 4, 5, 6]);
    const up = clicked(NO_SELECTION, all, 7, false);
    expect(sorted(clicked(up, all, 4, true))).toEqual([4, 5, 6, 7]);
  });

  it("gives the rows between the clicked row's new state", () => {
    // 3〜7 selected, then 6 clicked first and 4 with Shift: 4〜6 go off.
    const s = clicked(sel([3, 4, 5, 6, 7], 3), all, 6, false);
    expect(sorted(s)).toEqual([3, 4, 5, 7]);
    // 4 was on: it goes off, and so do 5 and 6.
    expect(sorted(clicked(s, all, 4, true))).toEqual([3, 7]);
  });

  it("flips one row when the earlier row is no longer in the list", () => {
    expect(sorted(clicked(sel([], 99), all, 4, true))).toEqual([4]);
    expect(sorted(clicked(NO_SELECTION, all, 4, true))).toEqual([4]);
  });

  it("does not select folded children between", () => {
    const folded = selectableIds(rows, new Set([5]));
    expect(sorted(clicked(sel([], 4), folded, 8, true))).toEqual([4, 5, 8]);
  });
});

describe("headerState and allToggled", () => {
  const all = selectableIds(rows, new Set());

  it("says none, some or all of the list", () => {
    expect(headerState(NO_SELECTION, all)).toBe("none");
    expect(headerState(sel([2, 4]), all)).toBe("some");
    expect(headerState(sel(all), all)).toBe("all");
    expect(headerState(NO_SELECTION, [])).toBe("none");
  });

  it("selects every row of the list, then clears", () => {
    const every = allToggled(sel([4]), all);
    expect(sorted(every)).toEqual(all);
    expect(sorted(allToggled(every, all))).toEqual([]);
  });

  it("selects the folded list only", () => {
    expect(sorted(allToggled(NO_SELECTION, selectableIds(rows, new Set([5]))))).toEqual([2, 3, 4, 5, 8]);
  });
});

describe("pruned and selectedIn", () => {
  const all = selectableIds(rows, new Set());

  it("keeps the same selection when everything is still listed", () => {
    const s = sel([2, 8], 8);
    expect(pruned(s, all)).toBe(s);
  });

  it("drops the TODOs no longer in the list", () => {
    // Another tab: only 4 and 8 are listed now.
    const s = pruned(sel([2, 4, 8], 2), [4, 8]);
    expect(sorted(s)).toEqual([4, 8]);
    expect(s.anchor).toBeNull();
    // Folded under 5.
    expect(sorted(pruned(sel([5, 6, 7]), selectableIds(rows, new Set([5]))))).toEqual([5]);
  });

  it("lists the selected TODOs in the list's order", () => {
    expect(selectedIn(sel([8, 2, 99, 5]), all)).toEqual([2, 5, 8]);
  });
});
