import type { KeyboardEvent } from "react";

/**
 * True while an IME is composing (Japanese input confirms a conversion with
 * Enter). Such an Enter must not submit anything.
 */
export function isComposing(e: KeyboardEvent): boolean {
  return e.nativeEvent.isComposing || e.keyCode === 229;
}
