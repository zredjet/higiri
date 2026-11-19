import { createElement, type ReactElement, type ReactNode, type SVGProps } from "react";
import { cx } from "./util";

type Shape = [tag: "path" | "circle" | "rect", attrs: SVGProps<SVGElement>];

/** Built-in icons: 24px grid, 1.75 stroke, round caps (docs/aizome bundle.js). */
const ICONS = {
  check: [["path", { d: "M5 12.5l4.5 4.5L19 7.5" }]],
  minus: [["path", { d: "M6 12h12" }]],
  plus: [["path", { d: "M12 5v14M5 12h14" }]],
  x: [["path", { d: "M6 6l12 12M18 6L6 18" }]],
  "chevron-down": [["path", { d: "M6 9l6 6 6-6" }]],
  "chevron-right": [["path", { d: "M9 6l6 6-6 6" }]],
  "chevron-left": [["path", { d: "M15 6l-6 6 6 6" }]],
  "arrow-right": [["path", { d: "M5 12h14M13 6l6 6-6 6" }]],
  search: [
    ["circle", { cx: 11, cy: 11, r: 6.5 }],
    ["path", { d: "M16 16l4 4" }],
  ],
  info: [
    ["circle", { cx: 12, cy: 12, r: 9 }],
    ["path", { d: "M12 11v5.5M12 7.6v.01" }],
  ],
  "check-circle": [
    ["circle", { cx: 12, cy: 12, r: 9 }],
    ["path", { d: "M8.5 12.3l2.4 2.4 4.6-5" }],
  ],
  "alert-triangle": [
    ["path", { d: "M12 4.2L3 19.5h18z" }],
    ["path", { d: "M12 10v4.2M12 16.9v.01" }],
  ],
  "alert-circle": [
    ["circle", { cx: 12, cy: 12, r: 9 }],
    ["path", { d: "M12 7.5v5.2M12 16.2v.01" }],
  ],
  home: [
    ["path", { d: "M4 11l8-6.5 8 6.5" }],
    ["path", { d: "M6 9.5V20h12V9.5" }],
  ],
  inbox: [
    ["path", { d: "M4 13.5L6.5 5h11L20 13.5V19H4z" }],
    ["path", { d: "M4 13.5h4.5l1.2 2.2h4.6l1.2-2.2H20" }],
  ],
  users: [
    ["circle", { cx: 9, cy: 8.5, r: 3.5 }],
    [
      "path",
      {
        d: "M3 19.5c0-3.3 2.7-5.5 6-5.5s6 2.2 6 5.5M15.5 5.2a3.5 3.5 0 0 1 0 6.6M17.5 14.4c2 .7 3.5 2.5 3.5 5.1",
      },
    ],
  ],
  chart: [["path", { d: "M4 4v16h16M8.5 16v-4M12.5 16V8M16.5 16v-6" }]],
  file: [
    ["path", { d: "M7 3.5h7l4 4v13H7z" }],
    ["path", { d: "M14 3.5V8h4M10 12.5h5M10 16h5" }],
  ],
  folder: [["path", { d: "M3.5 6.5h6l2 2h9v10h-17z" }]],
  "credit-card": [
    ["rect", { x: 3, y: 5.5, width: 18, height: 13, rx: 2 }],
    ["path", { d: "M3 10h18M7 15h3" }],
  ],
  settings: [
    ["path", { d: "M4 7h9M17 7h3M4 17h3M11 17h9" }],
    ["circle", { cx: 15, cy: 7, r: 2 }],
    ["circle", { cx: 9, cy: 17, r: 2 }],
  ],
  bell: [
    ["path", { d: "M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 2h-15z" }],
    ["path", { d: "M10 21h4" }],
  ],
  download: [["path", { d: "M12 4v11M7 10.5l5 4.5 5-4.5M5 20h14" }]],
  calendar: [
    ["rect", { x: 4, y: 5.5, width: 16, height: 14.5, rx: 2 }],
    ["path", { d: "M4 10h16M8.5 3.5v4M15.5 3.5v4" }],
  ],
  more: [
    ["circle", { cx: 6, cy: 12, r: 1.4, fill: "currentColor", stroke: "none" }],
    ["circle", { cx: 12, cy: 12, r: 1.4, fill: "currentColor", stroke: "none" }],
    ["circle", { cx: 18, cy: 12, r: 1.4, fill: "currentColor", stroke: "none" }],
  ],
  "log-out": [["path", { d: "M10 4.5H5.5v15H10M14 8l4 4-4 4M18 12H9" }]],
} satisfies Record<string, Shape[]>;

export type IconName = keyof typeof ICONS;

export const ICON_NAMES = Object.keys(ICONS) as IconName[];

export interface IconProps {
  name: IconName;
  /** px. 16 in small controls, 18 in buttons and nav, 20 standalone. Default 20. */
  size?: number;
  strokeWidth?: number;
  /** Accessible name. Omit for decorative icons (aria-hidden). */
  label?: string;
  className?: string;
}

export function Icon({ name, size = 20, strokeWidth, label, className }: IconProps): ReactElement | null {
  const spec: Shape[] | undefined = ICONS[name];
  if (!spec) return null;
  const a11y = label ? { role: "img", "aria-label": label } : { "aria-hidden": true };
  return createElement(
    "svg",
    {
      viewBox: "0 0 24 24",
      width: size,
      height: size,
      fill: "none",
      stroke: "currentColor",
      strokeWidth: strokeWidth || 1.75,
      strokeLinecap: "round",
      strokeLinejoin: "round",
      className: cx("az-icon", className),
      focusable: "false",
      ...a11y,
    },
    ...spec.map(([tag, attrs], i) => createElement(tag, { key: i, ...attrs })),
  );
}

/** A built-in icon name or any node. */
export type IconLike = IconName | ReactNode;

export function renderIcon(icon: IconLike | undefined, size: number): ReactNode {
  if (!icon) return null;
  if (typeof icon === "string") return <Icon name={icon as IconName} size={size} />;
  return icon;
}
