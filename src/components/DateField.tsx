import { TextField, type TextFieldProps } from "./aizome";

interface DateFieldProps extends Omit<TextFieldProps, "value" | "onChange" | "type"> {
  /** "YYYY-MM-DD", or null when empty. */
  value: string | null;
  onChange: (value: string | null) => void;
}

/**
 * A date input that maps "" to null. When empty it is marked `data-empty`, so
 * the CSS can dim what WebKit on macOS shows there (today's date) and the field
 * does not look filled in.
 */
export function DateField({ value, onChange, ...rest }: DateFieldProps) {
  return (
    <TextField
      {...rest}
      type="date"
      value={value ?? ""}
      data-empty={value ? undefined : "true"}
      onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
    />
  );
}
