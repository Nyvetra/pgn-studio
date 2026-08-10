// SPDX-License-Identifier: GPL-3.0-or-later
//! Job-lifecycle orchestration for the `commands::jobs` handlers
//! (`validate_job`, `compile_job_preview`, `start_job`, `cancel_job`,
//! `get_job`, `list_recent_jobs`, `delete_job_history`, `export_job_manifest`
//! - design-02 §4.1, architecture.md §13.7).
//!
//! This is the "business logic" `commands/` itself must not contain
//! (`commands/README.md`): every function here composes the already-tested
//! Phase 1a/1b engine/filesystem/jobs primitives, and does not reimplement
//! any of them.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::domain::{ArtifactKind, JobSpec, OutputArtifact, ProcessingMetrics, PublicError};
use crate::engine::command_compiler::{
    compile, CompileError, CompileLayout, CompiledEngineCommand,
};
use crate::errors;
use crate::filesystem::manifest::{ErrorRecord, FinalManifest, FinalStatus, WarningRecord};
use crate::filesystem::validate::{validate_job as run_validate_job, ValidationLayout};
use crate::filesystem::workspace::workspace_root_for;
use crate::jobs::RunJobContext;
use crate::persistence::history::{HistoryEntryInput, JobSummaryDto};

use super::context::AppContext;
use super::events::{FirstEventSignal, TauriJobEventSink};
use super::run_blocking;

// ---------------------------------------------------------------------
// DTOs owned by this module (design-02 §4.1; see the crate's Phase 2a
// report for why these are new shapes rather than reusing an
// engine/filesystem-internal type verbatim).
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ValidationStatus {
    Ready,
    Invalid,
}

/// `validate_job` response. Adds `advisories` beyond design-02 §4.1's
/// literal `{ status, errors, warnings, estimatedInputBytes, freeDiskBytes? }`
/// sketch: `filesystem::validate::ValidationOutcome` intentionally carries
/// free-text advisories (e.g. "these two inputs are the same file") that
/// design-02's own DTO comment omits - dropping real signal would violate
/// this project's "never silently drop signal" posture, the same posture
/// `validate.rs`'s own doc comment invokes to justify the advisories
/// existing at all. Flagged in this crate's Phase 2a report as a deliberate,
/// justified deviation from the literal design-02 signature.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReportDto {
    pub status: ValidationStatus,
    pub errors: Vec<PublicError>,
    pub warnings: Vec<crate::domain::JobWarning>,
    pub advisories: Vec<String>,
    pub estimated_input_bytes: u64,
    pub free_disk_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CriteriaFilePreviewDto {
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlannedArtifactDto {
    pub kind: ArtifactKind,
    pub final_path: PathBuf,
    /// `None` for artifact kinds the pure compiler cannot predict a temp
    /// path for (`LogText`/`ReportJson`/`ReportText`) - their temp paths are
    /// only chosen at run time inside `jobs::run`'s
    /// `build_artifacts_to_publish`, not by `command_compiler::compile`.
    pub temporary_path: Option<PathBuf>,
}

/// `compile_job_preview` response (design-02 §4.1). A pure presentation
/// shape for the Review screen - deliberately not
/// `engine::command_compiler::CompiledEngineCommand` itself, which carries
/// OS-native `OsString` argv and an `EngineExecutable` newtype that must
/// never reach the frontend (design-02 §1.6's never-executed guarantee: no
/// code outside the engine module should even be able to *name* the
/// sidecar path).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandPreviewDto {
    pub display_command: String,
    pub argv: Vec<String>,
    pub criteria_files: Vec<CriteriaFilePreviewDto>,
    pub planned_artifacts: Vec<PlannedArtifactDto>,
}

/// `start_job` response (design-02 §4.1: `{ jobId, startedAt }`).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JobAcceptedDto {
    pub job_id: Uuid,
    pub started_at: DateTime<Utc>,
}

