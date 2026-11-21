import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { DaySpan, DependentShift } from "../../lib/bindings";
import { usePreviewDependentShift, useShiftWithDependents, useUpdateTodo } from "../../lib/queries";
import { fieldsOf } from "../todos/detail/draft";
import { saveOpenDrafts } from "../todos/detail/useTodoDraft";
import { dayDelta, dragSpan, endsLater, isDrag, type DragMode } from "./geometry";

/** The bar being dragged, past the click threshold. */
export interface DragPreview {
  todoId: number;
  span: DaySpan;
}

/**
 * A drop that would make 後続TODO start before the dragged TODO ends (G-8):
 * the question asked before saving. The bar stays where it was dropped.
 */
export interface ShiftQuestion {
  todoId: number;
  span: DaySpan;
  /** From Rust, in the dependencies' order (never empty). */
  shifts: DependentShift[];
}

interface Active {
  todoId: number;
  mode: DragMode;
  origin: DaySpan;
  dayWidth: number;
  pointerId: number;
  x: number;
  y: number;
  /** Moved past DRAG_THRESHOLD: no longer a click. */
  dragging: boolean;
  /** Esc was pressed: nothing is saved, the release does not click. */
  cancelled: boolean;
  span: DaySpan;
}

const ROOT_CLASS = { move: "gantt-dragging--move", resize: "gantt-dragging--resize" } as const;

function sameSpan(a: DaySpan, b: DaySpan): boolean {
  return a.start === b.start && a.end === b.end;
}

/** Swallows the click that ends a drag, wherever it lands (it must not open a detail). */
function ignoreNextClick() {
  const swallow = (e: MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
  };
  window.addEventListener("click", swallow, { capture: true, once: true });
  // The click, if any, comes in the same task as the pointerup.
  window.setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
}

/**
 * Dragging planned bars (G-6). The state lives here, above the rows: the
 * chart draws only the rows in view, and the dragged row may scroll out of
 * view and back while the button is held. For the same reason the pointer is
 * followed on the window; the grabbed grip also takes pointer capture.
 *
 * A drop that ends later asks first when 後続TODO would start before it ends
 * (G-8, `question`): the screen shows the question and answers it with
 * `cancelShift`, `shiftOnlyThis` or `shiftWithDependents`.
 */
