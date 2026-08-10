// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The five-step workflow navigator (architecture.md §13.1). Semantic
 * `<nav><ol>` of real `<button>`s (§13.8) — each reachable step is a
 * focusable, activatable control; steps beyond the farthest one reached are
 * rendered as disabled buttons rather than removed, so their existence and
 * order are still discoverable. The current step carries `aria-current`
 * so assistive technology can announce "current step" without relying on
 * visual styling alone.
 */
import { STEP_LABELS, WORKFLOW_STEPS, type WorkflowStep } from "../types/workflow";
import "./Stepper.css";

export interface StepperProps {
  current: WorkflowStep;
  farthestStepIndex: number;
  onSelect: (step: WorkflowStep) => void;
}

export function Stepper({ current, farthestStepIndex, onSelect }: StepperProps) {
  return (
    <nav aria-label="Workflow progress" className="stepper">
      <ol className="stepper__list">
        {WORKFLOW_STEPS.map((step, index) => {
          const isCurrent = step === current;
          const isReachable = index <= farthestStepIndex;
          return (
            <li key={step} className="stepper__item">
              <button
                type="button"
                className={[
                  "stepper__step",
                  isCurrent ? "stepper__step--current" : "",
                  isReachable ? "stepper__step--reachable" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                aria-current={isCurrent ? "step" : undefined}
                disabled={!isReachable}
                onClick={() => onSelect(step)}
              >
                <span className="stepper__index" aria-hidden="true">
                  {index + 1}
                </span>
                <span className="stepper__label">{STEP_LABELS[step]}</span>
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
