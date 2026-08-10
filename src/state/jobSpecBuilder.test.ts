// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { createInitialWorkflowState, workflowReducer } from "./workflowReducer";
import { buildJobSpec, deriveJobName, JOB_SPEC_SCHEMA_VERSION } from "./jobSpecBuilder";

describe("deriveJobName", () => {
  it("uses the trimmed base name", () => {
    expect(deriveJobName("  my-collection  ")).toBe("my-collection");
  });

  it("falls back to a placeholder when the base name is blank", () => {
    expect(deriveJobName("   ")).toBe("Untitled job");
  });
});

describe("buildJobSpec", () => {
  it("assembles a schema-version-1 spec matching the backend's CURRENT_SCHEMA_VERSION", () => {
    const state = createInitialWorkflowState();
    const spec = buildJobSpec(state);
    expect(spec.schemaVersion).toBe(1);
    expect(JOB_SPEC_SCHEMA_VERSION).toBe(1);
    expect(spec.id).toBe(state.jobId);
  });

  it("derives InputFile priority strictly from array order, starting at 0 with no gaps", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, {
      type: "ADD_INPUTS",
      paths: ["C:\\a.pgn", "C:\\b.pgn", "C:\\c.pgn"],
    });
    const spec = buildJobSpec(state);
    expect(spec.inputs.map((i) => i.priority)).toEqual([0, 1, 2]);
    expect(spec.inputs.map((i) => i.path)).toEqual(["C:\\a.pgn", "C:\\b.pgn", "C:\\c.pgn"]);
  });

  it("keeps priorities contiguous after removing a middle input (validate_job requires a gap-free sequence)", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "ADD_INPUTS", paths: ["a.pgn", "b.pgn", "c.pgn"] });
    const middleId = state.inputs[1].id;
    state = workflowReducer(state, { type: "REMOVE_INPUT", id: middleId });
    const spec = buildJobSpec(state);
    expect(spec.inputs.map((i) => i.priority)).toEqual([0, 1]);
    expect(spec.inputs.map((i) => i.path)).toEqual(["a.pgn", "c.pgn"]);
  });

  it("reflects reordering in the compiled priority sequence (input order is duplicate-retention priority)", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "ADD_INPUTS", paths: ["first.pgn", "second.pgn"] });
    const secondId = state.inputs[1].id;
    state = workflowReducer(state, { type: "MOVE_INPUT", id: secondId, direction: "up" });
    const spec = buildJobSpec(state);
    expect(spec.inputs.map((i) => i.path)).toEqual(["second.pgn", "first.pgn"]);
  });

  it("carries output destination, operations, and runtime through unchanged", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "SET_OUTPUT_DIRECTORY", directory: "C:\\out" });
    state = workflowReducer(state, { type: "SET_BASE_NAME", baseName: "clean" });
    state = workflowReducer(state, { type: "SET_ECO_ENABLED", enabled: true });
    const spec = buildJobSpec(state);
    expect(spec.output.directory).toBe("C:\\out");
    expect(spec.output.baseName).toBe("clean");
    expect(spec.name).toBe("clean");
    expect(spec.operations.eco.enabled).toBe(true);
    expect(spec.runtime).toEqual(state.runtime);
  });

  it("maps artifact preferences onto OutputPlan's duplicateGames/logFile/manifest fields", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "SET_DUPLICATE_POLICY", policy: "reportAndKeepFirst" });
    state = workflowReducer(state, { type: "SET_ARTIFACTS", patch: { duplicateGames: "audit" } });
    const spec = buildJobSpec(state);
    expect(spec.output.duplicateGames).toBe("audit");
    expect(spec.output.logFile).toBe(true);
    expect(spec.output.manifest).toBe(true);
  });

  it("compiles filters through compileFilters rather than duplicating that logic", () => {
    let state = createInitialWorkflowState();
    state = workflowReducer(state, { type: "SET_FILTERS", patch: { checkmateOnly: true } });
    const spec = buildJobSpec(state);
    expect(spec.filters.checkmateOnly).toBe(true);
    expect(spec.filters.advancedArgs).toEqual([]);
  });
});
