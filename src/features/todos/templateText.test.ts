import { describe, expect, it } from "vitest";
import type { TemplateSummary } from "../../lib/bindings";
import {
  deleteTemplateText,
  replaceTemplateQuestion,
  templateChildCount,
  templateNamed,
  templateOptions,
  templateSavedText,
} from "./templateText";

function template(id: number, name: string, children: string[] = []): TemplateSummary {
  return {
    id,
    name,
    title: name,
    priority: "medium",
    child_titles: children,
    updated_at: "2026-11-19T01:00:00.000Z",
  };
}

describe("templateNamed", () => {
  const list = [template(1, "月次締め"), template(2, "週次")];

  it("finds the same name, spaces around it aside", () => {
    expect(templateNamed(list, "月次締め")?.id).toBe(1);
    expect(templateNamed(list, " 週次　")?.id).toBe(2);
  });

  it("does not match a part of a name", () => {
    expect(templateNamed(list, "月次")).toBeNull();
    expect(templateNamed(list, "")).toBeNull();
    expect(templateNamed([], "月次締め")).toBeNull();
  });
});

describe("templateOptions", () => {
  it("puts 「使わない」 first and keeps the order", () => {
    expect(templateOptions([template(3, "月次締め"), template(1, "週次")])).toEqual([
      { value: "", label: "使わない" },
      { value: "3", label: "月次締め" },
      { value: "1", label: "週次" },
    ]);
    expect(templateOptions([])).toEqual([{ value: "", label: "使わない" }]);
  });
});

describe("text", () => {
  it("counts the children", () => {
    expect(templateChildCount(template(1, "a", ["x", "y", "z"]))).toBe("3件");
    expect(templateChildCount(template(1, "a"))).toBe("0件");
  });

  it("names the template", () => {
    expect(replaceTemplateQuestion(" 月次締め ")).toBe(
      "「月次締め」という名前のテンプレートがあります。このTODOの内容で置き換えますか。置き換える前の内容は元に戻せません。",
    );
    expect(deleteTemplateText("月次締め")).toBe(
      "「月次締め」を削除します。このテンプレートから作ったTODOは、そのまま残ります。削除したテンプレートは元に戻せません。",
    );
    expect(templateSavedText("月次締め")).toBe("テンプレート「月次締め」を保存しました。「TODOを追加」で選ぶと使えます。");
  });
});
