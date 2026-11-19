import { describe, expect, it } from "vitest";
import { shortcutFor } from "./shortcuts";

const press = (key: string, mods: Partial<{ ctrl: boolean; meta: boolean; alt: boolean; shift: boolean }>) => ({
  key,
  ctrlKey: !!mods.ctrl,
  metaKey: !!mods.meta,
  altKey: !!mods.alt,
  shiftKey: !!mods.shift,
});

describe("shortcutFor", () => {
  it("uses Ctrl on Windows", () => {
    expect(shortcutFor(press("n", { ctrl: true }), false)).toEqual({ kind: "new_todo" });
    expect(shortcutFor(press("1", { ctrl: true }), false)).toEqual({ kind: "go", route: { page: "dashboard" } });
    expect(shortcutFor(press("2", { ctrl: true }), false)).toEqual({
      kind: "go",
      route: { page: "todos", projectId: null },
    });
    expect(shortcutFor(press("3", { ctrl: true }), false)).toEqual({ kind: "go", route: { page: "gantt" } });
    expect(shortcutFor(press("4", { ctrl: true }), false)).toEqual({ kind: "go", route: { page: "report" } });
    expect(shortcutFor(press("f", { ctrl: true }), false)).toEqual({ kind: "search" });
    expect(shortcutFor(press("n", { meta: true }), false)).toBeNull();
    expect(shortcutFor(press("f", { meta: true }), false)).toBeNull();
  });

  it("uses Command on macOS", () => {
    expect(shortcutFor(press("n", { meta: true }), true)).toEqual({ kind: "new_todo" });
    expect(shortcutFor(press("N", { meta: true }), true)).toEqual({ kind: "new_todo" });
    expect(shortcutFor(press("f", { meta: true }), true)).toEqual({ kind: "search" });
    expect(shortcutFor(press("F", { meta: true }), true)).toEqual({ kind: "search" });
    // Ctrl+N moves the cursor in macOS text fields; leave it alone.
    expect(shortcutFor(press("n", { ctrl: true }), true)).toBeNull();
  });

  it("ignores other keys and extra modifiers", () => {
    expect(shortcutFor(press("5", { ctrl: true }), false)).toBeNull();
    expect(shortcutFor(press("n", {}), false)).toBeNull();
    expect(shortcutFor(press("n", { ctrl: true, shift: true }), false)).toBeNull();
    expect(shortcutFor(press("f", {}), false)).toBeNull();
    expect(shortcutFor(press("f", { ctrl: true, shift: true }), false)).toBeNull();
    expect(shortcutFor(press("1", { ctrl: true, alt: true }), false)).toBeNull();
  });
});
