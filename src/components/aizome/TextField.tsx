import {
  createElement,
  useId,
  type ChangeEvent,
  type InputHTMLAttributes,
  type ReactElement,
  type ReactNode,
  type TextareaHTMLAttributes,
} from "react";
import { Icon, renderIcon, type IconLike } from "./Icon";
import { cx } from "./util";

type ControlProps = Omit<InputHTMLAttributes<HTMLInputElement>, "size"> &
  Pick<TextareaHTMLAttributes<HTMLTextAreaElement>, "rows">;

export interface TextFieldProps extends Omit<ControlProps, "onChange"> {
  label?: ReactNode;
  /** Help text under the field. Hidden while error is set. */
  description?: ReactNode;
  /** Error message; marks the field invalid. */
  error?: ReactNode;
  /** Renders a textarea. */
  multiline?: boolean;
  rows?: number;
  size?: "sm" | "md" | "lg";
  /** Leading icon inside the field. */
  icon?: IconLike;
  /** Trailing unit text, e.g. "円" or "件". */
  suffix?: ReactNode;
  onChange?: (e: ChangeEvent<HTMLInputElement & HTMLTextAreaElement>) => void;
}

export function TextField({
  label,
  description,
  error,
  multiline,
  rows,
  size,
  className,
  id: idProp,
  icon,
  suffix,
  required,
  disabled,
  ...rest
}: TextFieldProps): ReactElement {
  const autoId = useId();
  const id = idProp || autoId;
  const descId = id + "-desc";
  const errId = id + "-err";
  const describedBy =
    [description && !error ? descId : null, error ? errId : null].filter(Boolean).join(" ") ||
    undefined;
  const control = createElement(multiline ? "textarea" : "input", {
    ...rest,
    disabled,
    id,
    className: "az-input__control",
    rows: multiline ? rows || 3 : undefined,
    required,
    "aria-invalid": error ? true : undefined,
    "aria-describedby": describedBy,
  });
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
          "az-input--" + (size || "md"),
          multiline && "az-input--multiline",
          Boolean(error) && "az-input--invalid",
          disabled && "az-input--disabled",
        )}
      >
        {icon ? <span className="az-input__affix">{renderIcon(icon, 18)}</span> : null}
        {control}
        {suffix ? <span className="az-input__affix">{suffix}</span> : null}
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
