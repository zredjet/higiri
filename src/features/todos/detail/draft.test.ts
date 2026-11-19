import { describe, expect, it } from "vitest";
import type { Todo, TodoFields } from "../../../lib/bindings";
import { fieldsOf, mergeDraft, settleSave, type FieldKey } from "./draft";

const SAVED: TodoFields = {
  title: "移行手順書のドラフト",
  priority: "medium",
  due_date: "2026-11-27",
  planned_start: "2026-11-11",
  planned_end: "2026-11-18",
  actual_start: "2026-11-12",
  actual_end: null,
  waiting_on: null,
  follow_up_date: null,
  close_reason: null,
  memo_md: "## 手順",
};

const touched = (...keys: FieldKey[]) => new Set<FieldKey>(keys);

describe("fieldsOf", () => {
  it("takes the editable fields of a TODO", () => {
    const todo: Todo = {
      ...SAVED,
      id: 1,
      project_id: 1,
      parent_id: null,
      status: "doing",
      waiting_since: null,
      closed_at: null,
      sort_order: 0,
      created_at: "2026-11-10T00:00:00.000Z",
      updated_at: "2026-11-19T01:30:00.000Z",
    };
    expect(fieldsOf(todo)).toEqual(SAVED);
  });
});

describe("mergeDraft", () => {
  // A bar dragged on the Gantt chart while the panel is open.
  const dragged = { ...SAVED, planned_start: "2026-11-13", planned_end: "2026-11-20" };

  it("takes the server's values when nothing was edited by hand", () => {
    expect(mergeDraft(dragged, SAVED, touched())).toEqual(dragged);
  });

  it("keeps the fields edited by hand and takes the others", () => {
    const draft = { ...SAVED, memo_md: "## 手順\n- 確認する", title: "移行手順書" };
    expect(mergeDraft(dragged, draft, touched("memo_md", "title"))).toEqual({
      ...dragged,
      memo_md: "## 手順\n- 確認する",
      title: "移行手順書",
    });
  });

  it("keeps a field edited by hand even when the server changed it too", () => {
    const draft = { ...SAVED, planned_end: "2026-11-25" };
    expect(mergeDraft(dragged, draft, touched("planned_end"))).toEqual({ ...dragged, planned_end: "2026-11-25" });
  });

  it("keeps a field cleared by hand (null)", () => {
    const draft = { ...SAVED, due_date: null };
    expect(mergeDraft(dragged, draft, touched("due_date")).due_date).toBeNull();
  });

  it("sends only the panel's edits on top of the TODO as saved now", () => {
    // The panel opened before the drag and saves its memo after it: the
    // dragged span stays.
    const draft = { ...SAVED, memo_md: "追記" };
    const sent = mergeDraft(dragged, draft, touched("memo_md"));
    expect(sent.planned_start).toBe("2026-11-13");
    expect(sent.memo_md).toBe("追記");
  });
});

describe("settleSave", () => {
  it("ends the edits that were saved, with the saved value", () => {
    const draft = { ...SAVED, title: "  移行手順書  " };
    const saved = { ...SAVED, title: "移行手順書" };
    const next = settleSave(draft, touched("title"), draft, saved);
    expect(next.touched.size).toBe(0);
    expect(next.draft.title).toBe("移行手順書");
  });

  it("keeps the edits made while the save was on its way", () => {
    const sent = { ...SAVED, memo_md: "a", title: "b" };
    const draft = { ...sent, memo_md: "ab" };
    const next = settleSave(draft, touched("memo_md", "title"), sent, sent);
    expect([...next.touched]).toEqual(["memo_md"]);
    expect(next.draft.memo_md).toBe("ab");
  });

  it("leaves the fields not edited by hand as they are", () => {
    // The draft already shows a newer planned span than the saved one.
    const draft = { ...SAVED, planned_start: "2026-11-13", memo_md: "a" };
    const sent = { ...SAVED, memo_md: "a" };
    const next = settleSave(draft, touched("memo_md"), sent, sent);
    expect(next.draft.planned_start).toBe("2026-11-13");
  });
});
