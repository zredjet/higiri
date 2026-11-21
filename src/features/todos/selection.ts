// 一括変更 (T-14): which TODOs of the list are selected. The list is the one
// Rust sends (filters, the status tab, the page) folded as drawn (tree.ts):
// bridge rows and the children of a folded parent cannot be selected, and
// rows not drawn because they are far from the screen (useVirtualRows) can.
// What changes and why some are left out comes from Rust
// (preview_bulk_change); this file only keeps the selection.
import type { TodoListRow } from "../../lib/bindings";
import { foldRows, type TreeInfo } from "./tree";

/** Selected TODO ids, and the row a Shift+click counts from. */
export interface Selection {
  ids: ReadonlySet<number>;
  /** The row clicked last. */
  anchor: number | null;
}

export const NO_SELECTION: Selection = { ids: new Set(), anchor: null };

export const listTreeInfo = (r: TodoListRow): TreeInfo => ({
  id: r.item.todo.id,
  parentId: r.item.todo.parent_id,
  child: r.child,
});

/** The TODOs that can be selected, in the list's order. */
export function selectableIds(rows: TodoListRow[], folded: ReadonlySet<number>): number[] {
  return foldRows(rows, folded, listTreeInfo)
    .filter((r) => !r.bridge)
    .map((r) => r.item.todo.id);
}

/** The selected TODOs that are in the list, in its order (what is sent and counted). */
export function selectedIn(selection: Selection, selectable: readonly number[]): number[] {
  return selectable.filter((id) => selection.ids.has(id));
}

/**
 * A row's checkbox clicked: it flips. With Shift, every row from the row
 * clicked before to this one takes this one's new state.
 */
export function clicked(
  selection: Selection,
  selectable: readonly number[],
  id: number,
  shift: boolean,
): Selection {
  const on = !selection.ids.has(id);
  const ids = new Set(selection.ids);
  const from = shift && selection.anchor !== null ? selectable.indexOf(selection.anchor) : -1;
  const to = selectable.indexOf(id);
  if (from >= 0 && to >= 0) {
    for (const each of selectable.slice(Math.min(from, to), Math.max(from, to) + 1)) {
      if (on) ids.add(each);
      else ids.delete(each);
    }
  } else if (on) {
    ids.add(id);
  } else {
    ids.delete(id);
  }
  return { ids, anchor: id };
}

/** The header's checkbox: on, off, or some (`indeterminate`). */
export function headerState(selection: Selection, selectable: readonly number[]): "all" | "some" | "none" {
  const n = selectedIn(selection, selectable).length;
  if (n === 0) return "none";
  return n === selectable.length ? "all" : "some";
}

/** The header's checkbox clicked: everything selected is cleared, otherwise everything is selected. */
export function allToggled(selection: Selection, selectable: readonly number[]): Selection {
  if (headerState(selection, selectable) === "all") return NO_SELECTION;
  return { ids: new Set(selectable), anchor: selection.anchor };
}

/**
 * The selection without the TODOs no longer in the list (another filter,
 * tab or page, folded under a parent): what is not shown is not changed.
 * The same object when nothing goes.
 */
export function pruned(selection: Selection, selectable: readonly number[]): Selection {
  const shown = new Set(selectable);
  const gone = [...selection.ids].some((id) => !shown.has(id));
  const anchorGone = selection.anchor !== null && !shown.has(selection.anchor);
  if (!gone && !anchorGone) return selection;
  return {
    ids: new Set([...selection.ids].filter((id) => shown.has(id))),
    anchor: anchorGone ? null : selection.anchor,
  };
}
