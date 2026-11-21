import type { ProjectSummary } from "../../lib/bindings";

/** CLIに出すプロジェクト (v0.5.0, A-1): the card's words, as in the design document. */
export const CLI_PROJECTS_TEXT = {
  title: "CLIに出すプロジェクト",
  description: "higiri-cli(Claude Code などの AI から使うコマンド)に出すプロジェクトを選びます。",
  column: "CLIに出す",
  archived: "(アーカイブ済み)",
  note: "オフにしたプロジェクトのTODOは、CLIの読み取りにも書き込みにも出ません。新しく作ったプロジェクトは、初めはCLIに出します。",
  empty: "プロジェクトはありません。",
} as const;

/** The switch's name for screen readers: 「「社内ツール整備」をCLIに出す」. */
export function cliSwitchLabel(name: string): string {
  return `「${name}」をCLIに出す`;
}

/** One project of the card. */
export type CliProjectRow = {
  id: number;
  name: string;
  archived: boolean;
  /** The switch is on: higiri-cli shows the project's TODOs. */
  shown: boolean;
};

/**
 * The card's rows: the active projects first, then the archived ones (their TODOs are searched by the CLI
 * too), each in the side nav's order (`list_projects`: sort order, then id).
 */
export function cliProjectRows(projects: ProjectSummary[], hidden: readonly number[]): CliProjectRow[] {
  const hiddenIds = new Set(hidden);
  const byNavOrder = [...projects].sort(
    (a, b) => a.project.sort_order - b.project.sort_order || a.project.id - b.project.id,
  );
  const row = (p: ProjectSummary): CliProjectRow => ({
    id: p.project.id,
    name: p.project.name,
    archived: p.project.archived_at !== null,
    shown: !hiddenIds.has(p.project.id),
  });
  return [
    ...byNavOrder.filter((p) => p.project.archived_at === null).map(row),
    ...byNavOrder.filter((p) => p.project.archived_at !== null).map(row),
  ];
}

/**
 * `cli_hidden_projects` after turning a project's switch on (`shown`) or off: in increasing order, each id once
 * (as Rust saves it). The other ids stay as they are, so it is applied to the settings as saved now.
 */
export function hiddenAfter(hidden: readonly number[], id: number, shown: boolean): number[] {
  const ids = new Set(hidden);
  if (shown) ids.delete(id);
  else ids.add(id);
  return [...ids].sort((a, b) => a - b);
}