/// `get_job` response: a live snapshot of the currently `Running`/
/// `Cancelling` job, a terminal record reconstructed from the persisted
/// workspace manifest, or (if the manifest itself is gone) the bounded
/// history summary alone - see [`get_job`]'s doc comment for the exact
/// fallback order. Fields absent for the reporting job's current state are
/// `None` rather than fabricated (never a fake `EngineIdentity`/full metrics
/// row for a summary-only fallback with none recorded).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JobRecordDto {
    pub job_id: Uuid,
    pub name: String,
    pub status: crate::domain::JobStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub elapsed_ms: Option<u64>,
    pub engine_version: Option<String>,
    pub artifacts: Vec<OutputArtifact>,
    pub metrics: Option<ProcessingMetrics>,
    pub warnings: Vec<WarningRecord>,
    pub error: Option<ErrorRecord>,
}

// ---------------------------------------------------------------------
// validate_job / compile_job_preview
// ---------------------------------------------------------------------

pub async fn validate_job(
    ctx: &AppContext,
    spec: JobSpec,
) -> Result<ValidationReportDto, PublicError> {
    let bundle = ctx.engine_bundle()?;
    let engine = bundle.executable.clone();
    let caps = bundle.capabilities.clone();
    let workspace_root = workspace_root_for(&ctx.jobs_root, spec.id);
    let eco_file = ctx.eco_file.clone();

    let outcome = run_blocking(move || {
        let layout = ValidationLayout {
            engine,
            workspace_root,
            eco_file,
        };
        run_validate_job(&spec, &caps, &layout)
    })
    .await?;

    let status = if outcome.is_ready() {
        ValidationStatus::Ready
    } else {
        ValidationStatus::Invalid
    };
    Ok(ValidationReportDto {
        status,
        errors: outcome.errors,
        warnings: outcome.warnings,
        advisories: outcome.advisories,
        estimated_input_bytes: outcome.estimated_input_bytes,
        free_disk_bytes: outcome.free_disk_bytes,
    })
}

fn compile_error_to_public(e: CompileError) -> PublicError {
    match e {
        CompileError::InvalidSpec { field, reason } => errors::invalid_job_spec(&field, &reason),
        CompileError::UnsupportedOption { option, reason } => {
            errors::unsupported_engine_option(option, &reason)
        }
    }
}

fn to_command_preview_dto(compiled: CompiledEngineCommand) -> CommandPreviewDto {
    let argv = compiled
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let criteria_files = compiled
        .generated_files
        .iter()
        .map(|g| CriteriaFilePreviewDto {
            relative_path: g.relative_path.to_string(),
            content: g.content.clone(),
        })
        .collect();
    let planned_artifacts = compiled
        .final_outputs
        .iter()
        .map(|final_output| {
            let temporary_path = compiled
                .temporary_outputs
                .iter()
                .find(|t| t.kind == final_output.kind)
                .map(|t| t.path.clone());
            PlannedArtifactDto {
                kind: final_output.kind,
                final_path: final_output.path.clone(),
                temporary_path,
            }
        })
        .collect();
    CommandPreviewDto {
        display_command: compiled.display_command,
        argv,
        criteria_files,
        planned_artifacts,
    }
}

/// `compile_job_preview` (design-02 §4.1): pure - writes nothing to disk
/// (no workspace directory is created, no criteria file is written; only
/// the destination directory is *read* via `canonicalize`, which every
/// caller in this codebase already treats as read-only path resolution,
/// not a write).
pub async fn compile_job_preview(
    ctx: &AppContext,
    spec: JobSpec,
) -> Result<CommandPreviewDto, PublicError> {
    let bundle = ctx.engine_bundle()?;
    let engine = bundle.executable.clone();
    let caps = bundle.capabilities.clone();
    let workspace_root = workspace_root_for(&ctx.jobs_root, spec.id);
    let eco_file = ctx.eco_file.clone();
    let raw_destination_dir = spec.output.directory.clone();

    run_blocking(move || {
        let destination_dir = std::fs::canonicalize(&raw_destination_dir)
            .map_err(|e| errors::output_not_writable_io(&raw_destination_dir, &e))?;
        let layout = CompileLayout {
            engine,
            workspace_root,
            eco_file,
            destination_dir,
        };
        compile(&spec, &caps, &layout)
            .map(to_command_preview_dto)
            .map_err(compile_error_to_public)
    })
    .await?
}

// ---------------------------------------------------------------------
// start_job / cancel_job
// ---------------------------------------------------------------------

