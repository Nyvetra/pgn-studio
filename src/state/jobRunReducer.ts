// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Live state for the Run/Results screen (architecture.md §13.6, §13.7).
 *
 * Implements the binding event-correlation rule (design-02 §4.2, restated
 * in the task spec): drop any event whose `jobId` differs from the active
 * job, and any whose `seq` is not strictly greater than the last one seen.
 * That check lives directly in this reducer (not in the component that
 * dispatches) so it is a pure function the "event correlation by job ID"
 * test target (architecture.md §20.1) can exercise without mocking IPC or
 * React at all.
 */
import type {
  JobEvent,
  JobRecordDto,
  JobStage,
  JobStatus,
  LogLevel,
  OutputArtifact,
  ProcessingMetrics,
  PublicError,
  JobResult,
  WarningRecord,
} from "../ipc/client";

/** Bounded live log (architecture.md §13.6, §10.9: "keep, for example, the
 * most recent 2,000 rendered lines"). */
export const LOG_RING_LIMIT = 2000;

export interface LogEntry {
  seq: number;
  level: LogLevel;
  line: string;
}

export interface JobRunState {
  jobId: string | null;
  lastSeq: number;
  status: JobStatus | "idle";
  stage: JobStage | null;
  stageMessage: string;
  startedAt: string | null;
  metrics: ProcessingMetrics | null;
  artifacts: OutputArtifact[];
  logs: LogEntry[];
  /** Set once terminal, from whichever arrives — the `job://completed`
   * event's `result.elapsedMs`, or (if only ever reconciled via `get_job`,
   * e.g. after the frontend reloaded mid-run) the persisted record's own
   * `elapsedMs`. Kept as its own field, separate from `result`, so
   * `selectElapsedMs` freezes at the real duration even on the
   * reconcile-only path instead of falling back to a live tick that would
   * otherwise keep counting up forever after the job already ended. */
  elapsedMs: number | null;
  /** Set once by the terminal `job://completed` event. */
  result: JobResult | null;
  /** Set if `start_job` itself failed (e.g. `JOB_ALREADY_RUNNING`) — this
   * job never reached `Running`, so nothing above applies. */
  startError: PublicError | null;
}

export function createInitialJobRunState(): JobRunState {
  return {
    jobId: null,
    lastSeq: 0,
    status: "idle",
    stage: null,
    stageMessage: "",
    startedAt: null,
    metrics: null,
    artifacts: [],
    logs: [],
    elapsedMs: null,
    result: null,
    startError: null,
  };
}

export type JobRunAction =
  | { type: "JOB_ACCEPTED"; jobId: string; startedAt: string }
  | { type: "START_FAILED"; error: PublicError }
  | { type: "EVENT"; event: JobEvent }
  | { type: "RECONCILE"; record: JobRecordDto }
  | { type: "RESET" };

function pushLog(logs: LogEntry[], entry: LogEntry): LogEntry[] {
  const next = logs.length >= LOG_RING_LIMIT ? logs.slice(logs.length - LOG_RING_LIMIT + 1) : logs.slice();
  next.push(entry);
  return next;
}

/** `WarningRecord`/`JobWarning` are structurally identical ({code, message})
 * but nominally distinct generated types; this alias documents that this
 * reducer treats them the same way. */
export type NormalizedWarning = WarningRecord;

export function jobRunReducer(state: JobRunState, action: JobRunAction): JobRunState {
  switch (action.type) {
    case "JOB_ACCEPTED":
      return {
        ...createInitialJobRunState(),
        jobId: action.jobId,
        startedAt: action.startedAt,
        status: "running",
      };
    case "START_FAILED":
      return { ...createInitialJobRunState(), startError: action.error };
    case "RESET":
      return createInitialJobRunState();
    case "RECONCILE":
      if (state.jobId !== null && action.record.jobId !== state.jobId) return state;
      return {
        ...state,
        jobId: action.record.jobId,
        status: action.record.status,
        startedAt: action.record.startedAt,
        metrics: action.record.metrics ?? state.metrics,
        artifacts: action.record.artifacts.length > 0 ? action.record.artifacts : state.artifacts,
        elapsedMs: action.record.elapsedMs ?? state.elapsedMs,
      };
    case "EVENT": {
      const event = action.event;
      // --- Correlation rule (binding) ---------------------------------
      if (state.jobId === null || event.jobId !== state.jobId) return state;
      if (event.seq <= state.lastSeq) return state;

      const base = { ...state, lastSeq: event.seq };
      switch (event.type) {
        case "state":
          return { ...base, status: event.state };
        case "stage":
          return { ...base, stage: event.stage, stageMessage: event.message };
        case "log":
          return {
            ...base,
            logs: pushLog(base.logs, { seq: event.seq, level: event.level, line: event.line }),
          };
        case "metrics":
          return { ...base, metrics: event.metrics };
        case "artifact":
          return { ...base, artifacts: [...base.artifacts, event.artifact] };
        case "completed":
          return {
            ...base,
            status: event.result.status,
            result: event.result,
            elapsedMs: event.result.elapsedMs,
          };
        default:
          return base;
      }
    }
    default:
      return state;
  }
}

// ---------------------------------------------------------------------
// Selectors
// ---------------------------------------------------------------------

export function selectIsTerminal(state: JobRunState): boolean {
  return state.status === "succeeded" || state.status === "failed" || state.status === "cancelled";
}

export function selectLastLogLine(state: JobRunState): string | null {
  if (state.logs.length === 0) return null;
  return state.logs[state.logs.length - 1].line;
}

/** Live elapsed time. The caller supplies `now` (rather than this reading
 * `Date.now()` itself) so it stays a pure, deterministically testable
 * function — the ticking clock belongs in the component. */
export function selectElapsedMs(state: JobRunState, now: number): number | null {
  if (state.elapsedMs !== null) return state.elapsedMs;
  if (!state.startedAt) return null;
  const started = Date.parse(state.startedAt);
  if (Number.isNaN(started)) return null;
  return Math.max(0, now - started);
}
