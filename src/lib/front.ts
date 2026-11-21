import { focusManager } from "@tanstack/react-query";

/**
 * The events after which the app may be back in front of the user. TanStack
 * Query listens to `visibilitychange` only, which fires when the page is
 * hidden and shown again (minimised, hidden to the tray). Clicking the window
 * after working in a terminal beside it does not hide the page, so `focus` is
 * added: the lists then show what higiri-cli changed (v0.5.0).
 */
export const FRONT_EVENTS = ["visibilitychange", "focus"] as const;

/**
 * Calls `notify` on each of FRONT_EVENTS reaching `target` itself. A focus
 * moving between elements inside the page does not reach the window (`focus`
 * does not bubble, and the listener is not a capturing one). Returns the
 * function that stops listening.
 */
export function listenForFront(target: EventTarget, notify: () => void): () => void {
  const listener = () => notify();
  for (const type of FRONT_EVENTS) target.addEventListener(type, listener, false);
  return () => {
    for (const type of FRONT_EVENTS) target.removeEventListener(type, listener, false);
  };
}

/**
 * Refreshes the open screen's lists whenever the app comes to the front
 * (queries with `refetchOnWindowFocus`, the default). Called once in main.tsx.
 */
export function refetchOnFront(target: EventTarget = window): void {
  // With no argument, TanStack Query reads the page's visibility again and tells its queries.
  focusManager.setEventListener((setFocused) => listenForFront(target, () => setFocused()));
}
