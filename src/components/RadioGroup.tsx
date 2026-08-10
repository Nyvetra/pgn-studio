// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * An accessible radio group: `<fieldset>`/`<legend>` plus native
 * `<input type="radio">` options, so arrow-key navigation between options
 * and screen-reader group announcement both come from the browser for free
 * (architecture.md §13.8).
 */
import { useId } from "react";
import "./fields.css";

export interface RadioOption<T extends string> {
  value: T;
  label: string;
  help?: string;
  disabled?: boolean;
}

export interface RadioGroupProps<T extends string> {
  legend: string;
  legendHidden?: boolean;
  name?: string;
  options: readonly RadioOption<T>[];
  value: T;
  onValueChange: (value: T) => void;
  disabled?: boolean;
}

export function RadioGroup<T extends string>({
  legend,
  legendHidden,
  name,
  options,
  value,
  onValueChange,
  disabled,
}: RadioGroupProps<T>) {
  const autoName = useId();
  const groupName = name ?? autoName;

  return (
    <fieldset className="field-group">
      <legend className={legendHidden ? "visually-hidden" : undefined}>{legend}</legend>
      {options.map((option) => {
        const optionId = `${groupName}-${option.value}`;
        const helpId = option.help ? `${optionId}-help` : undefined;
        const isDisabled = disabled || option.disabled;
        return (
          <div
            className={["radio-option", isDisabled ? "radio-option--disabled" : ""]
              .filter(Boolean)
              .join(" ")}
            key={option.value}
          >
            <input
              type="radio"
              className="radio-option__control"
              id={optionId}
              name={groupName}
              value={option.value}
              checked={value === option.value}
              disabled={isDisabled}
              onChange={() => onValueChange(option.value)}
              aria-describedby={helpId}
            />
            <span className="radio-option__text">
              <label className="radio-option__label" htmlFor={optionId}>
                {option.label}
              </label>
              {option.help && (
                <p className="radio-option__help" id={helpId}>
                  {option.help}
                </p>
              )}
            </span>
          </div>
        );
      })}
    </fieldset>
  );
}
