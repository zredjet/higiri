import { describe, expect, it } from "vitest";
import type { BlockingTodo } from "../../lib/bindings";
import {
  asksBeforeDoing,
  blockedMarkTitle,
  blockedQuestion,
  blockingLine,
  blockingTitle,
  pickBlockedReason,
  removeLabel,
} from "./dependencyText";

const first: BlockingTodo = { id: 1, title: "要件の確定", parent_title: null, project_name: "基幹システム更改" };
const child: BlockingTodo = { id: 2, title: "手順書", parent_title: "開発環境の標準化", project_name: "社内ツール整備" };

describe("dependencyText", () => {
  it("names a child with its parent", () => {
    expect(blockingTitle(first)).toBe("要件の確定");
    expect(blockingTitle(child)).toBe("開発環境の標準化 / 手順書");
    expect(blockingLine(child)).toBe("開発環境の標準化 / 手順書(社内ツール整備)");
  });

  it("lists every unfinished TODO in the mark's tooltip", () => {
    expect(blockedMarkTitle([first, child])).toBe("先のTODOが未完了:要件の確定、開発環境の標準化 / 手順書");
  });

  it("asks before 作業中 only while something is unfinished", () => {
    expect(asksBeforeDoing({ blocked_by: [first] })).toBe(true);
    expect(asksBeforeDoing({ blocked_by: [] })).toBe(false);
    expect(blockedQuestion(2)).toBe("次の2件が、まだ完了も中止もしていません。このまま作業中にしますか。");
  });

  it("does not let the TODO itself or a joined one be picked", () => {
    const joined = new Set([3]);
    expect(pickBlockedReason(5, 5, joined)).toBe("このTODO");
    expect(pickBlockedReason(3, 5, joined)).toBe("足してあります");
    expect(pickBlockedReason(4, 5, joined)).toBeNull();
    expect(removeLabel("要件の確定")).toBe("「要件の確定」との依存を外す");
  });
});
