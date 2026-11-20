import { describe, expect, it } from "vitest";
import { rowWindow } from "./virtualRows";
import { foldRows, parentsWithChildRows, toggled, type TreeInfo } from "./tree";

const rows: TreeInfo[] = [
  { id: 1, parentId: null, child: false },
  { id: 2, parentId: 1, child: true },
  { id: 3, parentId: 1, child: true },
  { id: 4, parentId: null, child: false },
  { id: 5, parentId: null, child: false },
  { id: 6, parentId: 5, child: true },
];
const ids = (r: TreeInfo[]) => r.map((t) => t.id);
const self = (t: TreeInfo) => t;

describe("foldRows", () => {
  it("draws every row when nothing is folded", () => {
    expect(foldRows(rows, new Set(), self)).toBe(rows);
  });

  it("leaves out the children of folded parents only", () => {
    expect(ids(foldRows(rows, new Set([1]), self))).toEqual([1, 4, 5, 6]);
    expect(ids(foldRows(rows, new Set([1, 5]), self))).toEqual([1, 4, 5]);
    // Folding a TODO without children changes nothing.
    expect(ids(foldRows(rows, new Set([4]), self))).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it("keeps a row listed as a parent even if its parent id is folded", () => {
    // A child whose parent is not in the list comes as a parent row (child: false).
    const loose: TreeInfo[] = [{ id: 7, parentId: 1, child: false }];
    expect(ids(foldRows(loose, new Set([1]), self))).toEqual([7]);
  });

  it("works with the rows a long list draws", () => {
    // 300 parents with 2 children each; fold every other parent.
    const many: TreeInfo[] = [];
    for (let p = 0; p < 300; p++) {
      const id = p * 3 + 1;
      many.push({ id, parentId: null, child: false });
      many.push({ id: id + 1, parentId: id, child: true });
      many.push({ id: id + 2, parentId: id, child: true });
    }
    const folded = new Set(many.filter((t) => !t.child && t.id % 2 === 1).map((t) => t.id));
    const shown = foldRows(many, folded, self);
    expect(shown.length).toBe(300 + 2 * 150);
    // The drawn window is taken from the folded rows.
    const w = rowWindow(
      shown.map(() => 50),
      0,
      500,
    );
    expect(w.first).toBe(0);
    expect(w.end).toBe(20);
    expect(w.after).toBe((shown.length - 20) * 50);
  });
});

describe("parentsWithChildRows", () => {
  it("finds the parents whose children are listed", () => {
    expect([...parentsWithChildRows(rows, self)]).toEqual([1, 5]);
    expect(parentsWithChildRows([rows[0], rows[3]], self).size).toBe(0);
  });
});

describe("toggled", () => {
  it("folds and unfolds without changing the given set", () => {
    const before = new Set([1]);
    expect([...toggled(before, 5)]).toEqual([1, 5]);
    expect([...toggled(before, 1)]).toEqual([]);
    expect([...before]).toEqual([1]);
  });
});
