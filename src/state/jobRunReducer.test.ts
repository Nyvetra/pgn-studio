// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import type { JobEvent, JobResult } from "../ipc/client";
import {
  createInitialJobRunState,
  jobRunReducer,
  LOG_RING_LIMIT,
  selectElapsedMs,
  selectIsTerminal,
  selectLastLogLine,
  type JobRunState,
} from "./jobRunReducer";

const JOB_A = "11111111-1111-4111-8111-111111111111";
const JOB_B = "22222222-2222-4222-8222-222222222222";

function accepted(jobId = JOB_A): JobRunState {
  return jobRunReducer(createInitialJobRunState(), {
    type: "JOB_ACCEPTED",
    jobId,
    startedAt: "2026-08-10T10:00:00Z",
  });
}

function logEvent(jobId: string, seq: number, line = `line-${seq}`): JobEvent {
  return { type: "log", jobId, seq, level: "info", line };
}

describe("jobRunReducer event correlation (binding rule)", () => {
  it("drops every event whose jobId does not match the active job", () => {
    const state = accepted(JOB_A);
    const next = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_B, 1) });
    expect(next).toBe(state); // unchanged reference: truly a no-op
    expect(next.logs).toHaveLength(0);
  });

  it("accepts events whose jobId matches the active job", () => {
    const state = accepted(JOB_A);
    const next = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 1) });
    expect(next.logs).toHaveLength(1);
    expect(next.lastSeq).toBe(1);
  });

  it("drops an event whose seq is not strictly greater than the last seen", () => {
    let state = accepted(JOB_A);
    state = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 5, "first") });
    const replay = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 5, "replayed") });
    expect(replay.logs).toHaveLength(1);
    expect(replay.logs[0].line).toBe("first");

    const stale = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 3, "stale") });
    expect(stale.logs).toHaveLength(1);
  });

  it("accepts strictly increasing seq numbers, including gaps", () => {
    let state = accepted(JOB_A);
    state = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 2) });
    state = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 7) });
    expect(state.logs.map((l) => l.seq)).toEqual([2, 7]);
    expect(state.lastSeq).toBe(7);
  });

  it("ignores stray events before any job has been accepted", () => {
    const state = createInitialJobRunState();
    const next = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 1) });
    expect(next).toBe(state);
  });

  it("a fresh JOB_ACCEPTED for a new job makes the previous job's late events unreachable", () => {
    let state = accepted(JOB_A);
    state = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 1) });
    state = jobRunReducer(state, { type: "JOB_ACCEPTED", jobId: JOB_B, startedAt: "2026-08-10T11:00:00Z" });
    expect(state.logs).toHaveLength(0);
    const lateEventFromOldJob = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, 99) });
    expect(lateEventFromOldJob.logs).toHaveLength(0);
  });
});

describe("jobRunReducer event handling", () => {
  it("stage events update stage and message", () => {
    const state = jobRunReducer(accepted(), {
      type: "EVENT",
      event: { type: "stage", jobId: JOB_A, seq: 1, stage: "processing", message: "Processing" },
    });
    expect(state.stage).toBe("processing");
    expect(state.stageMessage).toBe("Processing");
  });

  it("metrics events replace the metrics snapshot wholesale", () => {
    const state = jobRunReducer(accepted(), {
      type: "EVENT",
      event: {
        type: "metrics",
        jobId: JOB_A,
        seq: 1,
        metrics: {
          inputFiles: 2,
          inputBytes: 1000,
          processedGames: 40,
          inputGames: null,
          outputGames: null,
          duplicateGames: null,
          brokenGames: null,
          outputBytes: null,
        },
      },
    });
    expect(state.metrics?.processedGames).toBe(40);
  });

  it("artifact events accumulate rather than replace", () => {
    let state = accepted();
    state = jobRunReducer(state, {
      type: "EVENT",
      event: {
        type: "artifact",
        jobId: JOB_A,
        seq: 1,
        artifact: { kind: "uniqueGames", path: "C:\\out.pgn", sizeBytes: 10 },
      },
    });
    state = jobRunReducer(state, {
      type: "EVENT",
      event: {
        type: "artifact",
        jobId: JOB_A,
        seq: 2,
        artifact: { kind: "logText", path: "C:\\out.log.txt", sizeBytes: 5 },
      },
    });
    expect(state.artifacts).toHaveLength(2);
  });

  it("a completed event sets the terminal status and result", () => {
    const result: JobResult = {
      jobId: JOB_A,
      status: "succeeded",
      startedAt: "2026-08-10T10:00:00Z",
      finishedAt: "2026-08-10T10:00:05Z",
      elapsedMs: 5000,
      engine: { version: "v26-06", sha256: "a".repeat(64), targetTriple: "x86_64-pc-windows-msvc" },
      artifacts: [],
      metrics: {
        inputFiles: 1,
        inputBytes: 10,
        processedGames: 1,
        inputGames: 1,
        outputGames: 1,
        duplicateGames: null,
        brokenGames: null,
        outputBytes: 10,
      },
      warnings: [],
      error: null,
    };
    const state = jobRunReducer(accepted(), {
      type: "EVENT",
      event: { type: "completed", jobId: JOB_A, seq: 1, result },
    });
    expect(state.status).toBe("succeeded");
    expect(state.result).toEqual(result);
    expect(selectIsTerminal(state)).toBe(true);
  });

  it("caps the live log ring buffer at LOG_RING_LIMIT entries, keeping the newest", () => {
    let state = accepted();
    for (let seq = 1; seq <= LOG_RING_LIMIT + 50; seq += 1) {
      state = jobRunReducer(state, { type: "EVENT", event: logEvent(JOB_A, seq) });
    }
    expect(state.logs).toHaveLength(LOG_RING_LIMIT);
    expect(state.logs[state.logs.length - 1].line).toBe(`line-${LOG_RING_LIMIT + 50}`);
    expect(selectLastLogLine(state)).toBe(`line-${LOG_RING_LIMIT + 50}`);
  });
});