/// `start_job` (design-02 §4.1/§2.1): spawns `jobs::run_job` as a
/// background task and resolves only once the job has genuinely entered
/// `Running` (or definitely failed to) - never once the whole run finishes.
/// Design-02 §4.2's correlation rule requires listeners to be registered
/// "before `start_job` is invoked (no gap)"; the frontend can only do that
/// correctly if `start_job`'s promise does not resolve until the backend
/// has actually committed to running this job id.
pub async fn start_job(app: AppHandle, spec: JobSpec) -> Result<JobAcceptedDto, PublicError> {
    let ctx = app.state::<AppContext>();
    let bundle = ctx.engine_bundle()?;
    let engine = bundle.executable.clone();
    let caps = bundle.capabilities.clone();
    let jobs_root = ctx.jobs_root.clone();
    let eco_file = ctx.eco_file.clone();

    // Fast pre-check: good UX (an immediate, specific error instead of a
    // job that silently never starts), not the authoritative guard -
    // `jobs::run_job`'s own internal `AppState::try_acquire` is what
    // actually prevents two engines from ever running at once (design-02
    // §2.6).
    if let Some(running) = ctx.jobs.active_job_id() {
        return Err(errors::job_already_running(running));
    }

    let job_id = spec.id;
    let name = spec.name.clone();
    let input_paths: Vec<PathBuf> = spec.inputs.iter().map(|i| i.path.clone()).collect();

    let (sink, accepted_rx) = FirstEventSignal::new(TauriJobEventSink::new(
        app.clone(),
        job_id,
        name.clone(),
        input_paths.clone(),
    ));

    let app_for_task = app.clone();
    tokio::spawn(async move {
        let run_ctx = RunJobContext {
            caps: &caps,
            engine: &engine,
            jobs_root: &jobs_root,
            eco_file: &eco_file,
        };
        let ctx = app_for_task.state::<AppContext>();
        // `run_job`'s `Err` path is exactly `JOB_ALREADY_RUNNING` (see its
        // own doc comment) and never calls `sink` at all - there is nothing
        // further to do here for that case; `start_job`'s caller observes
        // it through `accepted_rx` resolving to `Err` below.
        if let Ok(job_result) = crate::jobs::run_job(spec, &run_ctx, &ctx.jobs, &sink).await {
            record_history(&ctx, &job_result, &name, &input_paths);
        }
    });

    match accepted_rx.await {
        Ok(()) => Ok(JobAcceptedDto {
            job_id,
            started_at: Utc::now(),
        }),
        Err(_) => {
            let ctx = app.state::<AppContext>();
            Err(errors::job_already_running(
                ctx.jobs.active_job_id().unwrap_or(job_id),
            ))
        }
    }
}

fn record_history(
    ctx: &AppContext,
    job_result: &crate::domain::JobResult,
    name: &str,
    input_paths: &[PathBuf],
) {
    let max_entries = ctx.settings.load().max_recent_jobs;
    let summary = JobSummaryDto {
        job_id: job_result.job_id,
        name: name.to_string(),
        status: job_result.status,
        started_at: job_result.started_at,
        finished_at: Some(job_result.finished_at),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        engine_version: job_result.engine.version.clone(),
        error_code: job_result.error.as_ref().map(|e| e.code()),
    };
    let artifact_paths = job_result
        .artifacts
        .iter()
        .map(|a| a.path.clone())
        .collect();
    let evicted = ctx.history.record_completed(
        HistoryEntryInput {
            summary,
            input_paths: input_paths.to_vec(),
            artifact_paths,
        },
        max_entries,
    );
    // Design-02 §3.3: workspace retention is bounded by the same setting -
    // an evicted history entry's workspace (criteria files + log +
    // manifest; never the published destination artifacts, which live
    // outside `jobs_root` entirely) is deleted alongside it.
    for evicted_id in evicted {
        let _ = std::fs::remove_dir_all(ctx.jobs_root.join(evicted_id.to_string()));
    }
}

pub fn cancel_job(ctx: &AppContext, job_id: Uuid) -> Result<(), PublicError> {
    ctx.jobs.request_cancel(job_id)
}

// ---------------------------------------------------------------------
// get_job / list_recent_jobs / delete_job_history
// ---------------------------------------------------------------------

