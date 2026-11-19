import type * as React from 'react';

/** Names of the built-in icons (24px grid, 1.75 stroke). */
export type IconName =
  | 'check' | 'minus' | 'plus' | 'x' | 'chevron-down' | 'chevron-right' | 'chevron-left' | 'arrow-right'
  | 'search' | 'info' | 'check-circle' | 'alert-triangle' | 'alert-circle'
  | 'home' | 'inbox' | 'users' | 'chart' | 'file' | 'folder' | 'credit-card' | 'settings' | 'bell'
  | 'download' | 'calendar' | 'more' | 'log-out';

export interface IconProps {
  name: IconName;
  /** px. 16 in small controls, 18 in buttons and nav, 20 standalone. Default 20. */
  size?: number;
  strokeWidth?: number;
  /** Accessible name. Omit for decorative icons (aria-hidden). */
  label?: string;
  className?: string;
}
export declare function Icon(props: IconProps): React.ReactElement | null;

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** primary: at most one per view. Default secondary. */
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  /** sm 28px, md 36px, lg 44px. Default md. */
  size?: 'sm' | 'md' | 'lg';
  iconStart?: IconName | React.ReactNode;
  iconEnd?: IconName | React.ReactNode;
  /** Shows a spinner and disables the button. */
  loading?: boolean;
  fullWidth?: boolean;
  /** Omit for an icon-only square button — then aria-label is required. */
  children?: React.ReactNode;
}
export declare function Button(props: ButtonProps): React.ReactElement;

export interface TextFieldProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
  label?: React.ReactNode;
  /** Help text under the field. Hidden while error is set. */
  description?: React.ReactNode;
  /** Error message; marks the field invalid. */
  error?: React.ReactNode;
  /** Renders a textarea. */
  multiline?: boolean;
  rows?: number;
  size?: 'sm' | 'md' | 'lg';
  /** Leading icon inside the field. */
  icon?: IconName | React.ReactNode;
  /** Trailing unit text, e.g. "円" or "件". */
  suffix?: React.ReactNode;
}
export declare function TextField(props: TextFieldProps): React.ReactElement;

export interface SelectOption { value: string; label: React.ReactNode; disabled?: boolean }
export interface SelectProps extends Omit<React.SelectHTMLAttributes<HTMLSelectElement>, 'size'> {
  label?: React.ReactNode;
  description?: React.ReactNode;
  error?: React.ReactNode;
  options?: Array<SelectOption | string>;
  /** Disabled first option shown until a value is chosen. */
  placeholder?: string;
  size?: 'sm' | 'md' | 'lg';
}
export declare function Select(props: SelectProps): React.ReactElement;

export interface CheckboxProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'type'> {
  label?: React.ReactNode;
  description?: React.ReactNode;
  /** "Some selected" state, e.g. a table header checkbox. */
  indeterminate?: boolean;
}
export declare function Checkbox(props: CheckboxProps): React.ReactElement;

export interface RadioOption { value: string; label: React.ReactNode; description?: React.ReactNode; disabled?: boolean }
export interface RadioGroupProps {
  label?: React.ReactNode;
  options: RadioOption[];
  value?: string;
  defaultValue?: string;
  onChange?: (value: string) => void;
  name?: string;
  orientation?: 'vertical' | 'horizontal';
  disabled?: boolean;
  className?: string;
}
export declare function RadioGroup(props: RadioGroupProps): React.ReactElement;

export interface SwitchProps {
  label?: React.ReactNode;
  description?: React.ReactNode;
  checked?: boolean;
  defaultChecked?: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  id?: string;
  /** Required when there is no visible label. */
  'aria-label'?: string;
  className?: string;
}
export declare function Switch(props: SwitchProps): React.ReactElement;

export interface BadgeProps {
  /** primary doubles as "info". Default neutral. */
  tone?: 'neutral' | 'primary' | 'success' | 'warning' | 'danger';
  /** Leading status dot. */
  dot?: boolean;
  children?: React.ReactNode;
  className?: string;
}
export declare function Badge(props: BadgeProps): React.ReactElement;

