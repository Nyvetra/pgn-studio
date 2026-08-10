// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Orchestrates a single job run for the Run/Results screen (architecture.md
 * §13.6, §13.7; design-02 §4.2).
 *
 * Binding rule (task spec, restated): event listeners are registered
 * *before* `start_job` is invoked, so there is no gap in which an early
 * event could be missed — `client.ts`'s `startJob` wrapper does not resolve
 * until the backend has actually committed to running the job, which is
 * exactly what makes awaiting the `listen()` calls first sufficient.
 */
import { useCallback, useEffect, useReducer, useRef } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { cancelJob, getJob, startJob, type JobSpec } from "../../ipc/client";
import {
  onJobArtifact,
  onJobCompleted,
  onJobLog,
  onJobMetrics,
  onJobStage,
  onJobState,
} from "../../ipc/events";
import {
  createInitialJobRunState,
  jobRunReducer,
  selectIsTerminal,
  type JobRunState,
} from "../../state/jobRunReducer";

export interface UseJobRun {
  state: JobRunState;
  /** Registers listeners, then calls `start_job`. Resolves once the job is
   * either accepted (state transitions to `running`) or definitely rejected
   * (`state.startError` is set) — never once the whole run finishes. */
  start: (spec: JobSpec) => Promise<void>;
  cancel: () => Promise<void>;
  /** Clears live job state back to idle (Results screen's "Start New Job")
   * without starting anything new. `start()` already does this itself
   * before running the next job, so this is only needed when leaving the
   * Run/Results step without immediately starting another run. */
  reset: () => void;
}

export function useJobRun(): UseJobRun {
  const [state, dispatch] = useReducer(jobRunReducer, undefined, createInitialJobRunState);
  const unlistenFns = useRef<UnlistenFn[]>([]);

  const cleanupListeners = useCallback(() => {
    for (const unlisten of unlistenFns.current) unlisten();
    unlistenFns.current = [];
  }, []);

  useEffect(() => cleanupListeners, [cleanupListeners]);

  const start = useCallback(
    async (spec: JobSpec) => {
      cleanupListeners();
      dispatch({ type: "RESET" });

      // Register every listener before calling start_job (binding
      // correlation rule — see this module's doc comment).
      unlistenFns.current = await Promise.all([
        onJobState((event) => dispatch({ type: "EVENT", event })),
        onJobStage((event) => dispatch({ type: "EVENT", event })),
        onJobLog((event) => dispatch({ type: "EVENT", event })),
        onJobMetrics((event) => dispatch({ type: "EVENT", event })),
        onJobArtifact((event) => dispatch({ type: "EVENT", event })),
        onJobCompleted((event) => dispatch({ type: "EVENT", event })),
      ]);

      const result = await startJob(spec);
      if (result.status === "ok") {
        dispatch({ type: "JOB_ACCEPTED", jobId: result.data.jobId, startedAt: result.data.startedAt });
      } else {
        cleanupListeners();
        dispatch({ type: "START_FAILED", error: result.error });
      }
    },
    [cleanupListeners],
  );

  const cancel = useCallback(async () => {
    if (!state.jobId) return;
    // The backend reports the resulting `Cancelling`/`Cancelled` transition
    // through the normal `job://state` event; this call only *requests* it.
    await cancelJob(state.jobId);
  }, [state.jobId]);

  // Reconcile via get_job once the job reaches a terminal state (design-02
  // §4.2: "job://completed is also mirrored by get_job for reconciliation").
  useEffect(() => {
    if (!state.jobId || !selectIsTerminal(state)) return;
    let cancelled = false;
    const jobId = state.jobId;
    void getJob(jobId).then((result) => {
      if (cancelled || result.status !== "ok") return;
      dispatch({ type: "RECONCILE", record: result.data });
    });
    return () => {
      cancelled = true;
    };
    // Intentionally scoped to `jobId`/`status` (the two fields that decide
    // whether this effect should run at all), not the whole `state` object:
    // `state` gets a new reference on every log/metrics/artifact event
    // while a job is running, which would otherwise refire this fetch far
    // more often than "once per terminal transition". `dispatch` is the
    // stable function `useReducer` returns and never needs to be a dep.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.jobId, state.status]);

  const reset = useCallback(() => {
    cleanupListeners();
    dispatch({ type: "RESET" });
  }, [cleanupListeners]);

  return { state, start, cancel, reset };
}
