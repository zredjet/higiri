import type { SideNavSection } from "../components/aizome";
import type { ProjectSummary } from "../lib/bindings";
import { NEW_PROJECT_NAV_ID, projectNavId } from "./routes";

/**
 * Side-nav sections as in the mock: screens, projects (with their 日限注意
 * count as a badge), settings. The dashboard's total badge comes with
 * get_dashboard in phase 5.
 */
export function buildNavSections(projects: ProjectSummary[]): SideNavSection[] {
  return [
    {
      items: [
        { id: "dashboard", label: "ダッシュボード", icon: "home" },
        { id: "todos", label: "TODO", icon: "inbox" },
        { id: "gantt", label: "ガント", icon: "chart" },
        { id: "report", label: "日報", icon: "file" },
      ],
    },
    {
      label: "プロジェクト",
      items: [
        ...projects.map((p) => ({
          id: projectNavId(p.project.id),
          label: p.project.name,
          icon: "folder" as const,
          badge: p.attention_count > 0 ? String(p.attention_count) : undefined,
        })),
        { id: NEW_PROJECT_NAV_ID, label: "プロジェクトを追加", icon: "plus" as const },
      ],
    },
    {
      items: [{ id: "settings", label: "設定", icon: "settings" }],
    },
  ];
}
