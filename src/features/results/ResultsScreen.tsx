// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Results screen (architecture.md §13.7): status, elapsed time, metrics
 * (unknown values as "Not available", never 0), artifact list, View Log,
 * Save Job, Rerun Job, Start New Job.
 *
 * "Save Job" exports the completed job's full reproducible manifest
 * (architecture.md §15.3's checklist - schema version, app/engine
 * identity, inputs, options, argv, timestamps, artifacts) to a
 * user-chosen file via the real `export_job_manifest` command and the
 * native save dialog. This is distinct from the automatic local job
 * history every completed run already gets (`application::jobs::
 * record_history`, unconditional) - that history is bounded and local;
 * "Save Job" is an explicit, user-chosen, portable copy.
 */
import { useEffect, useState } from "react";
import { useWorkflow } from "../../state/useWorkflow";
import type { JobRunState } from "../../state/jobRunReducer";
import { exportJobManifest } from "../../ipc/client";
import { formatBytes, formatCount, formatDuration } from "../../state/formatters";
import { Banner, type BannerTone } from "../../components/Banner";
import { Button } from "../../components/Button";
import { Metric, MetricList } from "../../components/Metric";
import { useAnnounce } from "../../components/useAnnounce";
import { useFocusOnMount } from "../../components/useFocusOnMount";
import "../../components/workflow-screen.css";
import "./ResultsScreen.css";
import { ArtifactList } from "./ArtifactList";
import { LiveLog } from "../execution/LiveLog";
import { useJobRunContext } from "../execution/useJobRunContext";

type SaveJobState =
  | { status: "idle" }
  | { status: "saving" }
  | { status: "saved"; path: string }
  | { status: "error"; message: string };

const STATUS_INFO: Record<"succeeded" | "failed" | "cancelled", { label: string; tone: BannerTone }> = {
  succeeded: { label: "Job succeeded", tone: "success" },
  failed: { label: "Job failed", tone: "danger" },
  cancelled: { label: "Job cancelled", tone: "warning" },
};

function resolveStatusInfo(status: JobRunState["status"]) {
  if (status === "succeeded" || status === "failed" || status === "cancelled") {
    return STATUS_INFO[status];
  }
  // Defensive only: RunResultsStep gates this screen to terminal states.
  return { label: "Job finished", tone: "info" as BannerTone };
}

