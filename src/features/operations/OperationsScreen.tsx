// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Operations screen (architecture.md §13.3): preset, merge & validation
 * mode, duplicate policy, cleanup, ECO classification, output notation, and
 * audit artifacts.
 */
import { useWorkflow } from "../../state/useWorkflow";
import { selectActivePreset } from "../../state/workflowReducer";
import { StepNav } from "../../components/StepNav";
import "../../components/workflow-screen.css";
import "./OperationsScreen.css";
import { PresetPicker } from "./PresetPicker";
import { ModeAndValidationSection } from "./ModeAndValidationSection";
import { DuplicateSection } from "./DuplicateSection";
import { CleanupSection } from "./CleanupSection";
import { EcoOutputSection } from "./EcoOutputSection";
import { ArtifactsSection } from "./ArtifactsSection";

export function OperationsScreen() {
  const { state, dispatch } = useWorkflow();
  const activePreset = selectActivePreset(state);

  return (
    <section className="workflow-screen" aria-labelledby="operations-heading">
      <h2 id="operations-heading">Operations</h2>
      <p className="workflow-screen__intro">
        Choose a preset to start from a sensible baseline, then adjust anything below — every option
        is a real, editable setting, not a hidden command.
      </p>

      <section aria-labelledby="operations-preset-heading">
        <h3 id="operations-preset-heading">Preset</h3>
        <PresetPicker
          active={activePreset}
          onApply={(presetId) => dispatch({ type: "APPLY_PRESET", presetId })}
        />
      </section>

      <ModeAndValidationSection
        mode={state.operations.mode}
        broken={state.operations.broken}
        cleanup={state.operations.cleanup}
        capabilities={state.capabilities}
        onModeChange={(mode) => dispatch({ type: "SET_MODE", mode })}
        onBrokenChange={(value) => dispatch({ type: "SET_BROKEN_OUTPUT", value })}
        onCleanupChange={(patch) => dispatch({ type: "SET_CLEANUP", patch })}
      />

      <DuplicateSection
        mode={state.operations.mode}
        duplicates={state.operations.duplicates}
        checkFile={state.operations.checkFile}
        artifacts={state.artifacts}
        runtime={state.runtime}
        capabilities={state.capabilities}
        onDuplicatesChange={(policy) => dispatch({ type: "SET_DUPLICATE_POLICY", policy })}
        onCheckFileChange={(path) => dispatch({ type: "SET_CHECK_FILE", path })}
        onArtifactsChange={(patch) => dispatch({ type: "SET_ARTIFACTS", patch })}
        onRuntimeChange={(patch) => dispatch({ type: "SET_RUNTIME", patch })}
      />

      <CleanupSection
        cleanup={state.operations.cleanup}
        onChange={(patch) => dispatch({ type: "SET_CLEANUP", patch })}
      />

      <EcoOutputSection
        ecoEnabled={state.operations.eco.enabled}
        outputNotation={state.operations.outputNotation}
        capabilities={state.capabilities}
        onEcoChange={(enabled) => dispatch({ type: "SET_ECO_ENABLED", enabled })}
        onNotationChange={(notation) => dispatch({ type: "SET_OUTPUT_NOTATION", notation })}
      />

      <ArtifactsSection
        artifacts={state.artifacts}
        onChange={(patch) => dispatch({ type: "SET_ARTIFACTS", patch })}
      />

      <StepNav
        onBack={() => dispatch({ type: "GO_TO_STEP", step: "files" })}
        onNext={() => dispatch({ type: "GO_NEXT" })}
        nextLabel="Next: Filters"
      />
    </section>
  );
}
