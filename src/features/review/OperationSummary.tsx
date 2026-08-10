// SPDX-License-Identifier: GPL-3.0-or-later
/** Plain-language operation summary (architecture.md §13.5). */
import type { WorkflowState } from "../../state/workflowReducer";
import { selectActivePreset } from "../../state/workflowReducer";
import { DUPLICATE_POLICY_LABELS } from "../../types/workflow";
import { compileFilters } from "../../state/filterMapping";
import { getPreset } from "../../state/presets";

export function OperationSummary({ state }: { state: WorkflowState }) {
  const { operations, uniqueGames, artifacts } = state;
  const activePresetId = selectActivePreset(state);
  const activePreset = activePresetId === "custom" ? null : getPreset(activePresetId);
  const filterPlan = compileFilters(state.filters);
  const activeFilterCount =
    filterPlan.tagRules.length +
    (filterPlan.moveBounds ? 1 : 0) +
    (filterPlan.checkmateOnly ? 1 : 0) +
    (filterPlan.setupPolicy !== "any" ? 1 : 0);

  const cleanupItems = [
    operations.cleanup.removeComments && "comments",
    operations.cleanup.removeVariations && "variations",
    operations.cleanup.removeNags && "NAGs",
    operations.cleanup.removeMoveNumbers && "move numbers",
    operations.cleanup.removeResults && "movetext result markers",
    operations.cleanup.removeTags.length > 0 &&
      `the ${operations.cleanup.removeTags.join(", ")} tag${operations.cleanup.removeTags.length === 1 ? "" : "s"}`,
  ].filter(Boolean) as string[];

  return (
    <ul className="review-summary-list">
      <li>
        {activePreset
          ? `Started from preset: ${activePreset.label} (version ${activePreset.version}).`
          : "Custom configuration — not an unmodified built-in preset."}
      </li>
      <li>
        {operations.mode === "validateOnly"
          ? "Validate every source file for errors; do not write a merged games file."
          : "Merge every source file and write output games."}
      </li>
      <li>Duplicate handling: {DUPLICATE_POLICY_LABELS[operations.duplicates]}.</li>
      {operations.checkFile && <li>Checked against master file: {operations.checkFile}.</li>}
      <li>
        Games with errors:{" "}
        {operations.broken === "keepInMainOutput"
          ? "kept in the main output"
          : "discarded (reported in the log only)"}
        .
      </li>
      <li>
        Inconsistent result tags:{" "}
        {[
          operations.cleanup.rejectBadResults && "games with a mismatched result are excluded",
          operations.cleanup.fixResultTags && "resolvable ones are corrected automatically",
        ]
          .filter(Boolean)
          .join("; ") || "left as found"}
        .
      </li>
      <li>
        {cleanupItems.length > 0
          ? `Removes: ${cleanupItems.join(", ")}.`
          : "No comments, variations, or NAGs are removed."}
      </li>
      <li>ECO classification: {operations.eco.enabled ? "added" : "not added"}.</li>
      <li>Output notation: {operations.outputNotation === "san" ? "Standard Algebraic Notation" : operations.outputNotation}.</li>
      <li>{activeFilterCount === 0 ? "No filters applied — every valid game is included." : `${activeFilterCount} filter${activeFilterCount === 1 ? "" : "s"} applied.`}</li>
      <li>Main output file: {uniqueGames ? "written" : "not written"}.</li>
      <li>Duplicate audit file: {artifacts.duplicateGames === "audit" ? "saved" : "not saved"}.</li>
      <li>Log file: {artifacts.logFile ? "saved" : "not saved"}.</li>
      <li>Processing report: {artifacts.manifest ? "saved" : "not saved"}.</li>
    </ul>
  );
}
