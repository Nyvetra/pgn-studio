// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * React wiring for `workflowReducer`. Deliberately thin: this component
 * only creates the reducer and provides it — IPC side effects (capability
 * fetch, debounced validation) live in `state/effects.ts`, and the
 * `useWorkflow()` accessor hook lives in `state/useWorkflow.ts`, so this
 * file exports only the `WorkflowProvider` component.
 */
import { useReducer, type ReactNode } from "react";
import { createInitialWorkflowState, workflowReducer } from "./workflowReducer";
import { WorkflowContext } from "./workflowContextInstance";

export function WorkflowProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(workflowReducer, undefined, createInitialWorkflowState);
  return (
    <WorkflowContext.Provider value={{ state, dispatch }}>{children}</WorkflowContext.Provider>
  );
}