export function useBarDrag() {
  const { mutateAsync } = useUpdateTodo();
  const { mutateAsync: previewShift } = usePreviewDependentShift();
  const { mutateAsync: shiftAll } = useShiftWithDependents();
  const [preview, setPreview] = useState<DragPreview | null>(null);
  // Spans drawn instead of the fetched ones: dropped and asked about, or
  // being saved, until fetched again (or put back on failure).
  const [saving, setSaving] = useState<ReadonlyMap<number, { span: DaySpan; seq: number }>>(new Map());
  // With the mark of the hold that draws the dropped bar.
  const [question, setQuestion] = useState<(ShiftQuestion & { mark: number }) | null>(null);
  const [error, setError] = useState<unknown>(null);
  const stop = useRef<(() => void) | null>(null);
  const seq = useRef(0);

  // Leaving the screen mid-drag drops the drag.
  useEffect(() => () => stop.current?.(), []);

  /** Draws these bars at these spans until `release` with the returned mark. */
  const hold = useCallback((spans: ReadonlyArray<readonly [number, DaySpan]>) => {
    const mine = ++seq.current;
    setSaving((m) => {
      const next = new Map(m);
      for (const [id, span] of spans) next.set(id, { span, seq: mine });
      return next;
    });
    return mine;
  }, []);

  /** Draws the bars as fetched again, unless a later hold took them. */
  const release = useCallback((ids: readonly number[], mine: number) => {
    setSaving((m) => {
      if (!ids.some((id) => m.get(id)?.seq === mine)) return m;
      const next = new Map(m);
      for (const id of ids) if (next.get(id)?.seq === mine) next.delete(id);
      return next;
    });
  }, []);

  /** Saves the dragged TODO only (update_todo). */
  const save = useCallback(
    async (todoId: number, span: DaySpan) => {
      const mine = hold([[todoId, span]]);
      try {
        // Resolves after the lists (the chart too) are fetched again.
        await mutateAsync({
          id: todoId,
          change: (current) => ({ ...fieldsOf(current), planned_start: span.start, planned_end: span.end }),
        });
      } catch (e) {
        setError(e);
      } finally {
        release([todoId], mine);
      }
    },
    [mutateAsync, hold, release],
  );

  /**
   * A drop that ends later may push 後続TODO: Rust says which (G-8). None:
   * saved at once. Some: asked first, the bar staying where it was dropped.
   */
  const drop = useCallback(
    async (todoId: number, origin: DaySpan, span: DaySpan) => {
      if (!endsLater(origin, span)) return save(todoId, span);
      const mine = hold([[todoId, span]]);
      let shifts: DependentShift[];
      try {
        shifts = await previewShift({ todoId, start: span.start, end: span.end });
      } catch (e) {
        setError(e);
        release([todoId], mine);
        return;
      }
      if (shifts.length === 0) return save(todoId, span);
      setQuestion({ todoId, span, shifts, mark: mine });
    },
    [save, hold, release, previewShift],
  );

  /** 「キャンセル」, Esc, ×: the drag is undone, nothing is saved. */
  const cancelShift = useCallback(() => {
    if (!question) return;
    setQuestion(null);
    release([question.todoId], question.mark);
  }, [question, release]);

  /** 「このTODOだけ動かす」: saved as any drag. */
  const shiftOnlyThis = useCallback(() => {
    if (!question) return;
    setQuestion(null);
    void save(question.todoId, question.span);
  }, [question, save]);

  /**
   * 「後続もずらす」: the 後続TODO are drawn at their new spans while saving
   * (Rust works them out again: what it saves is what is fetched next).
   */
  const shiftWithDependents = useCallback(async () => {
    if (!question) return;
    const { todoId, span, shifts } = question;
    setQuestion(null);
    const spans: Array<readonly [number, DaySpan]> = [[todoId, span]];
    for (const s of shifts) {
      if (s.shifted_end) spans.push([s.id, { start: s.shifted_start, end: s.shifted_end }]);
    }
    const mine = hold(spans);
    try {
      await shiftAll({ todoId, start: span.start, end: span.end });
    } catch (e) {
      setError(e);
    } finally {
      release(
        spans.map(([id]) => id),
        mine,
      );
    }
  }, [question, hold, release, shiftAll]);

  /** pointerdown on a grip of the bar showing `span`. */
  const begin = useCallback(
    (e: ReactPointerEvent<Element>, todoId: number, span: DaySpan, mode: DragMode, dayWidth: number) => {
      if (e.button !== 0 || stop.current) return;
      // No text selection; the focus stays where it is (the detail panel's editor).
      e.preventDefault();
      const grip = e.currentTarget;
      const pointerId = e.pointerId;
      try {
        grip.setPointerCapture(pointerId);
      } catch {
        // The window listeners follow the pointer anyway.
      }
      const a: Active = {
        todoId,
        mode,
        origin: span,
        dayWidth,
        pointerId,
        x: e.clientX,
        y: e.clientY,
        dragging: false,
        cancelled: false,
        span,
      };
      const rootClass = mode === "move" ? ROOT_CLASS.move : ROOT_CLASS.resize;
      const root = document.documentElement;

      const end = () => {
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
        window.removeEventListener("pointercancel", onCancel);
        window.removeEventListener("keydown", onKey, true);
        window.removeEventListener("blur", end);
        root.classList.remove("gantt-dragging", rootClass);
        try {
          if (grip.hasPointerCapture(pointerId)) grip.releasePointerCapture(pointerId);
        } catch {
          // Already released (the grip may be gone).
        }
        stop.current = null;
        setPreview(null);
      };
      const onMove = (ev: PointerEvent) => {
        if (ev.pointerId !== pointerId || a.cancelled) return;
        const dx = ev.clientX - a.x;
        if (!a.dragging) {
          if (!isDrag(dx, ev.clientY - a.y)) return;
          a.dragging = true;
          setError(null);
          // A planned date typed in the panel just before is saved first.
          void saveOpenDrafts();
          root.classList.add("gantt-dragging", rootClass);
          setPreview({ todoId, span: a.span });
        }
        const next = dragSpan(a.origin, a.mode, dayDelta(dx, a.dayWidth));
        if (sameSpan(next, a.span)) return;
        a.span = next;
        setPreview({ todoId, span: next });
      };
      const onUp = (ev: PointerEvent) => {
        if (ev.pointerId !== pointerId) return;
        end();
        if (!a.dragging && !a.cancelled) return; // a click: opens the detail
        ignoreNextClick();
        if (a.dragging && !a.cancelled && !sameSpan(a.span, a.origin)) void drop(todoId, a.origin, a.span);
      };
      const onCancel = (ev: PointerEvent) => {
        if (ev.pointerId === pointerId) end();
      };
      const onKey = (ev: KeyboardEvent) => {
        if (ev.key !== "Escape") return;
        // Esc puts the bar back; it does not also close the detail panel.
        ev.preventDefault();
        ev.stopPropagation();
        a.cancelled = true;
        root.classList.remove("gantt-dragging", rootClass);
        setPreview(null);
      };

      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
      window.addEventListener("pointercancel", onCancel);
      window.addEventListener("keydown", onKey, true);
      window.addEventListener("blur", end);
      stop.current = end;
    },
    [drop],
  );

  /** The planned span to draw: being dragged, asked about or saved, or as fetched. */
  const plannedOf = useCallback(
    (todoId: number, planned: DaySpan | null): DaySpan | null =>
      preview?.todoId === todoId ? preview.span : (saving.get(todoId)?.span ?? planned),
    [preview, saving],
  );

  return {
    preview,
    plannedOf,
    begin,
    error,
    clearError: () => setError(null),
    /** The question before moving 後続TODO too, while it is asked. */
    question,
    cancelShift,
    shiftOnlyThis,
    shiftWithDependents,
  };
}
