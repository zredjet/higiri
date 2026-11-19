import { useCallback, useEffect, useRef, useState } from "react";
import type { TodoFields, TodoItem } from "../../../lib/bindings";
import { useUpdateTodo } from "../../../lib/queries";
import { fieldsOf, mergeDraft, settleSave, type FieldKey } from "./draft";

/** Autosave delay after the last keystroke (M-4). */
export const AUTOSAVE_MS = 1000;

/** Save functions of the open detail panels (one at most). */
const openDrafts = new Set<() => Promise<void>>();

/**
 * Saves the detail panel's waiting edits now. A Gantt drag calls this when
 * it starts, so a planned date typed in the panel just before is saved first
 * and the drag wins.
 */
export function saveOpenDrafts(): void {
  for (const save of openDrafts) void save();
}

/**
 * Local copy of a TODO's editable fields, saved with update_todo 1 second
 * after the last change and on unmount (closing the panel, switching TODOs).
 * Each field remembers whether it was edited by hand and not saved yet: those
 * keep their local value when the TODO changes elsewhere (a Gantt drag), the
 * others take the new value. A save sends only those fields, on top of the
 * TODO as saved just then (useUpdateTodo).
 */
export function useTodoDraft(item: TodoItem) {
  const id = item.todo.id;
  const { mutateAsync, error, isPending, reset } = useUpdateTodo();
  const [draft, setDraft] = useState<TodoFields>(() => fieldsOf(item.todo));
  // The current values for saves and merges between renders.
  const latest = useRef(draft);
  const touched = useRef<Set<FieldKey>>(new Set());
  const timer = useRef<number | null>(null);

  const show = useCallback((fields: TodoFields) => {
    latest.current = fields;
    setDraft(fields);
  }, []);

  useEffect(() => {
    show(mergeDraft(fieldsOf(item.todo), latest.current, touched.current));
  }, [item.todo, show]);

  const save = useCallback(async () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    if (touched.current.size === 0) return;
    // Built when the save runs (after any write before it), not now.
    const request: { sent: TodoFields | null } = { sent: null };
    let saved: TodoItem;
    try {
      saved = await mutateAsync({
        id,
        change: (current) => (request.sent = mergeDraft(fieldsOf(current), latest.current, touched.current)),
      });
    } catch {
      // Keep the edits; the error is shown and the next change retries.
      return;
    }
    if (request.sent === null) return;
    const next = settleSave(latest.current, touched.current, request.sent, fieldsOf(saved.todo));
    touched.current = next.touched;
    show(next.draft);
  }, [id, mutateAsync, show]);

  const set = useCallback(
    <K extends keyof TodoFields>(key: K, value: TodoFields[K]) => {
      show({ ...latest.current, [key]: value });
      touched.current.add(key);
      reset();
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => void save(), AUTOSAVE_MS);
    },
    [save, reset, show],
  );

  useEffect(() => {
    openDrafts.add(save);
    return () => {
      openDrafts.delete(save);
      void save();
    };
  }, [save]);

  return { draft, set, save, error, saving: isPending };
}
