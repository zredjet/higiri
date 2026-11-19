import type { SideNavSection } from "../components/aizome";
import type { ProjectSummary } from "../lib/bindings";
import { NEW_PROJECT_NAV_ID, projectNavId, SEARCH_NAV_ID } from "./routes";

/**
 * Side-nav sections as in the mock: screens (the dashboard with the total
 * 日限注意 from get_dashboard) and 「検索」 (a dialog, v1.1), projects (with
 * their own count), settings. A zero count shows no badge.
 */
export function buildNavSections(projects: ProjectSummary[], attentionTotal?: number): SideNavSection[] {
  return [
    {
      items: [
        {
          id: "dashboard",
          label: "ダッシュボード",
          icon: "home",
          badge: attentionTotal ? String(attentionTotal) : undefined,
        },
        { id: "todos", label: "TODO", icon: "inbox" },
        { id: "gantt", label: "ガント", icon: "chart" },
        { id: "report", label: "日報", icon: "file" },
        { id: SEARCH_NAV_ID, label: "検索", icon: "search" },
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
