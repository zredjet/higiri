import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReportKey } from "../../lib/bindings";
import { useReportSource, useSaveReport } from "../../lib/queries";
import { AUTOSAVE_MS } from "../todos/detail/useTodoDraft";

/** The text on screen and the day it belongs to. */
export interface ReportDoc {
  key: ReportKey;
  text: string;
}

export function dailyKey(date: string): ReportKey {
  return { kind: "daily", date };
}

/**
 * The report being edited for `date`. Opens the saved report of that day, or
 * a generated one when none is saved, without saving it. Edits are saved 1
 * second after the last keystroke, and when the day changes or the screen
 * closes; `saveNow` saves at once (再生成, コピー, .md で保存).
 */
export function useReportDraft(date: string) {
  const key = useMemo(() => dailyKey(date), [date]);
  const source = useReportSource(key);
  const { mutateAsync, error, reset } = useSaveReport();
  const [doc, setDoc] = useState<ReportDoc | null>(null);
  const latest = useRef(doc);
  latest.current = doc;
  const dirty = useRef(false);
  const timer = useRef<number | null>(null);

  // Take the loaded text once per day; later refetches never replace edits.
  useEffect(() => {
    if (source.data !== undefined && latest.current?.key.date !== key.date) {
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
        if (latest.current?.key.date === d.key.date) dirty.current = true;
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

  /** Puts a regenerated text for `forKey` on screen (if that day is still shown) and saves it. */
  const replace = useCallback(
    async (forKey: ReportKey, text: string) => {
      if (latest.current?.key.date === forKey.date) {
        const next = { key: forKey, text };
        latest.current = next;
        setDoc(next);
      }
      await save({ key: forKey, text });
    },
    [save],
  );

  // Save unsaved edits when the day changes or the screen closes. The doc of
  // the old day is still current when this cleanup runs.
  useEffect(
    () => () => {
      void flush();
    },
    [date, flush],
  );

  const ready = doc !== null && doc.key.date === date;
  return {
    doc: ready ? doc : null,
    loadError: source.error,
    saveError: error,
    edit,
    replace,
    saveNow,
  };
}
