import { useCallback, useSyncExternalStore } from "react";
import { NO_SELECTION, type Selection } from "./selection";

// The TODOs selected in the TODO list (T-14). Kept while the app is open (not
// saved), like the folded parents: going to the dashboard and back keeps it;
// another page drops what it does not list (TodosPage prunes it).

let selection: Selection = NO_SELECTION;
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function snapshot(): Selection {
  return selection;
}

/** The selection, and a function that changes it (a new value or one made from the current). */
export function useSelection(): {
  selection: Selection;
  setSelection: (next: Selection | ((current: Selection) => Selection)) => void;
} {
  const current = useSyncExternalStore(subscribe, snapshot);
  const setSelection = useCallback((next: Selection | ((current: Selection) => Selection)) => {
    const value = typeof next === "function" ? next(selection) : next;
    if (value === selection) return;
    selection = value;
    for (const l of listeners) l();
  }, []);
  return { selection: current, setSelection };
}
