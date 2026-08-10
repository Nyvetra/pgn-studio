// SPDX-License-Identifier: GPL-3.0-or-later
/** Shared Back/Next footer for each workflow screen (architecture.md
 * §13.1: "the user can move backward before execution without losing
 * settings" — Back is always a plain, unconditional navigation action;
 * only Next/Run ever gets disabled). */
import { Button } from "./Button";
import "./StepNav.css";

export interface StepNavProps {
  onBack?: () => void;
  onNext?: () => void;
  nextLabel?: string;
  nextDisabled?: boolean;
  backLabel?: string;
}

export function StepNav({ onBack, onNext, nextLabel = "Next", nextDisabled, backLabel = "Back" }: StepNavProps) {
  return (
    <div className="step-nav">
      <div>{onBack && <Button variant="secondary" onClick={onBack}>{backLabel}</Button>}</div>
      <div>
        {onNext && (
          <Button variant="primary" onClick={onNext} disabled={nextDisabled}>
            {nextLabel}
          </Button>
        )}
      </div>
    </div>
  );
}
