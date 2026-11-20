// サブTODO (S-1): folding parents in the TODO list and the Gantt chart.
// Rust sends the rows in tree order (a parent, then its children); folding
// only leaves the children of a folded parent out of what is drawn. The
// counts of the tabs do not change.

/** Where a row sits in the tree. */
export interface TreeInfo {
  /** The TODO of the row. */
  id: number;
  /** Its parent, for a child row. */
  parentId: number | null;
  /** Drawn indented under its parent. */
  child: boolean;
}

/** The rows to draw: the children of a folded parent are left out. */
export function foldRows<R>(rows: R[], folded: ReadonlySet<number>, info: (row: R) => TreeInfo): R[] {
  if (folded.size === 0) return rows;
  return rows.filter((r) => {
    const t = info(r);
    return !(t.child && t.parentId !== null && folded.has(t.parentId));
  });
}

/** Parents that have child rows in the list: they get the fold button. */
export function parentsWithChildRows<R>(rows: R[], info: (row: R) => TreeInfo): Set<number> {
  const out = new Set<number>();
  for (const r of rows) {
    const t = info(r);
    if (t.child && t.parentId !== null) out.add(t.parentId);
  }
  return out;
}

/** The set with `id` folded or unfolded. */
export function toggled(folded: ReadonlySet<number>, id: number): Set<number> {
  const next = new Set(folded);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}
