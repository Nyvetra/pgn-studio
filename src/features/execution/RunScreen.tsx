// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Run screen (architecture.md §13.6): current stage, honest progress
 * (indeterminate indicator + elapsed time + stage name + last log line —
 * never a fabricated percentage, architecture.md §4.7), bounded live log,
 * Cancel, output paths as they publish, and a clear statement that
 * original files are unchanged.
 */
import { useEffect, useRef, useState } from "react";
import { useWorkflow } from "../../state/useWorkflow";
import {
  selectElapsedMs,
  selectLastLogLine,
  type JobRunState,
} from "../../state/jobRunReducer";
import { ARTIFACT_KIND_LABELS, formatCount, formatDuration } from "../../state/formatters";
import { Banner } from "../../components/Banner";
import { Button } from "../../components/Button";
import { StepNav } from "../../components/StepNav";
import { ProgressIndicator } from "../../components/ProgressIndicator";
import { useAnnounce } from "../../components/useAnnounce";
import { useFocusOnMount } from "../../components/useFocusOnMount";
import "../../components/workflow-screen.css";
import "./RunScreen.css";
import { STAGE_LABELS } from "./stageLabels";
import { LiveLog } from "./LiveLog";
import { useJobRunContext } from "./useJobRunContext";

function useTicker(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

/** Announces stage and cancelling transitions (architecture.md §13.8:
 * "screen-reader announcements for stage/status changes") — deliberately
 * not every log line, which would make the app unusable with a screen
 * reader (see `LiveLog.tsx`'s doc comment). */
function useStageAnnouncements(state: JobRunState) {
  const announce = useAnnounce();
  const lastAnnouncedStage = useRef<string | null>(null);
  const lastAnnouncedStatus = useRef<string | null>(null);

  useEffect(() => {
    if (state.stage && state.stage !== lastAnnouncedStage.current) {
      lastAnnouncedStage.current = state.stage;
      announce(`Stage: ${STAGE_LABELS[state.stage]}`);
    }
  }, [state.stage, announce]);

  useEffect(() => {
    if (state.status !== lastAnnouncedStatus.current) {
      lastAnnouncedStatus.current = state.status;
      if (state.status === "cancelling") announce("Cancelling the job.");
    }
  }, [state.status, announce]);
}

export function RunScreen() {
  const { dispatch } = useWorkflow();
  const jobRun = useJobRunContext();
  const now = useTicker();
  const headingRef = useFocusOnMount<HTMLHeadingElement>();
  useStageAnnouncements(jobRun.state);

  if (jobRun.state.startError) {
    const error = jobRun.state.startError;
    return (
      <section className="workflow-screen" aria-labelledby="run-heading">
        <h2 id="run-heading" ref={headingRef} tabIndex={-1}>
          Run
        </h2>
        <Banner tone="danger" role="alert" title={error.title}>
          <p>{error.message}</p>
          {error.remediation && <p>{error.remediation}</p>}
        </Banner>
        <StepNav onBack={() => dispatch({ type: "GO_TO_STEP", step: "review" })} backLabel="Back to Review" />
      </section>
    );
  }

  if (jobRun.state.status === "idle") {
    return (
      <section className="workflow-screen" aria-labelledby="run-heading">
        <h2 id="run-heading" ref={headingRef} tabIndex={-1}>
          Run
        </h2>
        <p role="status">Waiting for the job to start…</p>
      </section>
    );
  }

  const { stage, status } = jobRun.state;
  const elapsed = selectElapsedMs(jobRun.state, now);
  const lastLine = selectLastLogLine(jobRun.state);
  const processedGames = jobRun.state.metrics?.processedGames;
  const cancelling = status === "cancelling";

  return (
    <section className="workflow-screen" aria-labelledby="run-heading">
      <h2 id="run-heading" ref={headingRef} tabIndex={-1}>
        Run
      </h2>
      <Banner tone="info">
        Your original source files are never modified while this runs — every output is a new file.
      </Banner>

      <div className="run-progress">
        <h3>{stage ? STAGE_LABELS[stage] : "Starting…"}</h3>
        <ProgressIndicator label={stage ? STAGE_LABELS[stage] : "Working"} />
        <dl className="run-progress__facts">
          <div>
            <dt>Elapsed time</dt>
            <dd>{formatDuration(elapsed)}</dd>
          </div>
          {processedGames != null && (
            <div>
              <dt>Games processed so far</dt>
              <dd>{formatCount(processedGames)}</dd>
            </div>
          )}
        </dl>
        {lastLine && (
          <p className="run-progress__last-line">
            Last log line: <code>{lastLine}</code>
          </p>
        )}
      </div>

      <Button variant="danger" onClick={() => void jobRun.cancel()} disabled={cancelling} busy={cancelling}>
        {cancelling ? "Cancelling…" : "Cancel"}
      </Button>

      <section aria-labelledby="run-artifacts-heading">
        <h3 id="run-artifacts-heading">Output files published so far</h3>
        {jobRun.state.artifacts.length === 0 ? (
          <p className="workflow-screen__section-help">None yet.</p>
        ) : (
          <ul className="run-artifact-list">
            {jobRun.state.artifacts.map((artifact) => (
              <li key={artifact.path}>
                {ARTIFACT_KIND_LABELS[artifact.kind]}: {artifact.path}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-labelledby="run-log-heading">
        <h3 id="run-log-heading">Log</h3>
        <LiveLog logs={jobRun.state.logs} />
      </section>
    </section>
  );
}
