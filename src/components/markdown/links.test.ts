import { describe, expect, it } from "vitest";
import { isExternalHttpUrl } from "./links";

describe("isExternalHttpUrl", () => {
  it("accepts http and https", () => {
    expect(isExternalHttpUrl("https://example.com/a?b=c")).toBe(true);
    expect(isExternalHttpUrl("http://localhost:8080")).toBe(true);
  });

  it("rejects everything else", () => {
    for (const href of ["javascript:alert(1)", "file:///etc/hosts", "mailto:a@example.com", "/relative", "#top", ""]) {
      expect(isExternalHttpUrl(href)).toBe(false);
    }
  });
});
