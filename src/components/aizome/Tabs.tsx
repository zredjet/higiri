import { useId, type KeyboardEvent, type ReactElement, type ReactNode } from "react";
import { cx, useControllable } from "./util";

export interface TabItem {
  id: string;
  label: ReactNode;
  count?: number | string;
  disabled?: boolean;
  content?: ReactNode;
}

export interface TabsProps {
  items: TabItem[];
  value?: string;
  defaultValue?: string;
  onChange?: (id: string) => void;
  /** Accessible name of the tab list. */
  label?: string;
  className?: string;
}

export function Tabs({ items = [], value, defaultValue, onChange, label, className }: TabsProps): ReactElement {
  const base = useId();
  const [current, set] = useControllable<string | undefined>(
    value,
    defaultValue !== undefined ? defaultValue : items[0] && items[0].id,
    onChange as ((id: string | undefined) => void) | undefined,
  );
  const active = items.find((t) => t.id === current) ?? null;

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (["ArrowRight", "ArrowLeft", "Home", "End"].indexOf(e.key) < 0) return;
    const enabled = items.filter((t) => !t.disabled);
    const idx = enabled.findIndex((t) => t.id === current);
    const next =
      e.key === "ArrowRight"
        ? enabled[(idx + 1) % enabled.length]
        : e.key === "ArrowLeft"
          ? enabled[(idx - 1 + enabled.length) % enabled.length]
          : e.key === "Home"
            ? enabled[0]
            : enabled[enabled.length - 1];
    if (!next) return;
    e.preventDefault();
    set(next.id);
    const el = e.currentTarget.querySelector<HTMLElement>(
      '[data-tab-id="' + String(next.id).replace(/"/g, '\\"') + '"]',
    );
    if (el) el.focus();
  }

  return (
    <div className={cx("az-tabs", className)}>
      <div role="tablist" className="az-tabs__list" aria-label={label} onKeyDown={onKeyDown}>
        {items.map((t) => {
          const sel = t.id === current;
          return (
            <button
              key={t.id}
              type="button"
              role="tab"
              id={base + "-tab-" + t.id}
              data-tab-id={t.id}
              aria-selected={sel}
              aria-controls={t.content !== undefined ? base + "-panel-" + t.id : undefined}
              tabIndex={sel ? 0 : -1}
              disabled={t.disabled}
              className="az-tabs__tab"
              onClick={() => set(t.id)}
            >
              {t.label}
              {t.count !== undefined && t.count !== null ? (
                <span className="az-tabs__count">{t.count}</span>
              ) : null}
            </button>
          );
        })}
      </div>
      {active && active.content !== undefined ? (
        <div
          role="tabpanel"
          id={base + "-panel-" + active.id}
          aria-labelledby={base + "-tab-" + active.id}
          className="az-tabs__panel"
          tabIndex={0}
        >
          {active.content}
        </div>
      ) : null}
    </div>
  );
}
