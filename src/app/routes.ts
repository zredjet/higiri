// Screens and how they map to side-nav item ids. No router library: the app
// is a single window and keeps the current screen in state.

export type Route =
  | { page: "dashboard" }
  | { page: "todos"; projectId: number | null }
  | { page: "gantt" }
  | { page: "report" }
  | { page: "settings" };

/** Side-nav id of the "add a project" item (opens a dialog, not a screen). */
export const NEW_PROJECT_NAV_ID = "project:new";

/** Side-nav id of 「検索」 (opens the search dialog, not a screen). */
export const SEARCH_NAV_ID = "search";

const PROJECT_PREFIX = "project:";

export function projectNavId(projectId: number): string {
  return PROJECT_PREFIX + projectId;
}

export function routeToNavId(route: Route): string {
  if (route.page === "todos") {
    return route.projectId === null ? "todos" : projectNavId(route.projectId);
  }
  return route.page;
}

/** The screen for a side-nav id, or null for ids that are not screens. */
export function navIdToRoute(id: string): Route | null {
  switch (id) {
    case "dashboard":
    case "gantt":
    case "report":
    case "settings":
      return { page: id };
    case "todos":
      return { page: "todos", projectId: null };
  }
  if (id.startsWith(PROJECT_PREFIX)) {
    const n = Number(id.slice(PROJECT_PREFIX.length));
    if (Number.isInteger(n) && n > 0) return { page: "todos", projectId: n };
  }
  return null;
}
