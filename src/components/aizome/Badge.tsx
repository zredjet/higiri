import type { ReactElement, ReactNode } from "react";
import { cx } from "./util";

export type BadgeTone = "neutral" | "primary" | "success" | "warning" | "danger";

export interface BadgeProps {
  /** primary doubles as "info". Default neutral. */
  tone?: BadgeTone;
  /** Leading status dot. */
  dot?: boolean;
  children?: ReactNode;
  className?: string;
}

export function Badge({ tone = "neutral", dot, children, className }: BadgeProps): ReactElement {
  return (
    <span className={cx("az-badge", "az-badge--" + tone, className)}>
      {dot ? <span className="az-badge__dot" aria-hidden={true} /> : null}
      {children}
    </span>
  );
}
