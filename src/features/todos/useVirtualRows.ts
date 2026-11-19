import { useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { ESTIMATED_ROW, rowWindow, VIRTUAL_FROM, type RowWindow } from "./virtualRows";

/** The element that scrolls the page (the main column). */
function scrollParent(el: HTMLElement): HTMLElement {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const { overflowY } = getComputedStyle(p);
    if (overflowY === "auto" || overflowY === "scroll") return p;
  }
  return document.documentElement;
}

/**
 * Draws only the rows near the screen when a list is longer than
 * VIRTUAL_FROM. Returns the window of rows to pass to the table, the props
 * for a wrapper element (its CSS reserves the space of the rows not drawn),
 * and the ref that wrapper needs.
 */
export function useVirtualRows(keys: number[]) {
  const enabled = keys.length > VIRTUAL_FROM;
  const wrapRef = useRef<HTMLDivElement>(null);
  /** Measured heights by TODO id (kept while the list is filtered or sorted). */
  const heights = useRef(new Map<number, number>());
  const [band, setBand] = useState({ top: 0, height: 1200 });
  const [, setMeasured] = useState(0);

  // Follow the page scroll and the window size.
  useLayoutEffect(() => {
    const wrap = wrapRef.current;
    if (!enabled || !wrap) return undefined;
    const scroller = scrollParent(wrap);
    let frame = 0;
    const update = () => {
      frame = 0;
      const body = wrap.querySelector("tbody");
      if (!body) return;
      const top = scroller.getBoundingClientRect().top - body.getBoundingClientRect().top;
      const height = scroller.clientHeight;
      setBand((b) => (Math.abs(b.top - top) < 1 && b.height === height ? b : { top, height }));
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    update();
    scroller.addEventListener("scroll", schedule, { passive: true });
    const resize = new ResizeObserver(schedule);
    resize.observe(scroller);
    return () => {
      scroller.removeEventListener("scroll", schedule);
      resize.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, [enabled]);

  const shown: RowWindow = enabled
    ? rowWindow(
        keys.map((k) => heights.current.get(k) ?? ESTIMATED_ROW),
        band.top,
        band.top + band.height,
      )
    : { first: 0, end: keys.length, before: 0, after: 0 };

  // Measure the drawn rows; draw again when an estimate was off.
  useLayoutEffect(() => {
    if (!enabled) return;
    const rows = wrapRef.current?.querySelector("tbody")?.rows;
    if (!rows) return;
    let changed = false;
    for (let i = 0; i < rows.length; i++) {
      const key = keys[shown.first + i];
      if (key === undefined) break;
      const h = rows[i].getBoundingClientRect().height;
      if (Math.abs((heights.current.get(key) ?? ESTIMATED_ROW) - h) > 0.5) {
        heights.current.set(key, h);
        changed = true;
      }
    }
    if (changed) setMeasured((n) => n + 1);
  });

  const style = enabled
    ? ({ "--vt-before": `${shown.before}px`, "--vt-after": `${shown.after}px` } as CSSProperties)
    : undefined;
  return {
    window: shown,
    wrapperProps: { ref: wrapRef, className: enabled ? "todo-table--virtual" : undefined, style },
  };
}
