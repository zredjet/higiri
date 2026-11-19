import { createElement, type ReactElement, type ReactNode } from "react";
import { Icon, type IconLike, type IconName } from "./Icon";
import { cx, useControllable } from "./util";

export interface SideNavItem {
  id: string;
  label: ReactNode;
  icon?: IconLike;
  badge?: ReactNode;
  href?: string;
}

export interface SideNavSection {
  label?: ReactNode;
  items: SideNavItem[];
}

export interface SideNavProps {
  sections: SideNavSection[];
  activeId?: string;
  defaultActiveId?: string;
  onSelect?: (id: string) => void;
  /** Product name or workspace switcher at the top. */
  header?: ReactNode;
  footer?: ReactNode;
  label?: string;
  className?: string;
}

export function SideNav({
  sections = [],
  activeId,
  defaultActiveId,
  onSelect,
  header,
  footer,
  label,
  className,
}: SideNavProps): ReactElement {
  const [current, set] = useControllable<string | undefined>(
    activeId,
    defaultActiveId,
    onSelect as ((id: string | undefined) => void) | undefined,
  );
  return (
    <nav className={cx("az-sidenav", className)} aria-label={label || "メインメニュー"}>
      {header ? <div className="az-sidenav__header">{header}</div> : null}
      <div className="az-sidenav__body">
        {sections.map((s, si) => (
          <div key={typeof s.label === "string" ? s.label : si} className="az-sidenav__section">
            {s.label ? <p className="az-sidenav__heading">{s.label}</p> : null}
            <ul className="az-sidenav__list">
              {(s.items || []).map((it) => {
                const active = it.id === current;
                return (
                  <li key={it.id}>
                    {createElement(
                      it.href ? "a" : "button",
                      {
                        href: it.href,
                        type: it.href ? undefined : "button",
                        className: cx("az-sidenav__item", active && "is-active"),
                        "aria-current": active ? "page" : undefined,
                        onClick: () => set(it.id),
                      },
                      typeof it.icon === "string" ? (
                        <Icon name={it.icon as IconName} size={18} className="az-sidenav__icon" />
                      ) : (
                        it.icon || null
                      ),
                      <span className="az-sidenav__label">{it.label}</span>,
                      it.badge !== undefined && it.badge !== null ? (
                        <span className="az-sidenav__badge">{it.badge}</span>
                      ) : null,
                    )}
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </div>
      {footer ? <div className="az-sidenav__footer">{footer}</div> : null}
    </nav>
  );
}
