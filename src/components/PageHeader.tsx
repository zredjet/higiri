import type { ReactNode } from "react";

interface PageHeaderProps {
  title: ReactNode;
  /** Next to the title on the same line (e.g. the dashboard's date). */
  meta?: ReactNode;
  /** Under the title (e.g. a project's counts). */
  subtitle?: ReactNode;
  /** Buttons on the right. At most one primary. */
  actions?: ReactNode;
}

/** Page title (h1) with its primary action on the right, as in every mock. */
export function PageHeader({ title, meta, subtitle, actions }: PageHeaderProps) {
  return (
    <div className="page-header">
      <div className="page-header__titles">
        <div className="page-header__title-row">
          <h1 className="page-title">{title}</h1>
          {meta ? <span className="page-header__meta">{meta}</span> : null}
        </div>
        {subtitle ? <span className="text-caption">{subtitle}</span> : null}
      </div>
      {actions ? <div className="page-header__actions">{actions}</div> : null}
    </div>
  );
}
