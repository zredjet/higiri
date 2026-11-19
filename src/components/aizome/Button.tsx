import type { ButtonHTMLAttributes, ReactElement, ReactNode } from "react";
import { renderIcon, type IconLike } from "./Icon";
import { cx } from "./util";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** primary: at most one per view. Default secondary. */
  variant?: "primary" | "secondary" | "ghost" | "danger";
  /** sm 28px, md 36px, lg 44px. Default md. */
  size?: "sm" | "md" | "lg";
  iconStart?: IconLike;
  iconEnd?: IconLike;
  /** Shows a spinner and disables the button. */
  loading?: boolean;
  fullWidth?: boolean;
  /** Omit for an icon-only square button — then aria-label is required. */
  children?: ReactNode;
}

export function Button({
  variant = "secondary",
  size = "md",
  iconStart,
  iconEnd,
  loading,
  fullWidth,
  className,
  children,
  type,
  disabled,
  ...rest
}: ButtonProps): ReactElement {
  const iconSize = size === "sm" ? 16 : 18;
  const hasLabel = children !== undefined && children !== null && children !== false;
  return (
    <button
      type={type || "button"}
      {...rest}
      className={cx(
        "az-btn",
        "az-btn--" + variant,
        "az-btn--" + size,
        !hasLabel && "az-btn--icon",
        fullWidth && "az-btn--block",
        loading && "az-btn--loading",
        className,
      )}
      disabled={disabled || loading}
      aria-busy={loading ? true : undefined}
    >
      {loading ? <span className="az-spinner" aria-hidden={true} /> : renderIcon(iconStart, iconSize)}
      {hasLabel ? <span className="az-btn__label">{children}</span> : null}
      {renderIcon(iconEnd, iconSize)}
    </button>
  );
}
