import type { ReactElement, ReactNode } from "react";
import { Button } from "./Button";
import { Icon, type IconName } from "./Icon";
import { cx } from "./util";

export type AlertTone = "info" | "success" | "warning" | "danger";

const ALERT_ICON: Record<AlertTone, IconName> = {
  info: "info",
  success: "check-circle",
  warning: "alert-triangle",
  danger: "alert-circle",
};

export interface AlertProps {
  tone?: AlertTone;
  title?: ReactNode;
  children?: ReactNode;
  /** Buttons under the message (size="sm"). */
  action?: ReactNode;
  /** Shows a close button. */
  onClose?: () => void;
  className?: string;
}

export function Alert({ tone = "info", title, children, action, onClose, className }: AlertProps): ReactElement {
  return (
    <div
      className={cx("az-alert", "az-alert--" + tone, className)}
      role={tone === "danger" || tone === "warning" ? "alert" : "status"}
    >
      <Icon name={ALERT_ICON[tone]} size={20} className="az-alert__icon" />
      <div className="az-alert__content">
        {title ? <p className="az-alert__title">{title}</p> : null}
        {children ? <div className="az-alert__body">{children}</div> : null}
        {action ? <div className="az-alert__action">{action}</div> : null}
      </div>
      {onClose ? (
        <Button
          variant="ghost"
          size="sm"
          iconStart="x"
          aria-label="閉じる"
          className="az-alert__close"
          onClick={onClose}
        />
      ) : null}
    </div>
  );
}
