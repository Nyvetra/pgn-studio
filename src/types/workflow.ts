// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * UI-only types for the five-step workflow (architecture.md §13.1). Nothing
 * here crosses the Tauri IPC boundary — wire types live in
 * `src/ipc/generated-types.ts` and are re-exported through `src/ipc/client.ts`.
 * These types describe how the workflow is *represented on screen* before
 * being compiled into a real `JobSpec` (see `src/state/jobSpecBuilder.ts`).
 */
import type {
  ConflictPolicy,
  DuplicateOutput,
  DuplicatePolicy,
  SetupPolicy,
} from "../ipc/client";

/** The five steps from architecture.md §13.1. Step 5 renders the Run screen
 * while the job is active and the Results screen once it reaches a terminal
 * state — architecture.md §13.1 names this one step ("Run & Results") even
 * though §13.6/§13.7 describe it as two screens. */
export type WorkflowStep =
  | "files"
  | "operations"
  | "filters"
  | "review"
  | "run-results";

export const WORKFLOW_STEPS: readonly WorkflowStep[] = [
  "files",
  "operations",
  "filters",
  "review",
  "run-results",
];

export const STEP_LABELS: Record<WorkflowStep, string> = {
  files: "Files",
  operations: "Operations",
  filters: "Filters",
  review: "Review",
  "run-results": "Run & Results",
};

/** Built-in presets (architecture.md §12.2). `"custom"` means the current
 * configuration no longer matches any preset's diff exactly (the user
 * edited a control by hand after applying one, or never applied one). */
export type PresetId =
  | "mergeSafely"
  | "cleanCollection"
  | "minimalMainline"
  | "lucenaReady"
  | "validateOnly"
  | "newGamesAgainstMaster"
  | "custom";

/** One source file as tracked by the Files screen (architecture.md §13.2).
 * `priority` is deliberately NOT stored here — array order in
 * `WorkflowState.inputs` *is* priority (validate_job rejects gaps/duplicates
 * in the priority sequence, so keeping a single source of truth for order
 * makes that requirement impossible to violate). `id` is a client-only key
 * for React lists and reordering, independent of the file path. */
export interface DraftInput {
  id: string;
  path: string;
  displayName: string;
  /** `null` until `inspect_inputs` returns, or if the probe failed. */
  sizeBytes: number | null;
  isReadable: boolean | null;
  extensionOk: boolean | null;
  warnings: string[];
  /** Whether `inspect_inputs` has returned for this row yet. */
  inspected: boolean;
}

/** One ECO code/prefix criterion (architecture.md §13.4; D-010: only prefix
 * ("starts with") and not-equal are reliable engine operators for ECO —
 * `=`, `>`, `>=`, `<`, `<=` silently match nothing). */
export interface EcoEntry {
  id: string;
  value: string;
  /** `false` → "starts with" (prefix); `true` → "is not" (`<>`). */
  exclude: boolean;
}

/** Which Elo tag a min/max range applies to. */
export type EloScope = "either" | "white" | "black";

/**
 * Filters screen UI state (architecture.md §13.4). Deliberately not a
 * `FilterPlan` — this is the shape controls bind to; `compileFilters` (in
 * `src/state/filterMapping.ts`) is the one place that turns it into typed
 * `TagRule[]`/`MoveBounds` values for the wire. React never composes
 * criteria-file syntax; it only ever builds these typed structures.
 */
export interface FilterDraft {
  player: string;
  white: string;
  black: string;
  resultWhiteWin: boolean;
  resultBlackWin: boolean;
  resultDraw: boolean;
  resultOther: boolean;
  /** Convenience toggle: unions in Result "1-0" and "0-1" alongside whatever
   * the explicit result checkboxes above already select. */
  decisiveOnly: boolean;
  /** Four-digit year strings, or "" for no bound. */
  dateFromYear: string;
  dateToYear: string;
  eloScope: EloScope;
  eloMin: string;
  eloMax: string;
  ecoEntries: EcoEntry[];
  moveMin: string;
  moveMax: string;
  checkmateOnly: boolean;
  setupPolicy: SetupPolicy;
}

/** Operations-screen fields that are not already a 1:1 slice of
 * `OperationPlan`/`OutputPlan` (those are kept in `WorkflowState` directly).
 * Nothing here has wire representation of its own. */
export interface ArtifactPreferences {
  logFile: boolean;
  manifest: boolean;
  duplicateGames: DuplicateOutput;
  alwaysCreateAudit: boolean;
}

export interface OutputDestination {
  directory: string;
  baseName: string;
  conflictPolicy: ConflictPolicy;
  /** Only ever set to `true` immediately after an explicit confirmation
   * dialog (architecture.md §11.5). Reset to `false` whenever the conflict
   * policy changes away from `replaceAfterConfirmation`. */
  confirmedReplace: boolean;
}

/** A11y helper: human labels for `DuplicatePolicy`, reused by Operations and
 * Review so the wording never drifts between screens. */
export const DUPLICATE_POLICY_LABELS: Record<DuplicatePolicy, string> = {
  none: "Do not check for duplicates",
  reportAndKeepFirst: "Keep first copy, save the rest to an audit file",
  suppressKeepFirst: "Keep first copy, discard the rest",
};
