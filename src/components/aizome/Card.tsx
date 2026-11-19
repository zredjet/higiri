import { createElement, type HTMLAttributes, type ReactElement, type ReactNode } from "react";
import type * as React from "react";
import { cx } from "./util";

export interface CardProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  title?: ReactNode;
  description?: ReactNode;
  /** Buttons at the header's right. */
  actions?: ReactNode;
  /** Right-aligned footer row, usually buttons. */
  footer?: ReactNode;
  /** "none" removes body padding (e.g. a full-bleed Table). */
  padding?: "default" | "none";
  /** Element to render, default "section". */
  as?: keyof React.JSX.IntrinsicElements;
  children?: ReactNode;
}

export function Card({
  title,
  description,
  actions,
  footer,
  children,
  padding,
  className,
  as,
  ...rest
}: CardProps): ReactElement {
  const interactive = !!rest.onClick;
  const extra = interactive
    ? { tabIndex: rest.tabIndex !== undefined ? rest.tabIndex : 0, role: rest.role || "button" }
    : {};
  return createElement(
    as || "section",
    {
      ...rest,
      ...extra,
      className: cx(
        "az-card",
        padding === "none" && "az-card--flush",
        interactive && "az-card--interactive",
        className,
      ),
    },
    title || actions ? (
      <header className="az-card__header">
        <div>
          {title ? <h3 className="az-card__title">{title}</h3> : null}
          {description ? <p className="az-card__desc">{description}</p> : null}
        </div>
        {actions ? <div className="az-card__actions">{actions}</div> : null}
      </header>
    ) : null,
    children !== undefined ? <div className="az-card__body">{children}</div> : null,
    footer ? <footer className="az-card__footer">{footer}</footer> : null,
  );
}
