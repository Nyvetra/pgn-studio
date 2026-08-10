// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Indeterminate progress indicator (architecture.md §4.7, §13.6): "if the
 * upstream process cannot provide a reliable percentage, show an
 * indeterminate progress indicator, elapsed time, current stage, and log
 * activity. Never fabricate a percentage." This component only ever renders
 * the indeterminate form — nothing in this codebase computes or passes a
 * percentage into it, because `pgn-extract`'s own progress ticks
 * (`ProcessingMetrics.processedGames`) have no known denominator.
 *
 * `aria-valuenow` is deliberately omitted: per the ARIA `progressbar` role,
 * that omission is what tells assistive technology the value is
 * indeterminate, rather than stuck at a real number.
 */
import "./ProgressIndicator.css";

export interface ProgressIndicatorProps {
  label: string;
}

export function ProgressIndicator({ label }: ProgressIndicatorProps) {
  return (
    <div
      className="progress-indicator"
      role="progressbar"
      aria-label={label}
      aria-valuetext="Working — progress is not measurable for this step"
    >
      <div className="progress-indicator__track">
        <div className="progress-indicator__bar" />
      </div>
    </div>
  );
}
