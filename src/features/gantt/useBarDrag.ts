import { useCallback, useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { DaySpan } from "../../lib/bindings";
import { useUpdateTodo } from "../../lib/queries";
import { fieldsOf } from "../todos/detail/draft";
import { saveOpenDrafts } from "../todos/detail/useTodoDraft";
import { dayDelta, dragSpan, isDrag, type DragMode } from "./geometry";

/** The bar being dragged, past the click threshold. */
export interface DragPreview {
  todoId: number;
  span: DaySpan;
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
 */
export function useBarDrag() {
  const { mutateAsync } = useUpdateTodo();
  const [preview, setPreview] = useState<DragPreview | null>(null);
  // Dropped spans drawn until saved and fetched again (or put back on failure).
  const [saving, setSaving] = useState<ReadonlyMap<number, { span: DaySpan; seq: number }>>(new Map());
  const [error, setError] = useState<unknown>(null);
  const stop = useRef<(() => void) | null>(null);
  const seq = useRef(0);

  // Leaving the screen mid-drag drops the drag.
  useEffect(() => () => stop.current?.(), []);

  const save = useCallback(
    async (todoId: number, span: DaySpan) => {
      const mine = ++seq.current;
      setSaving((m) => new Map(m).set(todoId, { span, seq: mine }));
      try {
        // Resolves after the lists (the chart too) are fetched again.
        await mutateAsync({
          id: todoId,
          change: (current) => ({ ...fieldsOf(current), planned_start: span.start, planned_end: span.end }),
        });
      } catch (e) {
        setError(e);
      } finally {
        setSaving((m) => {
          if (m.get(todoId)?.seq !== mine) return m;
          const next = new Map(m);
          next.delete(todoId);
          return next;
        });
      }
    },
    [mutateAsync],
  );

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
          saveOpenDrafts();
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
        if (a.dragging && !a.cancelled && !sameSpan(a.span, a.origin)) void save(todoId, a.span);
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
    [save],
  );

  /** The planned span to draw: being dragged, being saved, or as fetched. */
  const plannedOf = useCallback(
    (todoId: number, planned: DaySpan | null): DaySpan | null =>
      preview?.todoId === todoId ? preview.span : (saving.get(todoId)?.span ?? planned),
    [preview, saving],
  );

  return { preview, plannedOf, begin, error, clearError: () => setError(null) };
}
