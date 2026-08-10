// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import {
  createInitialWorkflowState,
  selectActivePreset,
  selectCanRun,
  selectFilesStepReady,
  workflowReducer,
  type WorkflowState,
} from "./workflowReducer";
import type { ValidationReportDto } from "../ipc/client";

function withInput(state: WorkflowState, path = "C:\\a.pgn"): WorkflowState {
  return workflowReducer(state, { type: "ADD_INPUTS", paths: [path] });
}

describe("workflowReducer navigation", () => {
  it("starts on the files step with only that step reachable", () => {
    const state = createInitialWorkflowState();
    expect(state.step).toBe("files");
    expect(state.farthestStepIndex).toBe(0);
  });

  it("GO_NEXT advances one step and extends the farthest-reached marker", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "GO_NEXT" });
    expect(state.step).toBe("operations");
    expect(state.farthestStepIndex).toBe(1);
  });

  it("GO_NEXT never advances past the final step", () => {
    let state = createInitialWorkflowState();
    for (let i = 0; i < 10; i += 1) {
      state = workflowReducer(state, { type: "GO_NEXT" });
    }
    expect(state.step).toBe("run-results");
  });

  it("GO_BACK moves backward without losing settings", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "SET_BASE_NAME", baseName: "keep-me" });
    state = workflowReducer(state, { type: "GO_NEXT" });
    state = workflowReducer(state, { type: "GO_BACK" });
    expect(state.step).toBe("files");
    expect(state.output.baseName).toBe("keep-me");
  });

  it("GO_TO_STEP refuses to jump ahead of the farthest step reached", () => {
    const state = createInitialWorkflowState();
    const jumped = workflowReducer(state, { type: "GO_TO_STEP", step: "review" });
    expect(jumped.step).toBe("files");
  });

  it("GO_TO_STEP allows jumping back to any already-reached step", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "GO_NEXT" });
    state = workflowReducer(state, { type: "GO_NEXT" });
    state = workflowReducer(state, { type: "GO_TO_STEP", step: "files" });
    expect(state.step).toBe("files");
    expect(state.farthestStepIndex).toBe(2); // unaffected by a backward jump
  });
});

describe("workflowReducer inputs", () => {
  it("adds inputs and derives a display name from the path", () => {
    const state = withInput(createInitialWorkflowState(), "C:\\games\\a.pgn");
    expect(state.inputs).toHaveLength(1);
    expect(state.inputs[0].displayName).toBe("a.pgn");
    expect(state.inputs[0].inspected).toBe(false);
  });

  it("removes an input by id", () => {
    let state = withInput(createInitialWorkflowState());
    const id = state.inputs[0].id;
    state = workflowReducer(state, { type: "REMOVE_INPUT", id });
    expect(state.inputs).toHaveLength(0);
  });

  it("moves an input up/down and clamps at the edges", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "ADD_INPUTS", paths: ["a.pgn", "b.pgn"] });
    const firstId = state.inputs[0].id;
    // Already at the top: moving up is a no-op.
    const unchanged = workflowReducer(state, { type: "MOVE_INPUT", id: firstId, direction: "up" });
    expect(unchanged.inputs.map((i) => i.path)).toEqual(["a.pgn", "b.pgn"]);

    const moved = workflowReducer(state, { type: "MOVE_INPUT", id: firstId, direction: "down" });
    expect(moved.inputs.map((i) => i.path)).toEqual(["b.pgn", "a.pgn"]);
  });

  it("applies inspection results by matching path", () => {
    let state = withInput(createInitialWorkflowState(), "C:\\a.pgn");
    state = workflowReducer(state, {
      type: "APPLY_INSPECTIONS",
      inspections: [
        {
          path: "C:\\a.pgn",
          displayName: "a.pgn",
          sizeBytes: 1234,
          modifiedAt: null,
          isReadable: true,
          extensionOk: true,
          sha256: null,
          warnings: [],
        },
      ],
    });
    expect(state.inputs[0].sizeBytes).toBe(1234);
    expect(state.inputs[0].inspected).toBe(true);
  });
});

describe("workflowReducer mode/duplicate consistency", () => {
  it('selecting "keep first, save an audit file" actually defaults to publishing it (§10.7 safe default)', () => {
    const state = workflowReducer(createInitialWorkflowState(), {
      type: "SET_DUPLICATE_POLICY",
      policy: "reportAndKeepFirst",
    });
    expect(state.artifacts.duplicateGames).toBe("audit");
  });

  it("switching to validateOnly forces uniqueGames false and duplicates none (compile() contract)", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "SET_DUPLICATE_POLICY", policy: "reportAndKeepFirst" });
    state = workflowReducer(state, { type: "SET_MODE", mode: "validateOnly" });
    expect(state.uniqueGames).toBe(false);
    expect(state.operations.duplicates).toBe("none");
    expect(state.artifacts.duplicateGames).toBe("none");
  });

  it("switching duplicate policy away from reportAndKeepFirst clears the audit-publish flags", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "SET_DUPLICATE_POLICY", policy: "reportAndKeepFirst" });
    state = workflowReducer(state, {
      type: "SET_ARTIFACTS",
      patch: { duplicateGames: "audit", alwaysCreateAudit: true },
    });
    state = workflowReducer(state, { type: "SET_DUPLICATE_POLICY", policy: "none" });
    expect(state.artifacts.duplicateGames).toBe("none");
    expect(state.artifacts.alwaysCreateAudit).toBe(false);
  });

  it("changing the conflict policy away from replaceAfterConfirmation resets confirmedReplace", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "SET_CONFLICT_POLICY", policy: "replaceAfterConfirmation" });
    state = workflowReducer(state, { type: "CONFIRM_REPLACE" });
    expect(state.output.confirmedReplace).toBe(true);
    state = workflowReducer(state, { type: "SET_CONFLICT_POLICY", policy: "addNumericSuffix" });
    expect(state.output.confirmedReplace).toBe(false);
  });
});

