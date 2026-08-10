// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Step 5 of the workflow (architecture.md §13.1 names it one step, "Run &
 * Results", even though §13.6/§13.7 describe it as two screens): renders
 * the Run screen while the job is active and the Results screen once it
 * reaches a terminal state.
 */
import { selectIsTerminal } from "../../state/jobRunReducer";
import { RunScreen } from "./RunScreen";
import { useJobRunContext } from "./useJobRunContext";
import { ResultsScreen } from "../results/ResultsScreen";

export function RunResultsStep() {
  const jobRun = useJobRunContext();
  return selectIsTerminal(jobRun.state) ? <ResultsScreen /> : <RunScreen />;
}