describe("START_FAILED", () => {
  it("records the start error without ever reaching running", () => {
    const state = jobRunReducer(createInitialJobRunState(), {
      type: "START_FAILED",
      error: {
        code: "JOB_ALREADY_RUNNING",
        title: "A job is already running",
        message: "Only one job can run at a time.",
        remediation: null,
        logPath: null,
        technicalId: "x",
      },
    });
    expect(state.status).toBe("idle");
    expect(state.startError?.code).toBe("JOB_ALREADY_RUNNING");
  });
});

describe("selectElapsedMs", () => {
  it("prefers the terminal result's own elapsedMs once available, via the real completed-event path", () => {
    const state = jobRunReducer(accepted(), {
      type: "EVENT",
      event: {
        type: "completed",
        jobId: JOB_A,
        seq: 1,
        result: {
          jobId: JOB_A,
          status: "succeeded",
          startedAt: "2026-08-10T10:00:00Z",
          finishedAt: "2026-08-10T10:00:05Z",
          elapsedMs: 5000,
          engine: { version: "v26-06", sha256: "a".repeat(64), targetTriple: "x86_64-pc-windows-msvc" },
          artifacts: [],
          metrics: {
            inputFiles: 1,
            inputBytes: 10,
            processedGames: null,
            inputGames: null,
            outputGames: null,
            duplicateGames: null,
            brokenGames: null,
            outputBytes: null,
          },
          warnings: [],
          error: null,
        },
      },
    });
    // The clock must stay frozen at the real duration, not keep counting up
    // just because more wall-clock time has passed since termination —
    // this is the whole reason elapsedMs is captured explicitly on state
    // rather than re-derived from `result` at read time.
    const muchLater = Date.parse("2026-08-10T10:00:05Z") + 10 * 60 * 1000;
    expect(selectElapsedMs(state, muchLater)).toBe(5000);
  });

  it("computes a live elapsed time from startedAt while still running (no terminal elapsedMs yet)", () => {
    const state: JobRunState = { ...createInitialJobRunState(), startedAt: "2026-08-10T10:00:00.000Z" };
    const now = Date.parse("2026-08-10T10:00:07.500Z");
    expect(selectElapsedMs(state, now)).toBe(7500);
  });

  it("returns null when there is nothing to measure from yet", () => {
    expect(selectElapsedMs(createInitialJobRunState(), Date.now())).toBeNull();
  });

  it("RECONCILE captures elapsedMs too, so a job resolved only via get_job (never the live completed event) still freezes correctly", () => {
    const state = jobRunReducer(accepted(), {
      type: "RECONCILE",
      record: {
        jobId: JOB_A,
        name: "test",
        status: "succeeded",
        startedAt: "2026-08-10T10:00:00Z",
        finishedAt: "2026-08-10T10:00:05Z",
        elapsedMs: 5000,
        engineVersion: "v26-06",
        artifacts: [],
        metrics: null,
        warnings: [],
        error: null,
      },
    });
    const muchLater = Date.parse("2026-08-10T10:00:05Z") + 10 * 60 * 1000;
    expect(selectElapsedMs(state, muchLater)).toBe(5000);
  });
});
