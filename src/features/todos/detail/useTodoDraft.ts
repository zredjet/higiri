import { useCallback, useEffect, useRef, useState } from "react";
import type { Todo, TodoFields, TodoItem } from "../../../lib/bindings";
import { useUpdateTodo } from "../../../lib/queries";

/** Autosave delay after the last keystroke (M-4). */
export const AUTOSAVE_MS = 1000;

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

/**
 * Local copy of a TODO's editable fields, saved with update_todo 1 second
 * after the last change and on unmount (closing the panel, switching TODOs).
 * While there are unsaved edits, updates from the server do not overwrite them.
 */
export function useTodoDraft(item: TodoItem) {
  const id = item.todo.id;
  const { mutateAsync, error, isPending, reset } = useUpdateTodo();
  const [draft, setDraft] = useState<TodoFields>(() => fieldsOf(item.todo));
  const latest = useRef(draft);
  latest.current = draft;
  const dirty = useRef(false);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    if (!dirty.current) setDraft(fieldsOf(item.todo));
  }, [item.todo]);

  const save = useCallback(async () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    if (!dirty.current) return;
    dirty.current = false;
    try {
      await mutateAsync({ id, fields: latest.current });
    } catch {
      // Keep the edits; the error is shown and the next change retries.
      dirty.current = true;
    }
  }, [id, mutateAsync]);

  const set = useCallback(
    <K extends keyof TodoFields>(key: K, value: TodoFields[K]) => {
      setDraft((d) => ({ ...d, [key]: value }));
      dirty.current = true;
      reset();
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => void save(), AUTOSAVE_MS);
    },
    [save, reset],
  );

  useEffect(() => () => void save(), [save]);

  return { draft, set, save, error, saving: isPending };
}