describe("workflowReducer presets", () => {
  it("APPLY_PRESET replaces operations/uniqueGames/artifacts", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "APPLY_PRESET", presetId: "minimalMainline" });
    expect(state.operations.duplicates).toBe("suppressKeepFirst");
    expect(state.operations.cleanup.removeComments).toBe(true);
    expect(selectActivePreset(state)).toBe("minimalMainline");
  });

  it("selectActivePreset reports custom after a manual edit diverges from the applied preset", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "APPLY_PRESET", presetId: "minimalMainline" });
    state = workflowReducer(state, { type: "SET_ECO_ENABLED", enabled: true });
    expect(selectActivePreset(state)).toBe("custom");
  });
});

describe("workflowReducer validation staleness", () => {
  const readyReport: ValidationReportDto = {
    status: "ready",
    errors: [],
    warnings: [],
    advisories: [],
    estimatedInputBytes: 100,
    freeDiskBytes: null,
  };

  it("a validation result for a stale spec revision is dropped", () => {
    let state = createInitialWorkflowState();
    const revisionAtRequestTime = state.specRevision;
    state = workflowReducer(state, { type: "SET_BASE_NAME", baseName: "changed-after-request" });
    state = workflowReducer(state, {
      type: "SET_VALIDATION_RESULT",
      report: readyReport,
      specRevision: revisionAtRequestTime,
    });
    expect(state.validation).toBeNull();
  });

  it("a validation result for the current spec revision is accepted", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, {
      type: "SET_VALIDATION_RESULT",
      report: readyReport,
      specRevision: state.specRevision,
    });
    expect(state.validation).toEqual(readyReport);
    expect(selectCanRun(state)).toBe(true);
  });

  it("selectCanRun is false while validating or before any report exists", () => {
    const state = createInitialWorkflowState();
    expect(selectCanRun(state)).toBe(false);
  });

  it("SET_VALIDATING_FAILED stops the spinner without fabricating a report", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "SET_VALIDATING" });
    expect(state.validating).toBe(true);
    state = workflowReducer(state, { type: "SET_VALIDATING_FAILED", specRevision: state.specRevision });
    expect(state.validating).toBe(false);
    expect(state.validation).toBeNull();
  });

  it("SET_VALIDATING_FAILED for a stale revision is ignored", () => {
    let state = createInitialWorkflowState();
    const staleRevision = state.specRevision;
    state = workflowReducer(state, { type: "SET_BASE_NAME", baseName: "changed" });
    state = workflowReducer(state, { type: "SET_VALIDATING" });
    state = workflowReducer(state, { type: "SET_VALIDATING_FAILED", specRevision: staleRevision });
    expect(state.validating).toBe(true); // untouched: the failure belonged to an old request
  });
});

describe("selectFilesStepReady", () => {
  it("requires at least one input, an output directory, and a base name", () => {
    let state = createInitialWorkflowState();
    expect(selectFilesStepReady(state)).toBe(false);
    state = withInput(state);
    expect(selectFilesStepReady(state)).toBe(false);
    state = workflowReducer(state, { type: "SET_OUTPUT_DIRECTORY", directory: "C:\\out" });
    expect(selectFilesStepReady(state)).toBe(false);
    state = workflowReducer(state, { type: "SET_BASE_NAME", baseName: "out" });
    expect(selectFilesStepReady(state)).toBe(true);
  });
});

describe("RESET_FOR_NEW_JOB", () => {
  it("returns to a fresh draft but keeps the already-fetched engine capabilities", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, {
      type: "SET_CAPABILITIES",
      capabilities: {
        identity: { version: "v26-06", sha256: "x".repeat(64), targetTriple: "x86_64-pc-windows-msvc" },
        duplicateDetection: true,
        duplicateAuditFile: true,
        externalDuplicateTable: true,
        checkFile: true,
        ecoClassification: true,
        fenPatterns: true,
        textualVariations: true,
        fixResultTags: true,
        rejectBadResults: true,
        separateBrokenOutput: false,
        supportedOutputFormats: ["san"],
        unicodePaths: false,
      },
    });
    state = withInput(state);
    state = workflowReducer(state, { type: "GO_NEXT" });
    const originalJobId = state.jobId;

    state = workflowReducer(state, { type: "RESET_FOR_NEW_JOB" });
    expect(state.inputs).toEqual([]);
    expect(state.step).toBe("files");
    expect(state.jobId).not.toBe(originalJobId);
    expect(state.capabilities).not.toBeNull();
  });
});
