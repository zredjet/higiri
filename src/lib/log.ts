// Writes to the operation log file (tauri-plugin-log). Only events and
// unexpected failures go there, never TODO contents.
import { error as writeError, info as writeInfo } from "@tauri-apps/plugin-log";

/** Errors already written, so a render loop does not repeat them. */
const loggedObjects = new WeakSet<object>();
const loggedTexts = new Set<string>();

function describe(e: unknown): string {
  if (e instanceof Error) return `${e.name}: ${e.message}`;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

/** Logs an unexpected failure once. Never throws (the log may be unavailable, e.g. in `npm run dev:web`). */
export function logUnexpected(where: string, e: unknown): void {
  if (typeof e === "object" && e !== null) {
    if (loggedObjects.has(e)) return;
    loggedObjects.add(e);
  }
  const line = `${where}: ${describe(e)}`;
  if (typeof e !== "object" || e === null) {
    if (loggedTexts.has(line)) return;
    if (loggedTexts.size > 100) loggedTexts.clear();
    loggedTexts.add(line);
  }
  writeError(line).catch(() => undefined);
}

export function logInfo(line: string): void {
  writeInfo(line).catch(() => undefined);
}

/** Errors no handler caught (a failed event handler, a rejected promise nobody awaited). */
export function installGlobalErrorLogging(): void {
  window.addEventListener("error", (ev) => logUnexpected("uncaught error", ev.error ?? ev.message));
  window.addEventListener("unhandledrejection", (ev) => logUnexpected("unhandled rejection", ev.reason));
}
