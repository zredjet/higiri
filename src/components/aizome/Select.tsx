import { useId, type ReactElement, type ReactNode, type SelectHTMLAttributes } from "react";
import { Icon } from "./Icon";
import { cx } from "./util";

export interface SelectOption {
  value: string;
  label: ReactNode;
  disabled?: boolean;
}

export interface SelectProps extends Omit<SelectHTMLAttributes<HTMLSelectElement>, "size"> {
  label?: ReactNode;
  description?: ReactNode;
  error?: ReactNode;
  options?: Array<SelectOption | string>;
  /** Disabled first option shown until a value is chosen. */
  placeholder?: string;
  size?: "sm" | "md" | "lg";
}

export function Select({
  label,
  description,
  error,
  options,
  placeholder,
  size,
  className,
  id: idProp,
  children,
  required,
  disabled,
  ...rest
}: SelectProps): ReactElement {
  const autoId = useId();
  const id = idProp || autoId;
  const descId = id + "-desc";
  const errId = id + "-err";
  const describedBy =
    [description && !error ? descId : null, error ? errId : null].filter(Boolean).join(" ") ||
    undefined;
  const extra =
    placeholder && rest.value === undefined && rest.defaultValue === undefined
      ? { defaultValue: "" }
      : {};
  const opts = (options || []).map((o) => {
    const v = typeof o === "string" ? o : o.value;
    const l = typeof o === "string" ? o : o.label;
    return (
      <option key={v} value={v} disabled={typeof o === "string" ? undefined : o.disabled}>
        {l}
      </option>
    );
  });
  if (placeholder) {
    opts.unshift(
      <option key="__placeholder" value="" disabled>
        {placeholder}
      </option>,
    );
  }
  return (
    <div className={cx("az-field", className)}>
      {label ? (
        <label className="az-field__label" htmlFor={id}>
          {label}
          {required ? <span className="az-field__req">必須</span> : null}
        </label>
      ) : null}
      <div
        className={cx(
          "az-input",
          "az-select",
          "az-input--" + (size || "md"),
          Boolean(error) && "az-input--invalid",
          disabled && "az-input--disabled",
        )}
      >
        <select
          {...rest}
          {...extra}
          disabled={disabled}
          id={id}
          className="az-input__control"
          required={required}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
        >
          {opts}
          {children}
        </select>
        <Icon name="chevron-down" size={16} className="az-select__chevron" />
      </div>
      {description && !error ? (
        <p id={descId} className="az-field__desc">
          {description}
        </p>
      ) : null}
      {error ? (
        <p id={errId} className="az-field__error">
          <Icon name="alert-circle" size={14} />
          <span>{error}</span>
        </p>
      ) : null}
    </div>
  );
}
