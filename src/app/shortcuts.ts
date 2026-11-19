// App-wide keyboard shortcuts (design: キーボードショートカット). Esc, Ctrl+Enter
// and Ctrl+E belong to the panel and the editors.
import { isPrimaryModifier } from "../lib/keys";
import type { Route } from "./routes";

export type Shortcut = { kind: "new_todo" } | { kind: "go"; route: Route };

/** Ctrl+1〜4: ダッシュボード, TODO, ガント, 日報. */
const SCREENS: Record<string, Route> = {
  "1": { page: "dashboard" },
  "2": { page: "todos", projectId: null },
  "3": { page: "gantt" },
  "4": { page: "report" },
};

interface KeyPress {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** The shortcut for a key press (Ctrl on Windows, Command on macOS), or null. */
export function shortcutFor(e: KeyPress, mac?: boolean): Shortcut | null {
  if (!isPrimaryModifier(e, mac) || e.altKey || e.shiftKey) return null;
  if (e.key.toLowerCase() === "n") return { kind: "new_todo" };
  const route = SCREENS[e.key];
  return route ? { kind: "go", route } : null;
}
