import { describe, expect, it } from "vitest";
import { isPrimaryModifier, shortcutText } from "./keys";

describe("shortcutText", () => {
  it("writes Ctrl on Windows", () => {
    expect(shortcutText("E", false)).toBe("Ctrl+E");
    expect(shortcutText("Enter", false)).toBe("Ctrl+Enter");
  });

  it("writes Command on macOS", () => {
    expect(shortcutText("E", true)).toBe("Command+E");
    expect(shortcutText("Enter", true)).toBe("Command+Enter");
  });

  it("names the key the shortcuts take", () => {
    // What the text says is what isPrimaryModifier accepts.
    expect(isPrimaryModifier({ ctrlKey: true, metaKey: false }, false)).toBe(true);
    expect(isPrimaryModifier({ ctrlKey: false, metaKey: true }, true)).toBe(true);
    expect(isPrimaryModifier({ ctrlKey: true, metaKey: false }, true)).toBe(false);
  });
});
