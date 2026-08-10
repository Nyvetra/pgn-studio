// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Renders one labeled metric value. The single place in the UI that decides
 * how an unmeasured `Option<u64>` metric is displayed — always the literal
 * text "Not available", never a bare `0` (architecture.md §9.3, §13.7,
 * §25). Every screen that shows `ProcessingMetrics` fields must go through
 * this component rather than interpolating a number directly.
 */
import type { ReactNode } from "react";
import "./Metric.css";

export interface MetricProps {
  label: string;
  /** The already-formatted display string (see `state/formatters.ts`) —
   * this component does not format numbers itself, it only lays the pair
   * out and gives the "unknown" state its own presentation. */
  value: string;
  hint?: ReactNode;
}

export function Metric({ label, value, hint }: MetricProps) {
  const unknown = value === "Not available";
  return (
    <div className="metric">
      <dt className="metric__label">{label}</dt>
      <dd className={["metric__value", unknown ? "metric__value--unknown" : ""].filter(Boolean).join(" ")}>
        {value}
      </dd>
      {hint && <p className="metric__hint">{hint}</p>}
    </div>
  );
}

export function MetricList({ children }: { children: ReactNode }) {
  return <dl className="metric-list">{children}</dl>;
}
