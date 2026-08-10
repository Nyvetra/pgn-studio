// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * IPC-driven side effects for the workflow draft: fetching engine
 * capabilities once, and re-running `validate_job` (debounced) whenever the
 * draft changes (architecture.md §13.5: "The Run button remains disabled
 * until backend validation returns Ready" — validation is kept warm as the
 * user edits earlier screens, not deferred until they reach Review).
 */
import { useEffect, useRef, type Dispatch } from "react";
import { getEngineCapabilities, validateJob } from "../ipc/client";
import { buildJobSpec } from "./jobSpecBuilder";
import type { WorkflowAction, WorkflowState } from "./workflowReducer";

const VALIDATION_DEBOUNCE_MS = 400;

export function useCapabilitiesEffect(dispatch: Dispatch<WorkflowAction>): void {
  useEffect(() => {
    let cancelled = false;
    getEngineCapabilities()
      .then((result) => {
        if (cancelled || result.status !== "ok") return;
        dispatch({ type: "SET_CAPABILITIES", capabilities: result.data });
      })
      .catch(() => {
        // A raw (non-PublicError) transport failure here leaves
        // `capabilities` at `null`, which every capability-gated control
        // already treats as "unknown, so disabled" (`capabilityHelp.ts`) —
        // the safe, conservative outcome, not a crash or a silent promise
        // rejection.
      });
    return () => {
      cancelled = true;
    };
  }, [dispatch]);
}

export function useValidationEffect(state: WorkflowState, dispatch: Dispatch<WorkflowAction>): void {
  const stateRef = useRef(state);
  // Refs must not be written during render (react-hooks/refs) — keep this
  // in its own effect with no dependency array, so it runs after every
  // commit and stays one render behind at most, which is irrelevant here
  // since the debounce effect below only ever reads it 400ms later.
  useEffect(() => {
    stateRef.current = state;
  });

  useEffect(() => {
    if (stateRef.current.inputs.length === 0) return;
    const revision = stateRef.current.specRevision;
    let cancelled = false;
    dispatch({ type: "SET_VALIDATING" });
    const timer = window.setTimeout(() => {
      const spec = buildJobSpec(stateRef.current);
      validateJob(spec)
        .then((result) => {
          if (cancelled) return;
          if (result.status === "ok") {
            dispatch({ type: "SET_VALIDATION_RESULT", report: result.data, specRevision: revision });
          } else {
            // A transport-level failure (e.g. the engine bundle itself
            // could not be resolved) still deserves a visible, actionable
            // report rather than a silently-stuck spinner.
            dispatch({
              type: "SET_VALIDATION_RESULT",
              specRevision: revision,
              report: {
                status: "invalid",
                errors: [result.error],
                warnings: [],
                advisories: [],
                estimatedInputBytes: 0,
                freeDiskBytes: null,
              },
            });
          }
        })
        .catch(() => {
          if (cancelled) return;
          // A raw (non-PublicError) failure: stop the spinner rather than
          // leaving `validating` stuck true forever, but do not fabricate a
          // validation report for it — `validation` simply stays whatever
          // it last was.
          dispatch({ type: "SET_VALIDATING_FAILED", specRevision: revision });
        });
    }, VALIDATION_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
    // Intentionally scoped to `specRevision` (see doc comment above): every
    // other field is read through `stateRef.current` at call time instead of
    // being a dependency, so this effect only re-runs when the draft
    // actually changes shape, never merely because `SET_VALIDATION_RESULT`
    // itself produced a new `state` reference (which would be an infinite
    // validate loop).
  }, [state.specRevision, dispatch]);
}
