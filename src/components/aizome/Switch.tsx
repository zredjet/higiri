import { useId, type ReactElement, type ReactNode } from "react";
import { cx, useControllable } from "./util";

export interface SwitchProps {
  label?: ReactNode;
  description?: ReactNode;
  checked?: boolean;
  defaultChecked?: boolean;
  onChange?: (checked: boolean) => void;
  disabled?: boolean;
  id?: string;
  /** Required when there is no visible label. */
  "aria-label"?: string;
  className?: string;
}

export function Switch({
  label,
  description,
  checked,
  defaultChecked,
  onChange,
  disabled,
  id: idProp,
  "aria-label": ariaLabel,
  className,
}: SwitchProps): ReactElement {
  const autoId = useId();
  const id = idProp || autoId;
  const [on, set] = useControllable(checked, !!defaultChecked, onChange);
  return (
    <div className={cx("az-switch", disabled && "is-disabled", className)}>
      <button
        type="button"
        role="switch"
        id={id}
        aria-checked={!!on}
        disabled={disabled}
        className="az-switch__track"
        onClick={() => set(!on)}
        aria-describedby={description ? id + "-desc" : undefined}
        aria-label={label ? undefined : ariaLabel}
      >
        <span className="az-switch__thumb" />
      </button>
      {label || description ? (
        <span className="az-check__text">
          {label ? (
            <label htmlFor={id} className="az-check__label">
              {label}
            </label>
          ) : null}
          {description ? (
            <span id={id + "-desc"} className="az-check__desc">
              {description}
            </span>
          ) : null}
        </span>
      ) : null}
    </div>
  );
}
