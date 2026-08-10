// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Calls `inspect_inputs` for any source rows that have not been inspected
 * yet (freshly added files), and folds the results back into the draft
 * (architecture.md §13.2: file sizes and warnings on the source list).
 */
import { useEffect } from "react";
import type { Dispatch } from "react";
import { inspectInputs } from "../../ipc/client";
import type { WorkflowAction } from "../../state/workflowReducer";
import type { DraftInput } from "../../types/workflow";

export function useInputInspectionEffect(inputs: DraftInput[], dispatch: Dispatch<WorkflowAction>): void {
  useEffect(() => {
    const pending = inputs.filter((input) => !input.inspected).map((input) => input.path);
    if (pending.length === 0) return;

    let cancelled = false;
    void inspectInputs(pending).then((result) => {
      if (cancelled || result.status !== "ok") return;
      dispatch({ type: "APPLY_INSPECTIONS", inspections: result.data });
    });
    return () => {
      cancelled = true;
    };
  }, [inputs, dispatch]);
}