export function ResultsScreen() {
  const { dispatch } = useWorkflow();
  const jobRun = useJobRunContext();
  const [logVisible, setLogVisible] = useState(false);
  const [saveJob, setSaveJob] = useState<SaveJobState>({ status: "idle" });
  const headingRef = useFocusOnMount<HTMLHeadingElement>();
  const announce = useAnnounce();

  const { result } = jobRun.state;
  const artifacts = result?.artifacts ?? jobRun.state.artifacts;
  const metrics = result?.metrics ?? jobRun.state.metrics;
  // Not `selectElapsedMs` (which also knows how to *live-tick* while a job
  // is still running): this screen is only ever shown once terminal
  // (`RunResultsStep`), at which point `elapsedMs` is already frozen to the
  // real duration — reading it directly avoids calling the impure
  // `Date.now()` during render for a "live" branch that can never apply here.
  const elapsedMs = jobRun.state.elapsedMs;
  const error = result?.error ?? null;
  const warnings = result?.warnings ?? [];
  const statusInfo = resolveStatusInfo(jobRun.state.status);

  // architecture.md §13.8: "screen-reader announcements for stage/status
  // changes" — the one status change `RunScreen`'s own
  // `useStageAnnouncements` cannot make (its component has already
  // unmounted by the time a terminal status is reached; see
  // `RunResultsStep`, which swaps `RunScreen` for this component the
  // instant `selectIsTerminal` becomes true). This screen mounts exactly
  // once per job, so a mount-only announcement through the same shared,
  // already-in-the-DOM live region `useAnnounce` uses is the reliable
  // equivalent — unlike relying on this screen's own fresh `role="status"`/
  // `role="alert"` banner to be picked up, which is inconsistent across
  // screen readers for a subtree that is inserted all at once rather than
  // mutated in place.
  useEffect(() => {
    announce(`${statusInfo.label}. Elapsed time: ${formatDuration(elapsedMs)}.`);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function handleRerun() {
    dispatch({ type: "REGENERATE_JOB_ID" });
    dispatch({ type: "GO_TO_STEP", step: "review" });
  }

  function handleStartNew() {
    jobRun.reset();
    dispatch({ type: "RESET_FOR_NEW_JOB" });
  }

  async function handleSaveJob() {
    const jobId = jobRun.state.jobId;
    if (!jobId) return;
    setSaveJob({ status: "saving" });
    const result = await exportJobManifest(jobId);
    if (result.status === "error") {
      setSaveJob({ status: "error", message: result.error.message });
      return;
    }
    if (result.data === null) {
      // User cancelled the save dialog - back to idle, no error to show.
      setSaveJob({ status: "idle" });
      return;
    }
    setSaveJob({ status: "saved", path: result.data });
  }

  return (
    <section className="workflow-screen" aria-labelledby="results-heading">
      <h2 id="results-heading" ref={headingRef} tabIndex={-1}>
        Results
      </h2>

      <Banner tone={statusInfo.tone} role={statusInfo.tone === "danger" ? "alert" : "status"} title={statusInfo.label}>
        <p>Elapsed time: {formatDuration(elapsedMs)}</p>
      </Banner>

      {error && (
        <Banner tone="danger" role="alert" title={error.title}>
          <p>{error.message}</p>
          {error.remediation && <p>{error.remediation}</p>}
          {error.logPath && <p>Log: {error.logPath}</p>}
        </Banner>
      )}

      {warnings.length > 0 && (
        <section aria-labelledby="results-run-warnings-heading">
          <h3 id="results-run-warnings-heading">Warnings</h3>
          {warnings.map((warning, index) => (
            <Banner tone="warning" key={`${warning.code}-${index}`}>
              {warning.message}
            </Banner>
          ))}
        </section>
      )}

      <section aria-labelledby="results-metrics-heading">
        <h3 id="results-metrics-heading">Metrics</h3>
        <MetricList>
          <Metric label="Input files" value={formatCount(metrics?.inputFiles)} />
          <Metric label="Input size" value={formatBytes(metrics?.inputBytes)} />
          <Metric label="Input games" value={formatCount(metrics?.inputGames)} />
          <Metric label="Output games" value={formatCount(metrics?.outputGames)} />
          <Metric label="Duplicate games" value={formatCount(metrics?.duplicateGames)} />
          <Metric label="Broken games" value={formatCount(metrics?.brokenGames)} />
          <Metric label="Output size" value={formatBytes(metrics?.outputBytes)} />
        </MetricList>
      </section>

      <section aria-labelledby="results-artifacts-heading">
        <h3 id="results-artifacts-heading">Output files</h3>
        <ArtifactList artifacts={artifacts} />
      </section>

      <section aria-labelledby="results-log-heading">
        <h3 id="results-log-heading">Log</h3>
        <Button variant="secondary" onClick={() => setLogVisible((v) => !v)} aria-expanded={logVisible}>
          {logVisible ? "Hide Log" : "View Log"}
        </Button>
        {logVisible && <LiveLog logs={jobRun.state.logs} />}
      </section>

      <Banner tone="info">
        Your original source files were not changed. This job has been added to your local job
        history.
      </Banner>

      {saveJob.status === "saved" && (
        <Banner tone="success" role="status">
          Job file saved to <code>{saveJob.path}</code>.
        </Banner>
      )}
      {saveJob.status === "error" && (
        <Banner tone="danger" role="alert">
          {saveJob.message}
        </Banner>
      )}

      <div className="results-actions">
        <Button variant="secondary" onClick={() => void handleSaveJob()} busy={saveJob.status === "saving"}>
          Save Job
        </Button>
        <Button variant="secondary" onClick={handleRerun}>
          Rerun Job
        </Button>
        <Button variant="primary" onClick={handleStartNew}>
          Start New Job
        </Button>
      </div>
    </section>
  );
}
