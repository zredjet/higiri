import { useCallback, useState } from "react";

/** Joins truthy class names. */
export function cx(...names: Array<string | false | null | undefined>): string {
  return names.filter(Boolean).join(" ");
}

/** Controlled when `value` is given, uncontrolled otherwise. */
export function useControllable<T>(
  value: T | undefined,
  defaultValue: T,
  onChange?: (next: T) => void,
): [T, (next: T) => void] {
  const [state, setState] = useState<T>(defaultValue);
  const controlled = value !== undefined;
  const current = controlled ? value : state;
  const set = useCallback(
    (next: T) => {
      if (!controlled) setState(next);
      if (onChange) onChange(next);
    },
    [controlled, onChange],
  );
  return [current, set];
}
