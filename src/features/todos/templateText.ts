// Text and small helpers of the TODO templates (T-10): the 「TODOを追加」 dialog,
// 「テンプレートとして保存」 in the detail panel, and the settings card.
import type { TemplateSummary } from "../../lib/bindings";

/**
 * The template with this name, or null. Spaces around the name do not count:
 * the Rust side saves names without them.
 */
export function templateNamed(templates: TemplateSummary[], name: string): TemplateSummary | null {
  const wanted = name.trim();
  return templates.find((t) => t.name === wanted) ?? null;
}

/** The 「テンプレート」 choices of the 「TODOを追加」 dialog: 「使わない」 first, then by name. */
export function templateOptions(templates: TemplateSummary[]): Array<{ value: string; label: string }> {
  return [{ value: "", label: "使わない" }, ...templates.map((t) => ({ value: String(t.id), label: t.name }))];
}

/** 「3件」: the children of a template. */
export function templateChildCount(t: TemplateSummary): string {
  return `${t.child_titles.length}件`;
}

export const TEMPLATE_DUE_HINT = "日限を入れると、子の日限も決まります。";

export const TEMPLATE_NO_CHILDREN = "子TODOはありません。";

export const NO_TEMPLATES =
  "テンプレートはありません。TODOの詳細パネルの「テンプレートとして保存」で作れます。";

export const SAVE_TEMPLATE_DESCRIPTION =
  "このTODOと子TODO(中止したものを除く)のタイトル、優先度、メモを保存します。子TODOの日限は、このTODOの日限からの日数で覚えます。「TODOを追加」でテンプレートを選ぶと、同じ一式を作れます。";

export function replaceTemplateQuestion(name: string): string {
  return `「${name.trim()}」という名前のテンプレートがあります。このTODOの内容で置き換えますか。置き換える前の内容は元に戻せません。`;
}

export function deleteTemplateText(name: string): string {
  return `「${name}」を削除します。このテンプレートから作ったTODOは、そのまま残ります。削除したテンプレートは元に戻せません。`;
}

export function templateSavedText(name: string): string {
  return `テンプレート「${name}」を保存しました。「TODOを追加」で選ぶと使えます。`;
}
