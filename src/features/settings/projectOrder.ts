import type { ProjectSummary } from "../../lib/bindings";

/** プロジェクトの並び (P-4, v0.6.0): the card's words, as in the design document. */
export const PROJECT_ORDER_TEXT = {
  title: "プロジェクトの並び",
  description:
    "サイドナビ、TODO一覧、ガント、ダッシュボード、日報のプロジェクトの順です。アーカイブしたプロジェクトは、元に戻すと前の位置に戻ります。",
  column: "並べ替え",
  empty: "稼働中のプロジェクトはありません。",
} as const;

/** The buttons' names for screen readers, as for the sections of the report template. */
export function moveUpLabel(name: string): string {
  return `「${name}」を上へ移動する`;
}

export function moveDownLabel(name: string): string {
  return `「${name}」を下へ移動する`;
}

/** The active projects in the side nav's order (`list_projects`: sort order, then id). */
export function activeProjects(projects: readonly ProjectSummary[]): ProjectSummary[] {
  return projects
    .filter((p) => p.project.archived_at === null)
    .sort((a, b) => a.project.sort_order - b.project.sort_order || a.project.id - b.project.id);
}

/** `ids` with `id` moved one place up (-1) or down (1). The same array when it cannot move. */
export function moved(ids: readonly number[], id: number, delta: -1 | 1): readonly number[] {
  const from = ids.indexOf(id);
  const to = from + delta;
  if (from < 0 || to < 0 || to >= ids.length) return ids;
  const next = [...ids];
  next[from] = ids[to];
  next[to] = id;
  return next;
}

/**
 * The order to show: the one being saved while it names the same projects as the saved order (a project
 * added or archived meanwhile makes it stale), else the saved one.
 */
export function shownOrder(saved: readonly number[], pending: readonly number[] | null): readonly number[] {
  if (pending === null || pending.length !== saved.length) return saved;
  const ids = new Set(saved);
  return pending.every((id) => ids.has(id)) ? pending : saved;
}
