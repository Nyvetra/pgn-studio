// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Files screen (architecture.md §13.2): drop zone, Add Files/Add Folder,
 * ordered source list with reorder controls, output folder + base
 * filename, and — whenever deduplication is enabled — an explanation that
 * earlier files win duplicate retention.
 */
import { useWorkflow } from "../../state/useWorkflow";
import { selectFilesStepReady } from "../../state/workflowReducer";
import { DUPLICATE_POLICY_LABELS } from "../../types/workflow";
import { Banner } from "../../components/Banner";
import { StepNav } from "../../components/StepNav";
import "../../components/workflow-screen.css";
import { DropZone } from "./DropZone";
import { SourceList } from "./SourceList";
import { OutputFields } from "./OutputFields";
import { useInputInspectionEffect } from "./useInputInspection";

export function FilesScreen() {
  const { state, dispatch } = useWorkflow();
  useInputInspectionEffect(state.inputs, dispatch);

  const deduplicationEnabled = state.operations.duplicates !== "none";
  const ready = selectFilesStepReady(state);

  return (
    <section className="workflow-screen" aria-labelledby="files-heading">
      <h2 id="files-heading">Files</h2>
      <p className="workflow-screen__intro">
        Add the PGN files you want to process, put them in the order you want, and choose where the
        new files should be written. Your original files are never modified.
      </p>

      {deduplicationEnabled && (
        <Banner tone="info" title="Order matters while duplicate handling is on">
          <p>
            When the same game appears more than once, the <strong>first copy in this list</strong> is
            the one that is kept — later copies are treated as duplicates, based on the moves played
            (not the headers, comments, or variations). If a later copy actually has better metadata
            or annotations, move it higher in the list, or you may lose that information.
          </p>
          <p className="workflow-screen__section-help">
            Current duplicate setting: {DUPLICATE_POLICY_LABELS[state.operations.duplicates]}.
          </p>
        </Banner>
      )}

      <section aria-labelledby="files-sources-heading">
        <h3 id="files-sources-heading">Source files</h3>
        <DropZone onFilesChosen={(paths) => dispatch({ type: "ADD_INPUTS", paths })} />
        <SourceList
          inputs={state.inputs}
          onMove={(id, direction) => dispatch({ type: "MOVE_INPUT", id, direction })}
          onRemove={(id) => dispatch({ type: "REMOVE_INPUT", id })}
        />
      </section>

      <section aria-labelledby="files-destination-heading">
        <h3 id="files-destination-heading">Destination</h3>
        <OutputFields
          directory={state.output.directory}
          baseName={state.output.baseName}
          conflictPolicy={state.output.conflictPolicy}
          onDirectoryChange={(directory) => dispatch({ type: "SET_OUTPUT_DIRECTORY", directory })}
          onBaseNameChange={(baseName) => dispatch({ type: "SET_BASE_NAME", baseName })}
          onConflictPolicyChange={(policy) => dispatch({ type: "SET_CONFLICT_POLICY", policy })}
        />
      </section>

      <StepNav
        onNext={() => dispatch({ type: "GO_NEXT" })}
        nextDisabled={!ready}
        nextLabel="Next: Operations"
      />
    </section>
  );
}
