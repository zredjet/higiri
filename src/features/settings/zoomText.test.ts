import { describe, expect, it } from "vitest";
import { zoomOptions } from "./zoomText";

describe("zoom options", () => {
  it("offers 70% to 150% with 100% as the standard", () => {
    expect(zoomOptions()).toEqual([
      { value: "70", label: "70%" },
      { value: "80", label: "80%" },
      { value: "90", label: "90%" },
      { value: "100", label: "100%(標準)" },
      { value: "110", label: "110%" },
      { value: "125", label: "125%" },
      { value: "150", label: "150%" },
    ]);
  });
});
