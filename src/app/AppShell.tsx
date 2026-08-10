// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Top-level layout: the step indicator (architecture.md §13.1) plus
 * whichever of the five screens matches the current step. Wires the two
 * always-on IPC effects (engine capabilities, kept-warm validation) that
 * every screen depends on.
 */
import { useWorkflow } from "../state/useWorkflow";
import { useCapabilitiesEffect, useValidationEffect } from "../state/effects";
import { Stepper } from "../components/Stepper";
import { FilesScreen } from "../features/inputs/FilesScreen";
import { OperationsScreen } from "../features/operations/OperationsScreen";
import { FiltersScreen } from "../features/filters/FiltersScreen";
import { ReviewScreen } from "../features/review/ReviewScreen";
import { RunResultsStep } from "../features/execution/RunResultsStep";
import "./AppShell.css";

export function AppShell() {
  const { state, dispatch } = useWorkflow();
  useCapabilitiesEffect(dispatch);
  useValidationEffect(state, dispatch);

  return (
    <main className="app-shell">
      <h1 className="app-shell__title">PGN Studio</h1>
      <Stepper
        current={state.step}
        farthestStepIndex={state.farthestStepIndex}
        onSelect={(step) => dispatch({ type: "GO_TO_STEP", step })}
      />
      <div className="app-shell__content">
        {state.step === "files" && <FilesScreen />}
        {state.step === "operations" && <OperationsScreen />}
        {state.step === "filters" && <FiltersScreen />}
        {state.step === "review" && <ReviewScreen />}
        {state.step === "run-results" && <RunResultsStep />}
      </div>
    </main>
  );
}
