// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Results screen (architecture.md §13.7): status, elapsed time, metrics
 * (unknown values as "Not available", never 0), artifact list, View Log,
 * Rerun Job, Start New Job.
 *
 * "Save Job" (task spec, §13.7): every completed job is already recorded
 * to the bounded local job history automatically by the backend
 * (`application::jobs::record_history`, called unconditionally after every
 * run) — there is no separate save *action* to perform, so this screen
 * states that fact rather than adding a button with nothing real behind
 * it. There is no `export_job_spec`-style IPC command to save a job to an
 * arbitrary user-chosen file, and browsing/reopening saved job history is
 * explicitly Version 1.1 scope (`src/features/history/README.md`) — both
 * reported rather than worked around; see the Phase 2b report.
 */
import { useState } from "react";
import { useWorkflow } from "../../state/useWorkflow";
import type { JobRunState } from "../../state/jobRunReducer";
import { formatBytes, formatCount, formatDuration } from "../../state/formatters";
import { Banner, type BannerTone } from "../../components/Banner";
import { Button } from "../../components/Button";
import { Metric, MetricList } from "../../components/Metric";
import "../../components/workflow-screen.css";
import "./ResultsScreen.css";
import { ArtifactList } from "./ArtifactList";
import { LiveLog } from "../execution/LiveLog";
import { useJobRunContext } from "../execution/useJobRunContext";

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

  function handleRerun() {
    dispatch({ type: "REGENERATE_JOB_ID" });
    dispatch({ type: "GO_TO_STEP", step: "review" });
  }

  function handleStartNew() {
    jobRun.reset();
    dispatch({ type: "RESET_FOR_NEW_JOB" });
  }

  return (
    <section className="workflow-screen" aria-labelledby="results-heading">
      <h2 id="results-heading">Results</h2>

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

      <div className="results-actions">
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
