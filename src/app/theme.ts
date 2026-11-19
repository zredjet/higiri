import { useEffect } from "react";
import type { Theme } from "../lib/bindings";

export type ResolvedTheme = "light" | "dark";

export function resolveTheme(theme: Theme, prefersDark: boolean): ResolvedTheme {
  if (theme === "system") return prefersDark ? "dark" : "light";
  return theme;
}

const DARK_QUERY = "(prefers-color-scheme: dark)";

/** Puts `data-theme` on <html>; follows the OS while the setting is "system". */
export function useAppliedTheme(theme: Theme | undefined): void {
  useEffect(() => {
    const root = document.documentElement;
    const setting = theme ?? "light";
    const media = window.matchMedia(DARK_QUERY);
    const apply = () => {
      root.dataset.theme = resolveTheme(setting, media.matches);
    };
    apply();
    if (setting !== "system") return undefined;
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
}
