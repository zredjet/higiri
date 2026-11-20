import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReportKey } from "../../lib/bindings";
import { useReportSource, useSaveReport } from "../../lib/queries";
import { AUTOSAVE_MS } from "../todos/detail/useTodoDraft";
import type { ShownKind } from "./reportText";

/** The text on screen and the report (day or week) it belongs to. */
export interface ReportDoc {
  key: ReportKey;
  text: string;
}

function sameKey(a: ReportKey | undefined, b: ReportKey): boolean {
  return a !== undefined && a.kind === b.kind && a.date === b.date;
}

/**
 * The report being edited: the daily report of `date`, or the weekly report
 * of the week starting on `date` (its Monday). Opens the saved report, or a
 * generated one when none is saved, without saving it. Edits are saved 1
 * second after the last keystroke, and when the report changes or the screen
 * closes; `saveNow` saves at once (再生成, コピー, .md で保存).
 */
export function useReportDraft(kind: ShownKind, date: string) {
  const key = useMemo<ReportKey>(() => ({ kind, date }), [kind, date]);
  const source = useReportSource(key);
  const { mutateAsync, error, reset } = useSaveReport();
  const [doc, setDoc] = useState<ReportDoc | null>(null);
  const latest = useRef(doc);
  latest.current = doc;
  const dirty = useRef(false);
  const timer = useRef<number | null>(null);

  // Take the loaded text once per report; later refetches never replace edits.
  useEffect(() => {
    if (source.data !== undefined && !sameKey(latest.current?.key, key)) {
      dirty.current = false;
      setDoc({ key, text: source.data });
    }
  }, [source.data, key]);

  const save = useCallback(
    async (d: ReportDoc) => {
      if (timer.current !== null) {
        window.clearTimeout(timer.current);
        timer.current = null;
      }
      dirty.current = false;
      try {
        await mutateAsync({ key: d.key, bodyMd: d.text });
      } catch {
        // Keep the edits; the error is shown and the next change retries.
        if (sameKey(latest.current?.key, d.key)) dirty.current = true;
      }
    },
    [mutateAsync],
  );

  const flush = useCallback(async () => {
    const d = latest.current;
    if (d && dirty.current) await save(d);
  }, [save]);

  /** Saves what is on screen now, edited or not. */
  const saveNow = useCallback(async () => {
    const d = latest.current;
    if (d) await save(d);
  }, [save]);

  const edit = useCallback(
    (text: string) => {
      const d = latest.current;
      if (!d || d.text === text) return;
      const next = { ...d, text };
      latest.current = next;
      setDoc(next);
      dirty.current = true;
      reset();
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => void flush(), AUTOSAVE_MS);
    },
    [flush, reset],
  );

  /** Puts a regenerated text for `forKey` on screen (if that report is still shown) and saves it. */
  const replace = useCallback(
    async (forKey: ReportKey, text: string) => {
      if (sameKey(latest.current?.key, forKey)) {
        const next = { key: forKey, text };
        latest.current = next;
        setDoc(next);
      }
      await save({ key: forKey, text });
    },
    [save],
  );

  // Save unsaved edits when the report changes or the screen closes. The doc
  // of the old report is still current when this cleanup runs.
  useEffect(
    () => () => {
      void flush();
    },
    [key, flush],
  );

  const ready = doc !== null && sameKey(doc.key, key);
  return {
    doc: ready ? doc : null,
    loadError: source.error,
    saveError: error,
    edit,
    replace,
    saveNow,
  };
}
