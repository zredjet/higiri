import { describe, expect, it } from "vitest";
import type { ProjectSummary } from "../../lib/bindings";
import { cliProjectRows, cliSwitchLabel, hiddenAfter } from "./cliProjects";

function project(id: number, name: string, sortOrder: number, archived = false): ProjectSummary {
  return {
    project: {
      id,
      name,
      color: "#1f4e79",
      description: "",
      sort_order: sortOrder,
      archived_at: archived ? "2026-11-10T01:00:00.000Z" : null,
      created_at: "2026-11-01T01:00:00.000Z",
      updated_at: "2026-11-01T01:00:00.000Z",
    },
    counts: { todo: 0, doing: 0, waiting: 0, hold: 0, done: 0, cancelled: 0 },
    todo_count: 0,
    progress_total: 0,
    progress_done: 0,
    attention_count: 0,
  };
}

describe("cliProjectRows", () => {
  it("lists the active projects first and the archived ones below, each in the side nav's order", () => {
    const projects = [
      project(4, "旧システム保守", 1, true),
      project(2, "社内ツール整備", 2),
      project(1, "基幹システム刷新", 1),
      project(5, "2025年度の棚卸し", 0, true),
      project(3, "採用", 2),
    ];
    expect(cliProjectRows(projects, []).map((r) => [r.id, r.name, r.archived])).toEqual([
      [1, "基幹システム刷新", false],
      [2, "社内ツール整備", false],
      [3, "採用", false],
      [5, "2025年度の棚卸し", true],
      [4, "旧システム保守", true],
    ]);
  });

  it("turns the switch off for the projects the CLI does not show", () => {
    const projects = [project(1, "基幹システム刷新", 0), project(2, "社内ツール整備", 1), project(3, "採用", 2, true)];
    expect(cliProjectRows(projects, [2, 3]).map((r) => r.shown)).toEqual([true, false, false]);
  });

  it("ignores hidden ids of projects that are gone", () => {
    expect(cliProjectRows([project(1, "基幹システム刷新", 0)], [9]).map((r) => r.shown)).toEqual([true]);
  });

  it("has no rows without projects", () => {
    expect(cliProjectRows([], [1])).toEqual([]);
  });
});

describe("hiddenAfter", () => {
  it("adds the project turned off, in increasing order", () => {
    expect(hiddenAfter([1, 7], 3, false)).toEqual([1, 3, 7]);
  });

  it("removes the project turned on", () => {
    expect(hiddenAfter([1, 3, 7], 3, true)).toEqual([1, 7]);
  });

  it("does not add an id twice or remove what is not there", () => {
    expect(hiddenAfter([3], 3, false)).toEqual([3]);
    expect(hiddenAfter([1], 3, true)).toEqual([1]);
    expect(hiddenAfter([], 2, true)).toEqual([]);
  });
});

describe("cliSwitchLabel", () => {
  it("names the project for screen readers", () => {
    expect(cliSwitchLabel("社内ツール整備")).toBe("「社内ツール整備」をCLIに出す");
  });
});
