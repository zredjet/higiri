import { describe, expect, it } from "vitest";
import type { ProjectSummary } from "../../lib/bindings";
import { activeProjects, moved, moveDownLabel, moveUpLabel, shownOrder } from "./projectOrder";

function summary(id: number, sort_order: number, archived = false): ProjectSummary {
  return {
    project: {
      id,
      name: `P${id}`,
      color: "#3B6FD4",
      description: "",
      sort_order,
      archived_at: archived ? "2026-11-20T00:00:00.000Z" : null,
      created_at: "2026-11-19T00:00:00.000Z",
      updated_at: "2026-11-19T00:00:00.000Z",
    },
    counts: { todo: 0, doing: 0, waiting: 0, hold: 0, done: 0, cancelled: 0 },
    todo_count: 0,
    progress_total: 0,
    progress_done: 0,
    attention_count: 0,
  };
}

describe("activeProjects", () => {
  it("leaves out archived projects and keeps the side nav's order", () => {
    const list = [summary(1, 3), summary(2, 1), summary(3, 2, true), summary(4, 1)];
    expect(activeProjects(list).map((p) => p.project.id)).toEqual([2, 4, 1]);
  });
});

describe("moved", () => {
  it("swaps with the neighbour", () => {
    expect(moved([1, 2, 3], 2, -1)).toEqual([2, 1, 3]);
    expect(moved([1, 2, 3], 2, 1)).toEqual([1, 3, 2]);
    expect(moved([1, 2, 3], 3, -1)).toEqual([1, 3, 2]);
  });

  it("returns the same array at the ends and for an unknown id", () => {
    const ids = [1, 2, 3];
    expect(moved(ids, 1, -1)).toBe(ids);
    expect(moved(ids, 3, 1)).toBe(ids);
    expect(moved(ids, 9, 1)).toBe(ids);
    expect(moved([], 1, 1)).toEqual([]);
  });
});

describe("shownOrder", () => {
  it("shows the order being saved", () => {
    expect(shownOrder([1, 2, 3], [3, 1, 2])).toEqual([3, 1, 2]);
  });

  it("shows the saved order when nothing is being saved or the projects changed", () => {
    const saved = [1, 2, 3];
    expect(shownOrder(saved, null)).toBe(saved);
    expect(shownOrder(saved, [2, 1])).toBe(saved);
    expect(shownOrder(saved, [3, 2, 1, 4])).toBe(saved);
    expect(shownOrder(saved, [4, 2, 1])).toBe(saved);
  });
});

describe("labels", () => {
  it("names the project", () => {
    expect(moveUpLabel("社内ツール整備")).toBe("「社内ツール整備」を上へ移動する");
    expect(moveDownLabel("社内ツール整備")).toBe("「社内ツール整備」を下へ移動する");
  });
});
