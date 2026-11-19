import { useEffect, useId, useRef, type ReactElement, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Button } from "./Button";
import { cx } from "./util";

export interface DialogProps {
  open: boolean;
  onClose?: () => void;
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  /** Right-aligned buttons. Primary last. */
  footer?: ReactNode;
  size?: "sm" | "md" | "lg";
  /** "danger" renders role="alertdialog" for destructive confirmations. */
  tone?: "default" | "danger";
  /** false: Esc and the scrim do not close it. */
  dismissible?: boolean;
  /** false renders in place instead of into document.body. */
  portal?: boolean;
  className?: string;
}

export function Dialog({
  open,
  onClose,
  title,
  description,
  children,
  footer,
  size,
  tone,
  dismissible,
  portal,
  className,
}: DialogProps): ReactElement | null {
  const titleId = useId();
  const descId = titleId + "-desc";
  const panel = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return undefined;
    const prev = document.activeElement as HTMLElement | null;
    if (panel.current) panel.current.focus();
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape" && onClose && dismissible !== false) onClose();
    }
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      if (prev && prev.focus) prev.focus();
    };
  }, [open, onClose, dismissible]);

  if (!open) return null;
  const node = (
    <div
      className="az-dialog__overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && onClose && dismissible !== false) onClose();
      }}
    >
      <div
        ref={panel}
        tabIndex={-1}
        role={tone === "danger" ? "alertdialog" : "dialog"}
        aria-modal={true}
        aria-labelledby={titleId}
        aria-describedby={description ? descId : undefined}
        className={cx("az-dialog", "az-dialog--" + (size || "md"), className)}
      >
        <div className="az-dialog__header">
          <h2 id={titleId} className="az-dialog__title">
            {title}
          </h2>
          {onClose && dismissible !== false ? (
            <Button
              variant="ghost"
              size="sm"
              iconStart="x"
              aria-label="閉じる"
              className="az-dialog__close"
              onClick={onClose}
            />
          ) : null}
        </div>
        {description ? (
          <p id={descId} className="az-dialog__desc">
            {description}
          </p>
        ) : null}
        {children ? <div className="az-dialog__body">{children}</div> : null}
        {footer ? <div className="az-dialog__footer">{footer}</div> : null}
      </div>
    </div>
  );
  if (portal !== false && typeof document !== "undefined") return createPortal(node, document.body);
  return node;
}
