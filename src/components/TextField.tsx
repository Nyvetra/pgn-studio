// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Labeled text input with help/error text wired through `aria-describedby`
 * (architecture.md §13.8: semantic labels, and §20.1's "form validation
 * presentation" test target).
 */
import { useId, type InputHTMLAttributes } from "react";
import "./fields.css";

export interface TextFieldProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "id" | "onChange"> {
  label: string;
  /** Keeps the label in the accessibility tree (still readable by a screen
   * reader, still the input's accessible name) but hides it visually — for
   * repeated rows (e.g. one ECO-code entry per list item) where a full
   * visible label per row would be redundant noise. */
  labelHidden?: boolean;
  help?: string;
  error?: string;
  onValueChange: (value: string) => void;
}

export function TextField({
  label,
  labelHidden,
  help,
  error,
  onValueChange,
  required,
  className,
  value,
  ...rest
}: TextFieldProps) {
  const id = useId();
  const helpId = help ? `${id}-help` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy = [helpId, errorId].filter(Boolean).join(" ") || undefined;

  return (
    <div className={["field", className].filter(Boolean).join(" ")}>
      <label className={labelHidden ? "visually-hidden" : "field__label"} htmlFor={id}>
        {label}
        {required && (
          <>
            <span className="field__required" aria-hidden="true">
              {" "}
              *
            </span>
            <span className="visually-hidden"> required</span>
          </>
        )}
      </label>
      <input
        id={id}
        className="field__control"
        value={value}
        onChange={(e) => onValueChange(e.target.value)}
        aria-describedby={describedBy}
        aria-invalid={error ? true : undefined}
        aria-required={required || undefined}
        {...rest}
      />
      {help && (
        <p className="field__help" id={helpId}>
          {help}
        </p>
      )}
      {error && (
        <p className="field__error" id={errorId} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
