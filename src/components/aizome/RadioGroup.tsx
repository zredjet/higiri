import { useId, type ReactElement, type ReactNode } from "react";
import { cx, useControllable } from "./util";

export interface RadioOption {
  value: string;
  label: ReactNode;
  description?: ReactNode;
  disabled?: boolean;
}

export interface RadioGroupProps {
  label?: ReactNode;
  options: RadioOption[];
  value?: string;
  defaultValue?: string;
  onChange?: (value: string) => void;
  name?: string;
  orientation?: "vertical" | "horizontal";
  disabled?: boolean;
  className?: string;
}

export function RadioGroup({
  label,
  options,
  value,
  defaultValue,
  onChange,
  name: nameProp,
  orientation,
  disabled,
  className,
}: RadioGroupProps): ReactElement {
  const autoId = useId();
  const name = nameProp || autoId;
  const [current, set] = useControllable<string | undefined>(value, defaultValue, onChange as
    | ((v: string | undefined) => void)
    | undefined);
  return (
    <fieldset
      className={cx("az-radio-group", orientation === "horizontal" && "az-radio-group--h", className)}
      disabled={disabled}
    >
      {label ? <legend className="az-field__label">{label}</legend> : null}
      <div className="az-radio-group__items">
        {(options || []).map((o) => {
          const oid = name + "-" + o.value;
          return (
            <div key={o.value} className={cx("az-check", o.disabled && "is-disabled")}>
              <span className="az-check__box">
                <input
                  type="radio"
                  id={oid}
                  name={name}
                  value={o.value}
                  checked={current === o.value}
                  disabled={o.disabled}
                  className="az-check__input az-radio__input"
                  onChange={() => set(o.value)}
                  aria-describedby={o.description ? oid + "-desc" : undefined}
                />
              </span>
              <span className="az-check__text">
                <label htmlFor={oid} className="az-check__label">
                  {o.label}
                </label>
                {o.description ? (
                  <span id={oid + "-desc"} className="az-check__desc">
                    {o.description}
                  </span>
                ) : null}
              </span>
            </div>
          );
        })}
      </div>
    </fieldset>
  );
}
