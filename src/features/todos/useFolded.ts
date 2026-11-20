import { useCallback, useSyncExternalStore } from "react";
import { toggled } from "./tree";

// Which parents are folded (S-1). Kept while the app is open (not saved), and
// shared by the TODO list and the Gantt chart: a parent folded in one is
// folded in the other. Every parent starts unfolded.

let folded: ReadonlySet<number> = new Set();
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function snapshot(): ReadonlySet<number> {
  return folded;
}

/** The folded parents, and a function that folds or unfolds one. */
export function useFolded(): { folded: ReadonlySet<number>; toggle: (id: number) => void } {
  const current = useSyncExternalStore(subscribe, snapshot);
  const toggle = useCallback((id: number) => {
    folded = toggled(folded, id);
    for (const l of listeners) l();
  }, []);
  return { folded: current, toggle };
}
