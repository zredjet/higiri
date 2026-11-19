import type { ProjectSummary } from "../../lib/bindings";

/** 「5件中1件完了 ・ 中止 1件 ・ 日限注意 3件」 under a project's title. */
export function projectStatsText(s: ProjectSummary): string {
  return `${s.progress_total}件中${s.progress_done}件完了 ・ 中止 ${s.counts.cancelled}件 ・ 日限注意 ${s.attention_count}件`;
}

/** Colours offered for a project. The first is the table default. */
export const PROJECT_COLORS: Array<{ value: string; label: string }> = [
  { value: "#3B6FD4", label: "藍" },
  { value: "#2E8B57", label: "緑" },
  { value: "#C98A1B", label: "琥珀" },
  { value: "#C0392B", label: "朱" },
  { value: "#7B5EA7", label: "紫" },
  { value: "#59616D", label: "墨" },
];

export const DEFAULT_PROJECT_COLOR = PROJECT_COLORS[0].value;

/** Options for the colour select; keeps a colour that is not in the list. */
export function colorOptions(current: string): Array<{ value: string; label: string }> {
  const known = PROJECT_COLORS.some((c) => c.value.toUpperCase() === current.toUpperCase());
  return known ? PROJECT_COLORS : [...PROJECT_COLORS, { value: current, label: `いまの色(${current})` }];
}