fn final_status_to_job_status(status: FinalStatus) -> crate::domain::JobStatus {
    match status {
        FinalStatus::Succeeded => crate::domain::JobStatus::Succeeded,
        FinalStatus::Failed => crate::domain::JobStatus::Failed,
        FinalStatus::Cancelled => crate::domain::JobStatus::Cancelled,
    }
}

fn live_job_record(ctx: &AppContext, job_id: Uuid) -> Option<JobRecordDto> {
    let guard = ctx.live_job.lock().unwrap_or_else(|p| p.into_inner());
    let live = guard.as_ref()?;
    if live.job_id != job_id {
        return None;
    }
    Some(JobRecordDto {
        job_id: live.job_id,
        name: live.name.clone(),
        status: live.status,
        started_at: live.started_at,
        finished_at: None,
        elapsed_ms: None,
        engine_version: ctx
            .engine_bundle()
            .ok()
            .map(|b| b.capabilities.identity.version.clone()),
        artifacts: live.artifacts.clone(),
        metrics: Some(live.metrics),
        warnings: live.warnings.iter().map(WarningRecord::from).collect(),
        error: None,
    })
}

/// Reads and parses `<jobs_root>/<job_id>/manifest.json` if present, `None`
/// otherwise (unknown id, job never reached a terminal state, or the
/// workspace was already evicted by history retention - see
/// [`record_history`]). Shared by [`manifest_record`] (the `get_job`
/// fallback) and [`export_job_manifest`] ("Save Job", architecture.md
/// §13.7) so both read the exact same on-disk record rather than two
/// slightly different reimplementations.
async fn read_final_manifest(ctx: &AppContext, job_id: Uuid) -> Option<FinalManifest> {
    let manifest_path = workspace_root_for(&ctx.jobs_root, job_id).join("manifest.json");
    run_blocking(move || {
        let raw = std::fs::read_to_string(&manifest_path).ok()?;
        serde_json::from_str::<FinalManifest>(&raw).ok()
    })
    .await
    .ok()
    .flatten()
}

async fn manifest_record(ctx: &AppContext, job_id: Uuid) -> Option<JobRecordDto> {
    let manifest = read_final_manifest(ctx, job_id).await?;

    let elapsed_ms = (manifest.finished_at - manifest.started_at)
        .num_milliseconds()
        .max(0) as u64;
    Some(JobRecordDto {
        job_id: manifest.job_id,
        name: manifest.spec.name,
        status: final_status_to_job_status(manifest.status),
        started_at: manifest.started_at,
        finished_at: Some(manifest.finished_at),
        elapsed_ms: Some(elapsed_ms),
        engine_version: Some(manifest.engine.version),
        artifacts: manifest.artifacts,
        metrics: Some(manifest.metrics),
        warnings: manifest.warnings,
        error: manifest.error,
    })
}

fn summary_only_record(summary: JobSummaryDto) -> JobRecordDto {
    JobRecordDto {
        job_id: summary.job_id,
        name: summary.name,
        status: summary.status,
        started_at: summary.started_at,
        finished_at: summary.finished_at,
        elapsed_ms: None,
        engine_version: Some(summary.engine_version),
        artifacts: Vec::new(),
        metrics: None,
        warnings: Vec::new(),
        error: None,
    }
}

/// `get_job` (design-02 §4.1: "history or active snapshot"; §4.2:
/// "`job://completed` is also mirrored by `get_job` for reconciliation
/// after frontend reloads"). Answers, in order of preference: (1) the live
/// in-memory snapshot if `job_id` is the currently `Running`/`Cancelling`
/// job; (2) the persisted workspace `manifest.json` (richest terminal
/// record, written last by `jobs::run::finalize` - design-02 §3.4 step 7);
/// (3) the bounded history summary alone, if the manifest is missing/
/// corrupt/already cleaned up but the index still remembers the job.
pub async fn get_job(ctx: &AppContext, job_id: Uuid) -> Result<JobRecordDto, PublicError> {
    if let Some(dto) = live_job_record(ctx, job_id) {
        return Ok(dto);
    }
    if let Some(dto) = manifest_record(ctx, job_id).await {
        return Ok(dto);
    }
    if let Some(summary) = ctx.history.get_summary(job_id) {
        return Ok(summary_only_record(summary));
    }
    Err(errors::invalid_job_spec(
        "jobId",
        "no job with this id was found in history or the active run",
    ))
}

