import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { commands, type StartupReminder } from "../lib/bindings";
import { todayYmd } from "../lib/date";

/**
 * The startup reminder (D-2): asked for once at startup, and again when the
 * window comes back to the front on a new day (the app was left open over
 * midnight) — after refreshing every list, since the categories moved.
 */
export function useStartupReminder() {
  const qc = useQueryClient();
  const [reminder, setReminder] = useState<StartupReminder | null>(null);
  const day = useRef(todayYmd());

  const check = useCallback(() => {
    commands
      .getStartupReminder()
      .then(setReminder)
      .catch((e: unknown) => console.error(e));
  }, []);

  useEffect(() => {
    check();
    function onFront() {
      if (document.visibilityState !== "visible") return;
      const today = todayYmd();
      if (today === day.current) return;
      day.current = today;
      void qc.invalidateQueries();
      check();
    }
    window.addEventListener("focus", onFront);
    document.addEventListener("visibilitychange", onFront);
    return () => {
      window.removeEventListener("focus", onFront);
      document.removeEventListener("visibilitychange", onFront);
    };
  }, [check, qc]);

  /** Closing the dialog records that today's reminder was seen. */
  const acknowledge = useCallback(() => {
    setReminder(null);
    commands.ackReminder().catch((e: unknown) => console.error(e));
  }, []);

  return { reminder, acknowledge };
}
