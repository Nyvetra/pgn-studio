// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Review screen (architecture.md §13.5): operation summary, ordered
 * sources, destination artifacts, conflict behavior, estimated input
 * bytes, warnings, engine identity, and the optional advanced command
 * view. The Run button stays disabled until `validate_job` returns Ready.
 */
import { useState } from "react";
import { useWorkflow } from "../../state/useWorkflow";
import { selectCanRun } from "../../state/workflowReducer";
import { buildJobSpec } from "../../state/jobSpecBuilder";
import { ARTIFACT_KIND_LABELS, formatBytes } from "../../state/formatters";
import { Banner } from "../../components/Banner";
import { StepNav } from "../../components/StepNav";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { useFocusOnMount } from "../../components/useFocusOnMount";
import "../../components/workflow-screen.css";
import "./ReviewScreen.css";
import { OperationSummary } from "./OperationSummary";
import { CommandPreview } from "./CommandPreview";
import { useCommandPreview } from "./useCommandPreview";
import { useJobRunContext } from "../execution/useJobRunContext";

const CONFLICT_POLICY_SUMMARY: Record<string, string> = {
  fail: "Stop before running if any output file already exists.",
  addNumericSuffix: "Automatically add a number to the new file's name if it already exists.",
  replaceAfterConfirmation:
    "Replace an existing file, after you confirm — the previous file is renamed to a timestamped .bak copy first.",
};

export function ReviewScreen() {
  const { state, dispatch } = useWorkflow();
  const jobRun = useJobRunContext();
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const preview = useCommandPreview(state);
  const canRun = selectCanRun(state);
  const headingRef = useFocusOnMount<HTMLHeadingElement>();

  function startRun(justConfirmedReplace: boolean) {
    const spec = buildJobSpec(state);
    if (justConfirmedReplace) {
      spec.output.confirmedReplace = true;
      dispatch({ type: "CONFIRM_REPLACE" });
    }
    dispatch({ type: "GO_NEXT" });
    void jobRun.start(spec);
  }

  function handleRunClick() {
    const needsConfirmation =
      state.output.conflictPolicy === "replaceAfterConfirmation" && !state.output.confirmedReplace;
    if (needsConfirmation) {
      setConfirmOpen(true);
      return;
    }
    startRun(false);
  }

  const validation = state.validation;
  const hasIssues = Boolean(
    validation && (validation.errors.length > 0 || validation.warnings.length > 0 || validation.advisories.length > 0),
  );

  return (
    <section className="workflow-screen" aria-labelledby="review-heading">
      <h2 id="review-heading" ref={headingRef} tabIndex={-1}>
        Review
      </h2>
      <p className="workflow-screen__intro">
        Check everything below before running. Nothing happens to your source files at any point —
        every output is a new file.
      </p>

      <section aria-labelledby="review-summary-heading">
        <h3 id="review-summary-heading">What will happen</h3>
        <OperationSummary state={state} />
      </section>

      <section aria-labelledby="review-sources-heading">
        <h3 id="review-sources-heading">Sources, in order</h3>
        <ol className="review-source-list">
          {state.inputs.map((input) => (
            <li key={input.id}>{input.path}</li>
          ))}
        </ol>
      </section>

      <section aria-labelledby="review-destination-heading">
        <h3 id="review-destination-heading">Destination</h3>
        <p>{state.output.directory || "(no folder chosen)"}</p>
        <p>{CONFLICT_POLICY_SUMMARY[state.output.conflictPolicy]}</p>
        <h4>Files that will be created</h4>
        {preview.loading && <p role="status">Working out what will be created…</p>}
        {preview.error && (
          <Banner tone="danger" role="alert">
            {preview.error.title}: {preview.error.message}
          </Banner>
        )}
        {preview.preview && (
          <ul className="review-artifact-list">
            {preview.preview.plannedArtifacts.map((artifact) => (
              <li key={artifact.finalPath}>
                <strong>{ARTIFACT_KIND_LABELS[artifact.kind]}</strong>
                <br />
                {artifact.finalPath}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-labelledby="review-estimate-heading">
        <h3 id="review-estimate-heading">Estimated size</h3>
        {state.validating && <p role="status">Checking your configuration…</p>}
        {validation && (
          <ul className="review-summary-list">
            <li>Estimated input size: {formatBytes(validation.estimatedInputBytes)}</li>
            {validation.freeDiskBytes !== null && (
              <li>Free disk space at destination: {formatBytes(validation.freeDiskBytes)}</li>
            )}
          </ul>
        )}
      </section>

      {hasIssues && validation && (
        <section aria-labelledby="review-warnings-heading">
          <h3 id="review-warnings-heading">Warnings</h3>
          {validation.errors.map((error) => (
            <Banner tone="danger" role="alert" key={error.technicalId}>
              <p>
                <strong>{error.title}:</strong> {error.message}
              </p>
              {error.remediation && <p>{error.remediation}</p>}
            </Banner>
          ))}
          {validation.warnings.map((warning, index) => (
            // Warnings are plain {code, message} with no stable id from the backend.
            <Banner tone="warning" key={`${warning.code}-${index}`}>
              {warning.message}
            </Banner>
          ))}
          {validation.advisories.map((advisory, index) => (
            // Advisories are free-text with no stable id from the backend.
            <Banner tone="info" key={`advisory-${index}`}>
              {advisory}
            </Banner>
          ))}
        </section>
      )}

      <section aria-labelledby="review-engine-heading">
        <h3 id="review-engine-heading">Engine</h3>
        {state.capabilities ? (
          <p>
            pgn-extract {state.capabilities.identity.version} ({state.capabilities.identity.targetTriple})
          </p>
        ) : (
          <p role="status">Loading engine information…</p>
        )}
      </section>

      <CommandPreview
        open={advancedOpen}
        onToggle={setAdvancedOpen}
        loading={preview.loading}
        preview={preview.preview}
        error={preview.error}
      />

      {jobRun.state.startError && (
        <Banner tone="danger" role="alert" title={jobRun.state.startError.title}>
          <p>{jobRun.state.startError.message}</p>
          {jobRun.state.startError.remediation && <p>{jobRun.state.startError.remediation}</p>}
        </Banner>
      )}

      <ConfirmDialog
        open={confirmOpen}
        title="Replace existing files?"
        description={
          <p>
            If any of the files listed above already exist, running this job will replace them. The
            previous file is renamed to a timestamped .bak copy first, so it is never simply deleted
            — but this step cannot be undone from within PGN Studio.
          </p>
        }
        confirmLabel="Replace and run"
        danger
        onConfirm={() => {
          setConfirmOpen(false);
          startRun(true);
        }}
        onCancel={() => setConfirmOpen(false)}
      />

      <StepNav
        onBack={() => dispatch({ type: "GO_TO_STEP", step: "filters" })}
        onNext={handleRunClick}
        nextDisabled={!canRun}
        nextLabel="Run Job"
      />
    </section>
  );
}
