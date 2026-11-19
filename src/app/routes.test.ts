import { describe, expect, it } from "vitest";
import { NEW_PROJECT_NAV_ID, navIdToRoute, projectNavId, routeToNavId, SEARCH_NAV_ID, type Route } from "./routes";

describe("routes", () => {
  it("round-trips every screen through its nav id", () => {
    const routes: Route[] = [
      { page: "dashboard" },
      { page: "todos", projectId: null },
      { page: "todos", projectId: 12 },
      { page: "gantt" },
      { page: "report" },
      { page: "settings" },
    ];
    for (const r of routes) {
      expect(navIdToRoute(routeToNavId(r))).toEqual(r);
    }
  });

  it("maps a project to the TODO screen of that project", () => {
    expect(projectNavId(3)).toBe("project:3");
    expect(navIdToRoute("project:3")).toEqual({ page: "todos", projectId: 3 });
  });

  it("does not treat the add-project item, 検索 or junk as a screen", () => {
    expect(navIdToRoute(NEW_PROJECT_NAV_ID)).toBeNull();
    expect(navIdToRoute(SEARCH_NAV_ID)).toBeNull();
    expect(navIdToRoute("project:0")).toBeNull();
    expect(navIdToRoute("project:1.5")).toBeNull();
    expect(navIdToRoute("unknown")).toBeNull();
  });
});
