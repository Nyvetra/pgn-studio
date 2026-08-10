// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Assembles the real, typed `JobSpec` wire DTO from `WorkflowState`
 * (architecture.md §9.2). This is the single place that touches every
 * field the IPC boundary expects — `validate_job`, `compile_job_preview`,
 * and `start_job` all take the exact same object this function returns.
 */
import type { InputFile, JobSpec } from "../ipc/client";
import type { WorkflowState } from "./workflowReducer";
import { compileFilters } from "./filterMapping";

/** Matches `domain::job_spec::CURRENT_SCHEMA_VERSION` (src-tauri/src/domain/job_spec.rs).
 * Not part of the generated bindings (it is a plain Rust constant, not a
 * DTO field), so it is restated here rather than left as a magic number. */
export const JOB_SPEC_SCHEMA_VERSION = 1;

const DEFAULT_JOB_NAME = "Untitled job";

/** There is no dedicated "job name" control anywhere in architecture.md
 * §13's screen list — `JobSpec.name` is derived from the output base
 * filename, which is the closest thing the workflow has to a job title. */
export function deriveJobName(baseName: string): string {
  const trimmed = baseName.trim();
  return trimmed.length > 0 ? trimmed : DEFAULT_JOB_NAME;
}

export function buildJobSpec(state: WorkflowState): JobSpec {
  const inputs: InputFile[] = state.inputs.map((input, index) => ({
    path: input.path,
    displayName: input.displayName,
    priority: index,
  }));

  return {
    schemaVersion: JOB_SPEC_SCHEMA_VERSION,
    id: state.jobId,
    name: deriveJobName(state.output.baseName),
    inputs,
    output: {
      directory: state.output.directory,
      baseName: state.output.baseName,
      uniqueGames: state.uniqueGames,
      duplicateGames: state.artifacts.duplicateGames,
      logFile: state.artifacts.logFile,
      manifest: state.artifacts.manifest,
      alwaysCreateAudit: state.artifacts.alwaysCreateAudit,
      conflictPolicy: state.output.conflictPolicy,
      confirmedReplace: state.output.confirmedReplace,
    },
    operations: state.operations,
    filters: compileFilters(state.filters),
    runtime: state.runtime,
  };
}
