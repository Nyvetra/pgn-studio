// SPDX-License-Identifier: GPL-3.0-or-later
/** Labeled native `<select>` — kept native rather than a custom listbox so
 * keyboard operation (arrow keys, type-ahead) and screen-reader support are
 * both free (architecture.md §13.8). */
import { useId } from "react";
import "./fields.css";

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface SelectFieldProps<T extends string> {
  label: string;
  help?: string;
  value: T;
  onValueChange: (value: T) => void;
  options: readonly SelectOption<T>[];
  disabled?: boolean;
  className?: string;
}

export function SelectField<T extends string>({
  label,
  help,
  value,
  onValueChange,
  options,
  disabled,
  className,
}: SelectFieldProps<T>) {
  const id = useId();
  const helpId = help ? `${id}-help` : undefined;

  return (
    <div className={["field", className].filter(Boolean).join(" ")}>
      <label className="field__label" htmlFor={id}>
        {label}
      </label>
      <select
        id={id}
        className="field__control"
        value={value}
        disabled={disabled}
        onChange={(e) => onValueChange(e.target.value as T)}
        aria-describedby={helpId}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value} disabled={option.disabled}>
            {option.label}
          </option>
        ))}
      </select>
      {help && (
        <p className="field__help" id={helpId}>
          {help}
        </p>
      )}
    </div>
  );
}
