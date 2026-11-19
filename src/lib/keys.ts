import type { KeyboardEvent } from "react";

/**
 * True while an IME is composing (Japanese input confirms a conversion with
 * Enter). Such an Enter must not submit anything.
 */
export function isComposing(e: KeyboardEvent): boolean {
  return e.nativeEvent.isComposing || e.keyCode === 229;
}

const IS_MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** Ctrl on Windows, Command on macOS (the design's "Ctrl" shortcuts). */
export function isPrimaryModifier(e: { ctrlKey: boolean; metaKey: boolean }, mac: boolean = IS_MAC): boolean {
  return mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/**
 * A shortcut as on-screen text: 「Ctrl+E」, or 「Command+E」 on macOS (the
 * editors' Mod- keys and the app shortcuts take Command there).
 */
export function shortcutText(key: string, mac: boolean = IS_MAC): string {
  return `${mac ? "Command" : "Ctrl"}+${key}`;
}

/** A dialog or menu is open: Esc and shortcuts belong to it. */
export function overlayOpen(): boolean {
  return document.querySelector(".az-dialog__overlay, .menu__popover") !== null;
}
