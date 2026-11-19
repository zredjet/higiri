import { describe, expect, it } from "vitest";
import type { ProjectSummary } from "../lib/bindings";
import { buildNavSections } from "./nav";

function summary(id: number, name: string, attention: number): ProjectSummary {
  return {
    project: {
      id,
      name,
      color: "#3B6FD4",
      description: "",
      sort_order: id,
      archived_at: null,
      created_at: "2026-11-01T00:00:00.000Z",
      updated_at: "2026-11-01T00:00:00.000Z",
    },
    counts: { todo: 0, doing: 0, waiting: 0, hold: 0, done: 0, cancelled: 0 },
    todo_count: 0,
    progress_total: 0,
    progress_done: 0,
    attention_count: attention,
  };
}

describe("buildNavSections", () => {
  it("lists screens, projects with badges, then settings", () => {
    const sections = buildNavSections([summary(1, "基幹システム更改", 3), summary(2, "社内ツール整備", 0)]);
    expect(sections.map((s) => s.items.map((i) => i.id))).toEqual([
      ["dashboard", "todos", "gantt", "report"],
      ["project:1", "project:2", "project:new"],
      ["settings"],
    ]);
    expect(sections[1].label).toBe("プロジェクト");
    expect(sections[1].items[0]).toMatchObject({ label: "基幹システム更改", icon: "folder", badge: "3" });
    // No badge when nothing needs attention.
    expect(sections[1].items[1].badge).toBeUndefined();
  });

  it("still offers to add a project when there is none", () => {
    const sections = buildNavSections([]);
    expect(sections[1].items.map((i) => i.label)).toEqual(["プロジェクトを追加"]);
  });
});