export interface AlertProps {
  tone?: 'info' | 'success' | 'warning' | 'danger';
  title?: React.ReactNode;
  children?: React.ReactNode;
  /** Buttons under the message (size="sm"). */
  action?: React.ReactNode;
  /** Shows a close button. */
  onClose?: () => void;
  className?: string;
}
export declare function Alert(props: AlertProps): React.ReactElement;

export interface CardProps extends Omit<React.HTMLAttributes<HTMLElement>, 'title'> {
  title?: React.ReactNode;
  description?: React.ReactNode;
  /** Buttons at the header's right. */
  actions?: React.ReactNode;
  /** Right-aligned footer row, usually buttons. */
  footer?: React.ReactNode;
  /** "none" removes body padding (e.g. a full-bleed Table). */
  padding?: 'default' | 'none';
  /** Element to render, default "section". */
  as?: keyof JSX.IntrinsicElements;
  children?: React.ReactNode;
}
export declare function Card(props: CardProps): React.ReactElement;

export interface TabItem { id: string; label: React.ReactNode; count?: number | string; disabled?: boolean; content?: React.ReactNode }
export interface TabsProps {
  items: TabItem[];
  value?: string;
  defaultValue?: string;
  onChange?: (id: string) => void;
  /** Accessible name of the tab list. */
  label?: string;
  className?: string;
}
export declare function Tabs(props: TabsProps): React.ReactElement;

export interface TableColumn<Row = any> {
  key: string;
  header: React.ReactNode;
  /** Right-aligned, tabular numerals. */
  numeric?: boolean;
  align?: 'left' | 'center' | 'right';
  width?: number | string;
  render?: (row: Row) => React.ReactNode;
}
export interface TableProps<Row = any> {
  columns: TableColumn<Row>[];
  rows: Row[];
  /** Row field used as the React key. Default "id". */
  rowKey?: string;
  selectedKey?: string | number;
  onRowClick?: (row: Row) => void;
  density?: 'default' | 'compact';
  caption?: React.ReactNode;
  emptyText?: React.ReactNode;
  /** Scrolls the body with a sticky header. */
  maxHeight?: number | string;
  className?: string;
}
export declare function Table<Row = any>(props: TableProps<Row>): React.ReactElement;

export interface DialogProps {
  open: boolean;
  onClose?: () => void;
  title: React.ReactNode;
  description?: React.ReactNode;
  children?: React.ReactNode;
  /** Right-aligned buttons. Primary last. */
  footer?: React.ReactNode;
  size?: 'sm' | 'md' | 'lg';
  /** "danger" renders role="alertdialog" for destructive confirmations. */
  tone?: 'default' | 'danger';
  /** false: Esc and the scrim do not close it. */
  dismissible?: boolean;
  /** false renders in place instead of into document.body. */
  portal?: boolean;
  className?: string;
}
export declare function Dialog(props: DialogProps): React.ReactElement | null;

export interface SideNavItem { id: string; label: React.ReactNode; icon?: IconName | React.ReactNode; badge?: React.ReactNode; href?: string }
export interface SideNavSection { label?: React.ReactNode; items: SideNavItem[] }
export interface SideNavProps {
  sections: SideNavSection[];
  activeId?: string;
  defaultActiveId?: string;
  onSelect?: (id: string) => void;
  /** Product name or workspace switcher at the top. */
  header?: React.ReactNode;
  footer?: React.ReactNode;
  label?: string;
  className?: string;
}
export declare function SideNav(props: SideNavProps): React.ReactElement;

declare global {
  interface Window {
    Aizome: {
      Button: typeof Button; TextField: typeof TextField; Select: typeof Select; Checkbox: typeof Checkbox;
      RadioGroup: typeof RadioGroup; Switch: typeof Switch; Badge: typeof Badge; Alert: typeof Alert;
      Card: typeof Card; Tabs: typeof Tabs; Table: typeof Table; Dialog: typeof Dialog; SideNav: typeof SideNav; Icon: typeof Icon;
    };
  }
}
