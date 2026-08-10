// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The application's screen/route map (architecture.md §13.1): the
 * five-step MVP workflow, `Files -> Operations -> Filters -> Review -> Run
 * & Results`.
 *
 * Re-exported from `src/types/workflow.ts` rather than duplicated here —
 * that module is also where `WorkflowState`/`workflowReducer`
 * (`src/state/workflowReducer.ts`) get their step type from, and a route
 * list that could drift from the one actually driving navigation would be
 * worse than no route list at all.
 */
export { WORKFLOW_STEPS as APP_ROUTES, STEP_LABELS, type WorkflowStep as AppRoute } from "../types/workflow";
