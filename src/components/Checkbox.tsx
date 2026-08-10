// SPDX-License-Identifier: GPL-3.0-or-later
/** Labeled checkbox with optional short plain-language help text
 * (architecture.md §13.3: "options should include short plain-language
 * explanations"). */
import { useId } from "react";
import "./fields.css";

export interface CheckboxProps {
  label: string;
  help?: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
}

export function Checkbox({ label, help, checked, onCheckedChange, disabled, className }: CheckboxProps) {
  const id = useId();
  const helpId = help ? `${id}-help` : undefined;

  return (
    <div
      className={["check-field", disabled ? "check-field--disabled" : "", className]
        .filter(Boolean)
        .join(" ")}
    >
      <input
        id={id}
        type="checkbox"
        className="check-field__control"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onCheckedChange(e.target.checked)}
        aria-describedby={helpId}
      />
      <span className="check-field__text">
        <label className="check-field__label" htmlFor={id}>
          {label}
        </label>
        {help && (
          <p className="check-field__help" id={helpId}>
            {help}
          </p>
        )}
      </span>
    </div>
  );
}
