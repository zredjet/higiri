import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Button } from "./aizome";
import { cx } from "./aizome/util";

export interface MenuItem {
  label: string;
  onSelect: () => void;
  /** Destructive action (shown in the danger colour, with its label). */
  danger?: boolean;
}

/**
 * An icon button that opens a small list of actions (Aizome has no menu).
 * Esc or a click outside closes it; arrow keys move between items.
 */
export function MenuButton({ label, items }: { label: string; items: MenuItem[] }) {
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const wrap = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return undefined;
    list.current?.querySelector<HTMLElement>("[role=menuitem]")?.focus();
    function onDown(e: MouseEvent) {
      if (wrap.current && !wrap.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    const nodes = Array.from(list.current?.querySelectorAll<HTMLElement>("[role=menuitem]") ?? []);
    const i = nodes.indexOf(document.activeElement as HTMLElement);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      wrap.current?.querySelector<HTMLElement>("button")?.focus();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      nodes[(i + 1) % nodes.length]?.focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      nodes[(i - 1 + nodes.length) % nodes.length]?.focus();
    }
  }

  return (
    <div className="menu" ref={wrap}>
      <Button
        variant="ghost"
        size="sm"
        iconStart="more"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => setOpen((o) => !o)}
      />
      {open ? (
        <div id={menuId} role="menu" aria-label={label} className="menu__popover" ref={list} onKeyDown={onKeyDown}>
          {items.map((it) => (
            <button
              key={it.label}
              type="button"
              role="menuitem"
              className={cx("menu__item", it.danger && "is-danger")}
              onClick={() => {
                setOpen(false);
                it.onSelect();
              }}
            >
              {it.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
