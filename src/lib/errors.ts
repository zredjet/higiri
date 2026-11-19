import type { AppError } from "./bindings";
import { logUnexpected } from "./log";

export function isAppError(e: unknown): e is AppError {
  return (
    typeof e === "object" &&
    e !== null &&
    "kind" in e &&
    "message" in e &&
    typeof (e as AppError).message === "string"
  );
}

/**
 * Text to show for a failed command. Commands reject with an AppError whose
 * message is already written for the user; anything else is a framework-level
 * failure, so show a generic message and keep the details in the console and
 * the log file.
 */
export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  console.error(e);
  logUnexpected("command failed", e);
  return "操作を完了できませんでした。アプリを再起動してから、もう一度操作してください。";
}
