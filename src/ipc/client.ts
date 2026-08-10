// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Typed wrapper around the generated Tauri command bindings.
 *
 * This is the only module the rest of the frontend is allowed to import
 * `./generated-types` (and therefore, transitively, `@tauri-apps/api/core`)
 * through — feature code must import commands and DTO types from here, not
 * from `./generated-types` directly, so the IPC boundary stays one seam
 * (architecture.md §14.1).
 *
 * `./generated-types.ts` is produced by `cargo run -p xtask --
 * export-bindings` (design-02 §4.3, decision D-17) from the
 * `tauri-specta`-annotated command surface in `src-tauri/src/commands/` —
 * it is committed, and CI fails if it drifts from the Rust source of truth
 * (`.github/workflows/rust.yml`). Do not hand-edit it; regenerate instead.
 *
 * Phase 2a scope: every command in architecture.md §14.1 has a typed
 * wrapper here. Building the actual five-step workflow UI that calls most
 * of them is Phase 2b's job (`App.tsx` only proves `getAppInfo` round-trips
 * end to end for now).
 */
import { commands } from "./generated-types";

export const {
  getAppInfo,
  getEngineInfo,
  getEngineCapabilities,
  selectInputFiles,
  selectInputDirectory,
  selectOutputDirectory,
  revealPath,
  openPath,
  inspectInputs,
  scanInputDirectory,
  validateJob,
  compileJobPreview,
  startJob,
  cancelJob,
  getJob,
  listRecentJobs,
  deleteJobHistory,
  exportJobManifest,
  getSettings,
  updateSettings,
  clearLogs,
} = commands;

export type {
  AppInfoDto,
  ArtifactKind,
  BrokenOutput,
  CleanupOptions,
  ClearLogsResultDto,
  CommandPreviewDto,
  ConflictPolicy,
  CriteriaFilePreviewDto,
  DirectoryScanDto,
  DuplicateOutput,
  DuplicatePolicy,
  EcoOptions,
  EngineCapabilities,
  EngineIdentity,
  ErrorCode,
  ErrorRecord,
  FenPatternFilter,
  FilterPlan,
  InputFile,
  InputInspectionDto,
  JobAcceptedDto,
  JobEvent,
  JobMode,
  JobRecordDto,
  JobResult,
  JobSpec,
  JobStage,
  JobStatus,
  JobSummaryDto,
  JobWarning,
  LogLevel,
  MoveBounds,
  OperationPlan,
  OutputArtifact,
  OutputNotation,
  OutputPlan,
  PlannedArtifactDto,
  ProcessingMetrics,
  PublicError,
  RuntimeOptions,
  ScanInputDirectoryOptions,
  SettingsDto,
  SettingsPatchDto,
  SetupPolicy,
  TagName,
  TagOp,
  TagRule,
  Theme,
  UpdateCheckPolicy,
  ValidationReportDto,
  ValidationStatus,
  WarningRecord,
} from "./generated-types";
