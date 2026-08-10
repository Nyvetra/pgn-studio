// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Preset selector (architecture.md §13.3, §12.1). Applying a preset writes
 * a complete, inspectable `JobSpec` diff into the draft — every field it
 * touches remains a normal, editable control below, never a hidden
 * command string.
 */
import { PRESETS } from "../../state/presets";
import type { PresetId } from "../../types/workflow";
import "./PresetPicker.css";

export interface PresetPickerProps {
  active: PresetId;
  onApply: (id: Exclude<PresetId, "custom">) => void;
}

export function PresetPicker({ active, onApply }: PresetPickerProps) {
  return (
    <div role="group" aria-label="Presets" className="preset-picker">
      {PRESETS.map((preset) => {
        const isActive = active === preset.id;
        return (
          <button
            key={preset.id}
            type="button"
            className={["preset-card", isActive ? "preset-card--active" : ""].filter(Boolean).join(" ")}
            aria-pressed={isActive}
            onClick={() => onApply(preset.id)}
          >
            <span className="preset-card__label">{preset.label}</span>
            <span className="preset-card__description">{preset.description}</span>
          </button>
        );
      })}
      {active === "custom" && (
        <p className="preset-picker__custom-note" role="status">
          Current configuration: <strong>Custom</strong> — it no longer matches a built-in preset because
          something below was changed by hand. That&rsquo;s fine; pick a preset above at any time to
          start over from a known baseline.
        </p>
      )}
    </div>
  );
}
