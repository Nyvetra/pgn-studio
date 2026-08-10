// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * The Files/Operations/Filters/Review draft store (architecture.md §13.1).
 * Backend-owned runtime states (`Running`/`Cancelling`/terminal) live in
 * `jobRunReducer.ts` instead — this reducer only ever describes the
 * *draft* the user is assembling before `start_job` is called (design-02
 * §2.1: "Draft/Validating/Ready are spec-lifecycle states held in frontend
 * store").
 *
 * Pure and framework-free by design (§20.1 "reducers/stores" is a named
 * test target) — `WorkflowContext.tsx` is the only module that wires this
 * to React and to IPC side effects.
 */
import type {
  BrokenOutput,
  CleanupOptions,
  ConflictPolicy,
  DuplicatePolicy,
  EngineCapabilities,
  InputInspectionDto,
  JobMode,
  OperationPlan,
  OutputNotation,
  RuntimeOptions,
  ValidationReportDto,
} from "../ipc/client";
import type {
  ArtifactPreferences,
  DraftInput,
  EcoEntry,
  FilterDraft,
  OutputDestination,
  PresetId,
  WorkflowStep,
} from "../types/workflow";
import { WORKFLOW_STEPS } from "../types/workflow";
import {
  defaultArtifactPreferences,
  defaultFilterDraft,
  defaultOperationPlan,
  defaultOutputDestination,
  defaultRuntimeOptions,
  generateId,
} from "./defaults";
import { matchesPreset, getPreset } from "./presets";
import { filterDraftHasProblems } from "./filterMapping";

export interface WorkflowState {
  step: WorkflowStep;
  farthestStepIndex: number;
  jobId: string;
  inputs: DraftInput[];
  output: OutputDestination;
  operations: OperationPlan;
  uniqueGames: boolean;
  artifacts: ArtifactPreferences;
  filters: FilterDraft;
  runtime: RuntimeOptions;
  capabilities: EngineCapabilities | null;
  validation: ValidationReportDto | null;
  validating: boolean;
  /** Bumped on every change that should invalidate a stale in-flight/last
   * `validate_job` result, so the UI never shows a validation report that no
   * longer matches the current draft. */
  specRevision: number;
}

export function createInitialWorkflowState(): WorkflowState {
  return {
    step: "files",
    farthestStepIndex: 0,
    jobId: generateId(),
    inputs: [],
    output: defaultOutputDestination(),
    operations: defaultOperationPlan(),
    uniqueGames: true,
    artifacts: defaultArtifactPreferences(),
    filters: defaultFilterDraft(),
    runtime: defaultRuntimeOptions(),
    capabilities: null,
    validation: null,
    validating: false,
    specRevision: 0,
  };
}

export type WorkflowAction =
  | { type: "GO_TO_STEP"; step: WorkflowStep }
  | { type: "GO_NEXT" }
  | { type: "GO_BACK" }
  | { type: "ADD_INPUTS"; paths: string[] }
  | { type: "REMOVE_INPUT"; id: string }
  | { type: "MOVE_INPUT"; id: string; direction: "up" | "down" }
  | { type: "APPLY_INSPECTIONS"; inspections: InputInspectionDto[] }
  | { type: "SET_OUTPUT_DIRECTORY"; directory: string }
  | { type: "SET_BASE_NAME"; baseName: string }
  | { type: "SET_CONFLICT_POLICY"; policy: ConflictPolicy }
  | { type: "CONFIRM_REPLACE" }
  | { type: "APPLY_PRESET"; presetId: Exclude<PresetId, "custom"> }
  | { type: "SET_MODE"; mode: JobMode }
  | { type: "SET_DUPLICATE_POLICY"; policy: DuplicatePolicy }
  | { type: "SET_BROKEN_OUTPUT"; value: BrokenOutput }
  | { type: "SET_CLEANUP"; patch: Partial<CleanupOptions> }
  | { type: "SET_ECO_ENABLED"; enabled: boolean }
  | { type: "SET_OUTPUT_NOTATION"; notation: OutputNotation }
  | { type: "SET_CHECK_FILE"; path: string | null }
  | { type: "SET_UNIQUE_GAMES"; value: boolean }
  | { type: "SET_ARTIFACTS"; patch: Partial<ArtifactPreferences> }
  | { type: "SET_RUNTIME"; patch: Partial<RuntimeOptions> }
  | { type: "SET_FILTERS"; patch: Partial<FilterDraft> }
  | { type: "ADD_ECO_ENTRY"; value: string }
  | { type: "UPDATE_ECO_ENTRY"; id: string; patch: Partial<EcoEntry> }
  | { type: "REMOVE_ECO_ENTRY"; id: string }
  | { type: "SET_CAPABILITIES"; capabilities: EngineCapabilities }
  | { type: "SET_VALIDATING" }
  | { type: "SET_VALIDATION_RESULT"; report: ValidationReportDto; specRevision: number }
  | { type: "SET_VALIDATING_FAILED"; specRevision: number }
  | { type: "RESET_FOR_NEW_JOB" }
  | { type: "REGENERATE_JOB_ID" };

function stepIndex(step: WorkflowStep): number {
  return WORKFLOW_STEPS.indexOf(step);
}

function reorder(id: string, direction: "up" | "down", inputs: DraftInput[]): DraftInput[] {
  const index = inputs.findIndex((i) => i.id === id);
  if (index === -1) return inputs;
  const target = direction === "up" ? index - 1 : index + 1;
  if (target < 0 || target >= inputs.length) return inputs;
  const next = inputs.slice();
  const [item] = next.splice(index, 1);
  next.splice(target, 0, item);
  return next;
}

/** Any change that could affect what `validate_job` would say must bump
 * `specRevision`, so a stale validation report is never displayed as if it
 * still applied (a `RESULT`-shaped guard, exercised by
 * `selectValidationIsStale`). */
function bump(state: WorkflowState): number {
  return state.specRevision + 1;
}

export function workflowReducer(state: WorkflowState, action: WorkflowAction): WorkflowState {
  switch (action.type) {
    case "GO_TO_STEP": {
      const target = stepIndex(action.step);
      if (target === -1 || target > state.farthestStepIndex) return state;
      return { ...state, step: action.step };
    }
    case "GO_NEXT": {
      const next = Math.min(stepIndex(state.step) + 1, WORKFLOW_STEPS.length - 1);
      return {
        ...state,
        step: WORKFLOW_STEPS[next],
        farthestStepIndex: Math.max(state.farthestStepIndex, next),
      };
    }
    case "GO_BACK": {
      const prev = Math.max(stepIndex(state.step) - 1, 0);
      return { ...state, step: WORKFLOW_STEPS[prev] };
    }
    case "ADD_INPUTS": {
      const additions: DraftInput[] = action.paths.map((path) => ({
        id: generateId(),
        path,
        displayName: path.split(/[\\/]/).pop() ?? path,
        sizeBytes: null,
        isReadable: null,
        extensionOk: null,
        warnings: [],
        inspected: false,
      }));
      return {
        ...state,
        inputs: [...state.inputs, ...additions],
        specRevision: bump(state),
      };
    }
    case "REMOVE_INPUT":
      return {
        ...state,
        inputs: state.inputs.filter((i) => i.id !== action.id),
        specRevision: bump(state),
      };
    case "MOVE_INPUT":
      return {
        ...state,
        inputs: reorder(action.id, action.direction, state.inputs),
        specRevision: bump(state),
      };
    case "APPLY_INSPECTIONS": {
      const byPath = new Map(action.inspections.map((i) => [i.path, i]));
      return {
        ...state,
        inputs: state.inputs.map((input) => {
          const found = byPath.get(input.path);
          if (!found) return input;
          return {
            ...input,
            sizeBytes: found.sizeBytes,
            isReadable: found.isReadable,
            extensionOk: found.extensionOk,
            warnings: found.warnings,
            inspected: true,
          };
        }),
      };
    }
    case "SET_OUTPUT_DIRECTORY":
      return {
        ...state,
        output: { ...state.output, directory: action.directory },
        specRevision: bump(state),
      };
    case "SET_BASE_NAME":
      return {
        ...state,
        output: { ...state.output, baseName: action.baseName },
        specRevision: bump(state),
      };
    case "SET_CONFLICT_POLICY":
      // Any change of policy invalidates a prior confirmation (architecture.md
      // §11.5: "silent overwrite is prohibited") — re-selecting
      // replaceAfterConfirmation must always require a fresh confirm dialog.
      return {
        ...state,
        output: { ...state.output, conflictPolicy: action.policy, confirmedReplace: false },
        specRevision: bump(state),
      };
    case "CONFIRM_REPLACE":
      return {
        ...state,
        output: { ...state.output, confirmedReplace: true },
        specRevision: bump(state),
      };
    case "APPLY_PRESET": {
      const preset = getPreset(action.presetId);
      return {
        ...state,
        operations: { ...preset.effect.operations },
        uniqueGames: preset.effect.uniqueGames,
        artifacts: { ...preset.effect.artifacts },
        specRevision: bump(state),
      };
    }
    case "SET_MODE": {
      const forcingValidateOnly = action.mode === "validateOnly";
      return {
        ...state,
        operations: {
          ...state.operations,
          mode: action.mode,
          duplicates: forcingValidateOnly ? "none" : state.operations.duplicates,
        },
        uniqueGames: forcingValidateOnly ? false : state.uniqueGames,
        artifacts: forcingValidateOnly
          ? { ...state.artifacts, duplicateGames: "none" }
          : state.artifacts,
        specRevision: bump(state),
      };
    }
    case "SET_DUPLICATE_POLICY":
      // architecture.md §10.7's safe default is "create a unique-games file
      // AND a duplicate-games audit file" — so choosing "keep first copy,
      // save the rest to an audit file" must actually publish that file by
      // default (the user can still turn publishing off afterward via its
      // own control). Any other policy has no audit file to publish at all.
      return {
        ...state,
        operations: { ...state.operations, duplicates: action.policy },
        artifacts: {
          ...state.artifacts,
          duplicateGames: action.policy === "reportAndKeepFirst" ? "audit" : "none",
          alwaysCreateAudit: action.policy === "reportAndKeepFirst" ? state.artifacts.alwaysCreateAudit : false,
        },
        specRevision: bump(state),
      };
    case "SET_BROKEN_OUTPUT":
      return {
        ...state,
        operations: { ...state.operations, broken: action.value },
        specRevision: bump(state),
      };
    case "SET_CLEANUP":
      return {
        ...state,
        operations: {
          ...state.operations,
          cleanup: { ...state.operations.cleanup, ...action.patch },
        },
        specRevision: bump(state),
      };
    case "SET_ECO_ENABLED":
      return {
        ...state,
        operations: { ...state.operations, eco: { enabled: action.enabled } },
        specRevision: bump(state),
      };
    case "SET_OUTPUT_NOTATION":
      return {
        ...state,
        operations: { ...state.operations, outputNotation: action.notation },
        specRevision: bump(state),
      };
    case "SET_CHECK_FILE":
      return {
        ...state,
        operations: { ...state.operations, checkFile: action.path },
        specRevision: bump(state),
      };
    case "SET_UNIQUE_GAMES":
      return { ...state, uniqueGames: action.value, specRevision: bump(state) };
    case "SET_ARTIFACTS":
      return {
        ...state,
        artifacts: { ...state.artifacts, ...action.patch },
        specRevision: bump(state),
      };
    case "SET_RUNTIME":
      return {
        ...state,
        runtime: { ...state.runtime, ...action.patch },
        specRevision: bump(state),
      };
    case "SET_FILTERS":
      return {
        ...state,
        filters: { ...state.filters, ...action.patch },
        specRevision: bump(state),
      };
    case "ADD_ECO_ENTRY":
      return {
        ...state,
        filters: {
          ...state.filters,
          ecoEntries: [
            ...state.filters.ecoEntries,
            { id: generateId(), value: action.value, exclude: false },
          ],
        },
        specRevision: bump(state),
      };
    case "UPDATE_ECO_ENTRY":
      return {
        ...state,
        filters: {
          ...state.filters,
          ecoEntries: state.filters.ecoEntries.map((e) =>
            e.id === action.id ? { ...e, ...action.patch } : e,
          ),
        },
        specRevision: bump(state),
      };
    case "REMOVE_ECO_ENTRY":
      return {
        ...state,
        filters: {
          ...state.filters,
          ecoEntries: state.filters.ecoEntries.filter((e) => e.id !== action.id),
        },
        specRevision: bump(state),
      };
    case "SET_CAPABILITIES":
      return { ...state, capabilities: action.capabilities };
    case "SET_VALIDATING":
      return { ...state, validating: true };
    case "SET_VALIDATION_RESULT":
      // Drop stale responses: only accept a result computed for the spec
      // revision we most recently asked about.
      if (action.specRevision !== state.specRevision) return { ...state, validating: false };
      return { ...state, validation: action.report, validating: false };
    case "SET_VALIDATING_FAILED":
      // A raw transport failure: stop the spinner without fabricating a
      // validation report. Still revision-guarded, for the same reason as
      // SET_VALIDATION_RESULT above.
      if (action.specRevision !== state.specRevision) return state;
      return { ...state, validating: false };
    case "RESET_FOR_NEW_JOB": {
      const fresh = createInitialWorkflowState();
      return { ...fresh, capabilities: state.capabilities };
    }
    case "REGENERATE_JOB_ID":
      // "Rerun Job" (§13.7) keeps every setting but must not reuse the
      // previous run's job id — each run gets its own workspace/history
      // entry (persistence::history "recording the same job id again
      // replaces rather than duplicates" — a rerun should add a new
      // history entry, not silently overwrite the one just created).
      return { ...state, jobId: generateId() };
    default:
      return state;
  }
}

// ---------------------------------------------------------------------
// Selectors — pure functions of WorkflowState, kept alongside the reducer
// so business rules (e.g. what "ready to proceed" means) live in one
// tested place rather than being re-decided inside each screen component.
// ---------------------------------------------------------------------

export function selectActivePreset(state: WorkflowState): PresetId {
  const current = {
    operations: state.operations,
    uniqueGames: state.uniqueGames,
    artifacts: state.artifacts,
  };
  const ids: Exclude<PresetId, "custom">[] = [
    "mergeSafely",
    "cleanCollection",
    "minimalMainline",
    "lucenaReady",
    "validateOnly",
    "newGamesAgainstMaster",
  ];
  return ids.find((id) => matchesPreset(id, current)) ?? "custom";
}

export function selectFilesStepReady(state: WorkflowState): boolean {
  return (
    state.inputs.length > 0 &&
    state.output.directory.trim().length > 0 &&
    state.output.baseName.trim().length > 0
  );
}

export function selectFiltersAreValid(state: WorkflowState): boolean {
  return !filterDraftHasProblems(state.filters);
}

export function selectValidationIsStale(state: WorkflowState): boolean {
  return state.validation === null || state.validating;
}

export function selectCanRun(state: WorkflowState): boolean {
  return (
    !selectValidationIsStale(state) &&
    state.validation !== null &&
    state.validation.status === "ready"
  );
}
