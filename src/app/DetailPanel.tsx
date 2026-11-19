import { useEffect, type ReactNode } from "react";
import { Button } from "../components/aizome";

interface DetailPanelProps {
  /** Small line above the content (the TODO's project in the mock). */
  context: ReactNode;
  onClose: () => void;
  /** Buttons next to the close button. */
  actions?: ReactNode;
  children: ReactNode;
}

/** A dialog or menu is open: Esc belongs to it, not to the panel. */
function overlayOpen(): boolean {
  return document.querySelector(".az-dialog__overlay, .menu__popover") !== null;
}

/**
 * The right-hand panel (max 440px) that shows one TODO next to a list or the
 * Gantt chart. Esc closes it.
 */
export function DetailPanel({ context, onClose, actions, children }: DetailPanelProps) {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape" && !e.defaultPrevented && !overlayOpen()) onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside className="app__detail" aria-label="TODOの詳細">
      <div className="detail-panel__header">
        <span className="text-caption">{context}</span>
        <div className="detail-panel__actions">
          {actions}
          <Button variant="ghost" size="sm" iconStart="x" aria-label="詳細を閉じる" onClick={onClose} />
        </div>
      </div>
      {children}
    </aside>
  );
}
