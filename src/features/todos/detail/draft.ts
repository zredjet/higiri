// The detail panel's local copy of a TODO's fields (useTodoDraft). Pure
// functions, tested in draft.test.ts.
import type { Todo, TodoFields } from "../../../lib/bindings";

export type FieldKey = keyof TodoFields;

export function fieldsOf(t: Todo): TodoFields {
  return {
    title: t.title,
    priority: t.priority,
    due_date: t.due_date,
    planned_start: t.planned_start,
    planned_end: t.planned_end,
    actual_start: t.actual_start,
    actual_end: t.actual_end,
    waiting_on: t.waiting_on,
    follow_up_date: t.follow_up_date,
    close_reason: t.close_reason,
    memo_md: t.memo_md,
  };
}

function copy<K extends FieldKey>(to: TodoFields, from: TodoFields, key: K) {
  to[key] = from[key];
}

/**
 * `server` with the fields edited by hand and not saved yet (`touched`)
 * taken from `draft`. Used both ways: when the server's copy changes (a bar
 * dragged on the Gantt chart), the panel shows the new values of the fields
 * it did not edit; when the panel saves, it sends only its edits on top of
 * the TODO as saved just then.
 */
export function mergeDraft(server: TodoFields, draft: TodoFields, touched: ReadonlySet<FieldKey>): TodoFields {
  const out = { ...server };
  for (const key of touched) copy(out, draft, key);
  return out;
}

/**
 * After the panel's edits `sent` were saved as `saved`: the fields edited
 * again while the save was on its way stay edited by hand; the others are
 * done and show the saved value (the title as trimmed by the server).
 */
export function settleSave(
  draft: TodoFields,
  touched: ReadonlySet<FieldKey>,
  sent: TodoFields,
  saved: TodoFields,
): { draft: TodoFields; touched: Set<FieldKey> } {
  const still = new Set<FieldKey>();
  const out = { ...draft };
  for (const key of touched) {
    if (draft[key] !== sent[key]) still.add(key);
    else copy(out, saved, key);
  }
  return { draft: out, touched: still };
}
