import { useEffect, useId, useRef, type InputHTMLAttributes, type ReactElement, type ReactNode } from "react";
import { Icon } from "./Icon";
import { cx } from "./util";

export interface CheckboxProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type"> {
  label?: ReactNode;
  description?: ReactNode;
  /** "Some selected" state, e.g. a table header checkbox. */
  indeterminate?: boolean;
}

export function Checkbox({
  label,
  description,
  indeterminate,
  className,
  id: idProp,
  ...rest
}: CheckboxProps): ReactElement {
  const autoId = useId();
  const id = idProp || autoId;
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = !!indeterminate;
  }, [indeterminate]);
  return (
    <div className={cx("az-check", rest.disabled && "is-disabled", className)}>
      <span className="az-check__box">
        <input
          {...rest}
          ref={ref}
          id={id}
          type="checkbox"
          className="az-check__input"
          aria-describedby={description ? id + "-desc" : undefined}
        />
        <Icon
          name={indeterminate ? "minus" : "check"}
          size={14}
          strokeWidth={2.5}
          className="az-check__mark"
        />
      </span>
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