pub fn list_recent_jobs(ctx: &AppContext, limit: u32) -> Vec<JobSummaryDto> {
    ctx.history.list_recent(limit)
}

/// `delete_job_history` (design-02 §4.1: "history + workspace; never
/// artifacts"). Removes the bounded history entry and the job's workspace
/// directory (`<jobs_root>/<job_id>/` - criteria files, log, manifest);
/// never touches the published destination artifacts, which live entirely
/// outside `jobs_root`.
pub fn delete_job_history(ctx: &AppContext, job_id: Uuid) -> Result<(), PublicError> {
    let existed_in_history = ctx.history.delete(job_id);
    let workspace_dir = ctx.jobs_root.join(job_id.to_string());
    let workspace_existed = workspace_dir.exists();
    if workspace_existed {
        let _ = std::fs::remove_dir_all(&workspace_dir);
    }
    if !existed_in_history && !workspace_existed {
        return Err(errors::invalid_job_spec(
            "jobId",
            "no job with this id was found in history",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// export_job_manifest ("Save Job", architecture.md §13.7)
// ---------------------------------------------------------------------

/// "Save Job" (architecture.md §13.7, §4.4, §15.3): exports a completed
/// job's full reproducible manifest - schema version, app/engine identity,
/// inputs, options, sanitized argv, criteria-file hashes, timestamps,
/// artifacts, metrics, warnings/error (§15.3's complete checklist, already
/// exactly what [`FinalManifest`] carries - see that type's own doc
/// comment) - to a user-chosen file via the native save dialog. Returns the
/// chosen path, or `None` if the user cancelled the dialog.
///
/// The dialog is called directly here rather than from `commands::dialogs`
/// because, unlike that module's pure pickers, this handler is
/// fundamentally a *job* operation (it reads the job's own manifest and
/// validates the destination against the job's own paths) - keeping it
/// alongside the other `commands::jobs` handlers matches how [`get_job`]
/// already reads this exact manifest file.
///
/// The destination is validated in Rust exactly like any other output path
/// (`filesystem::export::validate_export_destination` - parent-directory
/// writability, reserved device names, and non-aliasing against this job's
/// own input/artifact paths) before anything is written; the frontend never
/// writes files directly (architecture.md §16.2).
pub async fn export_job_manifest(
    app: &AppHandle,
    ctx: &AppContext,
    job_id: Uuid,
) -> Result<Option<PathBuf>, PublicError> {
    let manifest = read_final_manifest(ctx, job_id).await.ok_or_else(|| {
        errors::invalid_job_spec("jobId", "no completed job manifest was found for this id")
    })?;

    // `blocking_save_file` is the dialog plugin's own documented pattern
    // for an `async fn` Tauri command - it blocks a tokio worker thread,
    // never the main/UI thread (see `commands::dialogs`'s module doc
    // comment for the identical reasoning already established for
    // `blocking_pick_files`/`blocking_pick_folder`).
    let suggested_name = format!("{}.pgnstudio-job.json", manifest.spec.output.base_name);
    let picked = app
        .dialog()
        .file()
        .add_filter("PGN Studio job", &["json"])
        .set_file_name(&suggested_name)
        .blocking_save_file();
    let Some(destination) = picked.and_then(|fp| fp.simplified().into_path().ok()) else {
        return Ok(None); // user cancelled the dialog
    };

    let mut protected: Vec<PathBuf> = manifest
        .spec
        .inputs
        .iter()
        .map(|i| i.path.clone())
        .collect();
    protected.extend(manifest.artifacts.iter().map(|a| a.path.clone()));
    crate::filesystem::export::validate_export_destination(&destination, &protected)?;

    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| {
        #[allow(deprecated)]
        errors::unknown_internal_error(anyhow::anyhow!("serializing job manifest for export: {e}"))
    })?;

    let destination_for_write = destination.clone();
    run_blocking(move || {
        crate::filesystem::export::write_export_file_atomically(&destination_for_write, &bytes)
    })
    .await?
    .map_err(|e| errors::output_not_writable_io(&destination, &e))?;

    Ok(Some(destination))
}
