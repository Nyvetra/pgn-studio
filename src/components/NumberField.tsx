// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Labeled numeric input. Keeps its value as a plain string (never `number`)
 * so the field can be legitimately empty ("no bound") without collapsing to
 * `0` — the same "never substitute 0 for unknown" discipline architecture.md
 * §9.3/§25 requires for displayed metrics also applies to *input* of an
 * optional bound here (an empty Elo/move-count field means "no limit", not
 * "limit of zero").
 */
import { useId, type InputHTMLAttributes } from "react";
import "./fields.css";

export interface NumberFieldProps
  extends Omit<InputHTMLAttributes<HTMLInputElement>, "id" | "onChange" | "type" | "value"> {
  label: string;
  help?: string;
  error?: string;
  value: string;
  onValueChange: (value: string) => void;
}

export function NumberField({
  label,
  help,
  error,
  onValueChange,
  required,
  className,
  value,
  ...rest
}: NumberFieldProps) {
  const id = useId();
  const helpId = help ? `${id}-help` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy = [helpId, errorId].filter(Boolean).join(" ") || undefined;

  return (
    <div className={["field", className].filter(Boolean).join(" ")}>
      <label className="field__label" htmlFor={id}>
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
        type="number"
        inputMode="numeric"
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
