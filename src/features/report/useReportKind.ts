import { useCallback, useSyncExternalStore } from "react";
import type { ShownKind } from "./reportText";

// The kind the report screen shows (日報 or 週報, R-6). Kept while the app is
// open (not saved): moving to another screen and back opens the same kind.

let kind: ShownKind = "daily";
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function snapshot(): ShownKind {
  return kind;
}

export function useReportKind(): [ShownKind, (next: ShownKind) => void] {
  const current = useSyncExternalStore(subscribe, snapshot);
  const set = useCallback((next: ShownKind) => {
    if (next === kind) return;
    kind = next;
    for (const l of listeners) l();
  }, []);
  return [current, set];
}
