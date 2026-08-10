// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The raw context object, split out from `WorkflowContext.tsx` so that file
 * can export only the `WorkflowProvider` component (react-refresh's
 * `only-export-components` rule wants one file per component when Fast
 * Refresh needs to work) while `useWorkflow.ts` exports only the hook.
 */
import { createContext, type Dispatch } from "react";
import type { WorkflowAction, WorkflowState } from "./workflowReducer";

export interface WorkflowContextValue {
  state: WorkflowState;
  dispatch: Dispatch<WorkflowAction>;
}

export const WorkflowContext = createContext<WorkflowContextValue | null>(null);
