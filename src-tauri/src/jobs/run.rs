// SPDX-License-Identifier: GPL-3.0-or-later
//! Top-level job orchestration (architecture.md §9.1, §10.3; design-02 §2):
//! [`run_job`] turns a [`JobSpec`] into a [`JobResult`], covering every step
//! architecture.md §10.3 lists for the (Phase 2) `start_job` command to
//! delegate to: re-validate, canonicalize, compile, create the workspace,
//! write criteria files, spawn, stream, watch for cancellation, validate
//! artifacts, publish atomically, and write the final manifest.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{
    ArtifactKind, EngineCapabilities, EngineIdentity, JobResult, JobSpec, JobStatus, JobWarning,
    OutputArtifact, ProcessingMetrics, PublicError,
};
use crate::engine::command_compiler::{
    compile, CompileError, CompileLayout, CompiledEngineCommand,
};
use crate::engine::EngineExecutable;
use crate::errors;
use crate::filesystem::{
    self,
    manifest::{
        CriteriaFileRecord, DraftManifest, ErrorRecord, FinalManifest, FinalStatus, WarningRecord,
        MANIFEST_SCHEMA_VERSION,
    },
    publish::{self, ArtifactToPublish},
    validate::{validate_job, ValidationLayout},
    workspace::{
        create_job_workspace, workspace_root_for, write_draft_manifest, write_final_manifest,
        JobWorkspace,
    },
};

use super::events::{JobEventSink, JobStage};
use super::process::{run_engine, EngineRunResult};
use super::AppState;

/// Everything [`run_job`] needs beyond the spec itself, gathered in one
/// place so its own signature stays readable. `jobs_root` is
/// `<app-cache>/jobs/`; `eco_file` is the bundled, verified
/// `resources/pgn-extract/eco.pgn` (absolute path).
pub struct RunJobContext<'a> {
    pub caps: &'a EngineCapabilities,
    pub engine: &'a EngineExecutable,
    pub jobs_root: &'a Path,
    pub eco_file: &'a Path,
}

/// Runs one job end to end (design-02 §2.1's `Ready -> Running -> {Succeeded,
/// Failed, Cancelled}`).
///
/// Returns `Err` **only** for the one failure mode that means the job never
/// entered `Running` at all: the single-flight slot was already held
/// (`JOB_ALREADY_RUNNING`, design-02 §2.6). Every other failure - a
/// re-validation error, a compile error, a spawn failure, a nonzero engine
/// exit, a publication failure - happens *after* `Running` begins, so it is
/// reported as `Ok(JobResult { status: Failed, error: Some(..), .. })`
/// rather than as an `Err`, matching design-02 §2.1's state table (only
/// `Running -> Failed`/`Cancelled` are terminal-with-detail transitions;
/// nothing transitions "back out of" Running).
pub async fn run_job(
    spec: JobSpec,
    ctx: &RunJobContext<'_>,
    state: &AppState,
    sink: &dyn JobEventSink,
) -> Result<JobResult, PublicError> {
    let job_id = spec.id;
    let started_at = Utc::now();
    let seq = AtomicU64::new(0);
    let engine_identity = ctx.caps.identity.clone();

    let guard = state.try_acquire(job_id)?;
    sink.state(seq.fetch_add(1, Ordering::SeqCst), JobStatus::Running);
    sink.stage(
        seq.fetch_add(1, Ordering::SeqCst),
        JobStage::Preparing,
        "Preparing workspace",
    );

    macro_rules! fail_before_workspace {
        ($error:expr) => {
            return Ok(finalize(
                None,
                job_id,
                &spec,
                Vec::new(),
                Vec::new(),
                started_at,
                engine_identity,
                FinalStatus::Failed,
                Vec::new(),
                ProcessingMetrics {
                    input_files: spec.inputs.len() as u64,
                    input_bytes: 0,
                    processed_games: None,
                    input_games: None,
                    output_games: None,
                    duplicate_games: None,
                    broken_games: None,
                    output_bytes: None,
                },
                Vec::new(),
                Some($error),
                Vec::new(),
                Vec::new(),
                sink,
                &seq,
            ))
        };
    }

    // Re-validate (design-02 §2.1: "Ready is a UI gate, not a trusted
    // precondition" - `Running` always re-validates internally).
    let workspace_root = workspace_root_for(ctx.jobs_root, job_id);
    let validation_layout = ValidationLayout {
        engine: ctx.engine.clone(),
        workspace_root: workspace_root.clone(),
        eco_file: ctx.eco_file.to_path_buf(),
    };
    let outcome = validate_job(&spec, ctx.caps, &validation_layout);
    if !outcome.is_ready() {
        let error = outcome
            .errors
            .into_iter()
            .next()
            .expect("is_ready() false implies at least one error");
        fail_before_workspace!(error);
    }
    let input_bytes = outcome.estimated_input_bytes;

    let destination_dir = match std::fs::canonicalize(&spec.output.directory) {
        Ok(d) => d,
        Err(e) => {
            fail_before_workspace!(errors::output_not_writable_io(&spec.output.directory, &e))
        }
    };

    // Re-compile (same rationale).
    let compile_layout = CompileLayout {
        engine: ctx.engine.clone(),
        workspace_root: workspace_root.clone(),
        eco_file: ctx.eco_file.to_path_buf(),
        destination_dir: destination_dir.clone(),
    };
    let compiled = match compile(&spec, ctx.caps, &compile_layout) {
        Ok(c) => c,
        Err(CompileError::InvalidSpec { field, reason }) => {
            fail_before_workspace!(errors::invalid_job_spec(&field, &reason))
        }
        Err(CompileError::UnsupportedOption { option, reason }) => {
            fail_before_workspace!(errors::unsupported_engine_option(option, &reason))
        }
    };

    // TOCTOU baseline for publication (design-02 §3.4 step 6a): captured
    // now, before spawn, held for the run's duration.
    let destination_dir_identity = match same_file::Handle::from_path(&destination_dir) {
        Ok(h) => h,
        Err(e) => fail_before_workspace!(errors::output_not_writable_io(&destination_dir, &e)),
    };

    // Workspace + criteria files + draft manifest, all BEFORE spawn
    // (design-02 §3.3).
    let workspace = match create_job_workspace(ctx.jobs_root, job_id) {
        Ok(w) => w,
        Err(e) => fail_before_workspace!(errors::engine_start_failed_io(&e)),
    };
    for generated in &compiled.generated_files {
        if let Err(e) = std::fs::write(
            workspace.root().join(generated.relative_path),
            &generated.content,
        ) {
            let leftovers = temp_output_paths(&compiled);
            return Ok(finalize(
                Some(&workspace),
                job_id,
                &spec,
                Vec::new(),
                Vec::new(),
                started_at,
                engine_identity,
                FinalStatus::Failed,
                Vec::new(),
                empty_metrics(&spec, input_bytes),
                Vec::new(),
                Some(errors::engine_start_failed_io(&e)),
                Vec::new(),
                leftovers,
                sink,
                &seq,
            ));
        }
    }

    let argv_record: Vec<String> = compiled
        .args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let criteria_records: Vec<CriteriaFileRecord> = compiled
        .generated_files
        .iter()
        .map(|g| CriteriaFileRecord {
            relative_path: g.relative_path.to_string(),
            sha256: g.sha256.clone(),
        })
        .collect();
    let mut warnings: Vec<JobWarning> = Vec::new();
    let draft = DraftManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        job_id,
        spec: spec.clone(),
        argv: argv_record.clone(),
        criteria_files: criteria_records.clone(),
        temp_outputs: temp_output_paths(&compiled),
        created_at: started_at,
    };
    if let Err(e) = write_draft_manifest(&workspace, &draft) {
        // Not fatal to the run itself (design-02 §5.1: "job itself may
        // still be Succeeded"); the crash-recovery safety net for *this*
        // run is degraded, which is exactly what the warning says.
        warnings.push(errors::history_write_failed(&e));
    }

    sink.stage(
        seq.fetch_add(1, Ordering::SeqCst),
        JobStage::Starting,
        "Starting engine",
    );

    let cancel_rx = guard.cancel_receiver();
    let run_result = run_engine(
        &compiled,
        &workspace.engine_log_path(),
        cancel_rx,
        sink,
        &seq,
        empty_metrics(&spec, input_bytes),
    )
    .await;

    let run_result = match run_result {
        Ok(r) => r,
        Err(e) => {
            let leftovers = temp_output_paths(&compiled);
            let (deleted, leftover) = publish::cleanup_temp_paths(&leftovers);
            return Ok(finalize(
                Some(&workspace),
                job_id,
                &spec,
                argv_record,
                criteria_records,
                started_at,
                engine_identity,
                FinalStatus::Failed,
                Vec::new(),
                empty_metrics(&spec, input_bytes),
                warnings,
                Some(errors::engine_start_failed_io(&e)),
                deleted,
                leftover,
                sink,
                &seq,
            ));
        }
    };

    match run_result {
        EngineRunResult::Cancelled { last_progress } => {
            let mut leftovers = temp_output_paths(&compiled);
            leftovers.push(workspace.virtual_tmp_path());
            let (deleted, leftover) = publish::cleanup_temp_paths(&leftovers);
            let metrics = ProcessingMetrics {
                input_files: spec.inputs.len() as u64,
                input_bytes,
                processed_games: last_progress,
                input_games: None,
                output_games: None,
                duplicate_games: None,
                broken_games: None,
                output_bytes: None,
            };
            Ok(finalize(
                Some(&workspace),
                job_id,
                &spec,
                argv_record,
                criteria_records,
                started_at,
                engine_identity,
                FinalStatus::Cancelled,
                Vec::new(),
                metrics,
                warnings,
                Some(errors::job_cancelled()),
                deleted,
                leftover,
                sink,
                &seq,
            ))
        }
        EngineRunResult::Completed {
            exit_code,
            final_summary,
            last_progress,
        } => {
            if exit_code != Some(0) {
                let leftovers = temp_output_paths(&compiled);
                let (deleted, leftover) = publish::cleanup_temp_paths(&leftovers);
                let stderr_tail = read_log_tail(&workspace.engine_log_path(), 20);
                let metrics = ProcessingMetrics {
                    input_files: spec.inputs.len() as u64,
                    input_bytes,
                    processed_games: last_progress,
                    input_games: None,
                    output_games: None,
                    duplicate_games: None,
                    broken_games: None,
                    output_bytes: None,
                };
                return Ok(finalize(
                    Some(&workspace),
                    job_id,
                    &spec,
                    argv_record,
                    criteria_records,
                    started_at,
                    engine_identity,
                    FinalStatus::Failed,
                    Vec::new(),
                    metrics,
                    warnings,
                    Some(errors::engine_exit_nonzero(
                        exit_code,
                        &workspace.engine_log_path(),
                        &stderr_tail,
                    )),
                    deleted,
                    leftover,
                    sink,
                    &seq,
                ));
            }

            sink.stage(
                seq.fetch_add(1, Ordering::SeqCst),
                JobStage::Finalizing,
                "Publishing outputs",
            );

            let matched_games = final_summary.map(|(matched, _)| matched);
            let (artifacts_to_publish, synth_errors) =
                match build_artifacts_to_publish(&compiled, &workspace, &spec, &destination_dir) {
                    Ok(v) => (v, None),
                    Err(e) => (Vec::new(), Some(e)),
                };
            if let Some(e) = synth_errors {
                let leftovers = temp_output_paths(&compiled);
                let (deleted, leftover) = publish::cleanup_temp_paths(&leftovers);
                let metrics = ProcessingMetrics {
                    input_files: spec.inputs.len() as u64,
                    input_bytes,
                    processed_games: last_progress,
                    input_games: final_summary.map(|(_, total)| total),
                    output_games: None,
                    duplicate_games: None,
                    broken_games: None,
                    output_bytes: None,
                };
                return Ok(finalize(
                    Some(&workspace),
                    job_id,
                    &spec,
                    argv_record,
                    criteria_records,
                    started_at,
                    engine_identity,
                    FinalStatus::Failed,
                    Vec::new(),
                    metrics,
                    warnings,
                    Some(e),
                    deleted,
                    leftover,
                    sink,
                    &seq,
                ));
            }

            let publish_outcome = publish::publish_all(
                &artifacts_to_publish,
                &destination_dir,
                &destination_dir_identity,
                spec.output.conflict_policy,
                matched_games,
            );

            match publish_outcome {
                Ok(published) => {
                    let metrics = compute_final_metrics(
                        &compiled,
                        &spec,
                        input_bytes,
                        last_progress,
                        final_summary,
                        &published,
                    );
                    Ok(finalize(
                        Some(&workspace),
                        job_id,
                        &spec,
                        argv_record,
                        criteria_records,
                        started_at,
                        engine_identity,
                        FinalStatus::Succeeded,
                        published,
                        metrics,
                        warnings,
                        None,
                        Vec::new(),
                        Vec::new(),
                        sink,
                        &seq,
                    ))
                }
                Err(failure) => {
                    let metrics = ProcessingMetrics {
                        input_files: spec.inputs.len() as u64,
                        input_bytes,
                        processed_games: last_progress,
                        input_games: final_summary.map(|(_, total)| total),
                        output_games: None,
                        duplicate_games: None,
                        broken_games: None,
                        output_bytes: None,
                    };
                    let error = publish_error_to_public(&failure.error);
                    Ok(finalize(
                        Some(&workspace),
                        job_id,
                        &spec,
                        argv_record,
                        criteria_records,
                        started_at,
                        engine_identity,
                        FinalStatus::Failed,
                        failure.published_before_failure,
                        metrics,
                        warnings,
                        Some(error),
                        failure.deleted_temp_files,
                        failure.leftover_temp_files,
                        sink,
                        &seq,
                    ))
                }
            }
        }
    }
}

fn empty_metrics(spec: &JobSpec, input_bytes: u64) -> ProcessingMetrics {
    ProcessingMetrics {
        input_files: spec.inputs.len() as u64,
        input_bytes,
        processed_games: None,
        input_games: None,
        output_games: None,
        duplicate_games: None,
        broken_games: None,
        output_bytes: None,
    }
}

fn temp_output_paths(compiled: &CompiledEngineCommand) -> Vec<PathBuf> {
    compiled
        .temporary_outputs
        .iter()
        .map(|t| t.path.clone())
        .collect()
}

fn publish_error_to_public(error: &publish::PublishError) -> PublicError {
    match error {
        publish::PublishError::DestinationIdentityChanged => errors::output_not_writable_io(
            Path::new(""),
            &std::io::Error::other("destination directory identity changed mid-job"),
        ),
        publish::PublishError::OutputMissing(p) => errors::engine_output_missing(p, Path::new("")),
        publish::PublishError::OutputInvalid { path, reason } => {
            errors::engine_output_invalid(path, reason, Path::new(""))
        }
        publish::PublishError::OutputExists(p) => errors::output_exists(p),
        publish::PublishError::Io(e) => errors::engine_start_failed_io(e),
    }
}

/// Builds the `.pgnstudio-tmp-*`-named artifact list [`publish::publish_all`]
/// needs, mapping the engine-produced temp outputs to their planned final
/// paths and, for the Rust-generated artifact kinds (`LogText`/
/// `ReportJson`/`ReportText`), writing their temp content first.
fn build_artifacts_to_publish(
    compiled: &CompiledEngineCommand,
    workspace: &JobWorkspace,
    spec: &JobSpec,
    destination_dir: &Path,
) -> Result<Vec<ArtifactToPublish>, PublicError> {
    let mut result = Vec::new();
    let id_prefix = &spec.id.simple().to_string()[..12];

    for final_output in &compiled.final_outputs {
        match final_output.kind {
            ArtifactKind::UniqueGames | ArtifactKind::DuplicateGames => {
                if let Some(temp) = compiled
                    .temporary_outputs
                    .iter()
                    .find(|t| t.kind == final_output.kind)
                {
                    result.push(ArtifactToPublish {
                        temp_path: temp.path.clone(),
                        kind: final_output.kind,
                        final_path: final_output.path.clone(),
                        publish_if_empty: final_output.publish_if_empty,
                    });
                }
            }
            ArtifactKind::LogText => {
                let temp_path =
                    destination_dir.join(format!(".pgnstudio-tmp-{id_prefix}-logcopy.txt"));
                std::fs::copy(workspace.engine_log_path(), &temp_path)
                    .map_err(|e| errors::engine_start_failed_io(&e))?;
                result.push(ArtifactToPublish {
                    temp_path,
                    kind: ArtifactKind::LogText,
                    final_path: final_output.path.clone(),
                    publish_if_empty: final_output.publish_if_empty,
                });
            }
            ArtifactKind::ReportJson | ArtifactKind::ReportText => {
                // Rendered once both kinds are requested together; skip a
                // second render if ReportJson already produced both (they
                // are always requested as a pair - see
                // `command_compiler::compile`'s `OutputPlan.manifest`
                // handling - but this stays defensive rather than assuming
                // it).
                let temp_path = destination_dir.join(format!(
                    ".pgnstudio-tmp-{id_prefix}-{}",
                    match final_output.kind {
                        ArtifactKind::ReportJson => "report-json",
                        _ => "report-text",
                    }
                ));
                let content = render_minimal_report(spec, final_output.kind);
                std::fs::write(&temp_path, content)
                    .map_err(|e| errors::engine_start_failed_io(&e))?;
                result.push(ArtifactToPublish {
                    temp_path,
                    kind: final_output.kind,
                    final_path: final_output.path.clone(),
                    publish_if_empty: final_output.publish_if_empty,
                });
            }
        }
    }
    Ok(result)
}

/// A deliberately minimal report body. Rich, human-friendly report
/// formatting is `reporting/`'s eventual job (architecture.md §4.4, §15.3);
/// this exists so that `output.manifest: true` produces a real, honest
/// (if plain) artifact today rather than an empty/missing one.
fn render_minimal_report(spec: &JobSpec, kind: ArtifactKind) -> String {
    match kind {
        ArtifactKind::ReportJson => serde_json::json!({
            "jobId": spec.id,
            "name": spec.name,
            "baseName": spec.output.base_name,
        })
        .to_string(),
        _ => format!(
            "PGN Studio job report\nJob: {}\nOutput base name: {}\n",
            spec.name, spec.output.base_name
        ),
    }
}

fn compute_final_metrics(
    compiled: &CompiledEngineCommand,
    spec: &JobSpec,
    input_bytes: u64,
    last_progress: Option<u64>,
    final_summary: Option<(u64, u64)>,
    published: &[OutputArtifact],
) -> ProcessingMetrics {
    let plan = &compiled.metrics_plan;
    let input_games = final_summary.map(|(_, total)| total);
    let broken_games = if plan.broken_games {
        final_summary.map(|(matched, total)| total.saturating_sub(matched))
    } else {
        None
    };
    let output_games = if plan.output_games {
        published
            .iter()
            .find(|a| a.kind == ArtifactKind::UniqueGames)
            .and_then(|a| filesystem::count_games_in_file(&a.path).ok())
    } else {
        None
    };
    let duplicate_games = if plan.duplicate_games {
        published
            .iter()
            .find(|a| a.kind == ArtifactKind::DuplicateGames)
            .and_then(|a| filesystem::count_games_in_file(&a.path).ok())
    } else {
        None
    };
    let output_bytes = if plan.output_bytes {
        Some(published.iter().map(|a| a.size_bytes).sum())
    } else {
        None
    };
    ProcessingMetrics {
        input_files: spec.inputs.len() as u64,
        input_bytes,
        processed_games: last_progress,
        input_games,
        output_games,
        duplicate_games,
        broken_games,
        output_bytes,
    }
}

fn read_log_tail(log_path: &Path, max_lines: usize) -> Vec<String> {
    let content = match std::fs::read_to_string(log_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let all: Vec<String> = content.lines().map(str::to_string).collect();
    let start = all.len().saturating_sub(max_lines);
    all[start..].to_vec()
}

#[allow(clippy::too_many_arguments)]
fn finalize(
    workspace: Option<&JobWorkspace>,
    job_id: Uuid,
    spec: &JobSpec,
    argv: Vec<String>,
    criteria_files: Vec<CriteriaFileRecord>,
    started_at: DateTime<Utc>,
    engine_identity: EngineIdentity,
    status: FinalStatus,
    artifacts: Vec<OutputArtifact>,
    metrics: ProcessingMetrics,
    mut warnings: Vec<JobWarning>,
    error: Option<PublicError>,
    deleted_temp_files: Vec<PathBuf>,
    leftover_temp_files: Vec<PathBuf>,
    sink: &dyn JobEventSink,
    seq: &AtomicU64,
) -> JobResult {
    let finished_at = Utc::now();
    let elapsed_ms = (finished_at - started_at).num_milliseconds().max(0) as u64;

    if !leftover_temp_files.is_empty() {
        warnings.push(errors::temp_cleanup_failed(&leftover_temp_files));
    }

    if let Some(ws) = workspace {
        let manifest = FinalManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            job_id,
            spec: spec.clone(),
            argv,
            criteria_files,
            status,
            engine: engine_identity.clone(),
            started_at,
            finished_at,
            artifacts: artifacts.clone(),
            metrics,
            warnings: warnings.iter().map(WarningRecord::from).collect(),
            error: error.as_ref().map(ErrorRecord::from),
            deleted_temp_files,
            leftover_temp_files,
        };
        // A manifest-write failure here is itself surfaced only via the
        // local tracing log, never as a second layer of warning-about-a-
        // warning - `HISTORY_WRITE_FAILED` already covers the draft path;
        // by the time we reach here the job's real outcome is fixed and
        // must still be returned to the caller regardless.
        if let Err(e) = write_final_manifest(ws, &manifest) {
            errors::log_technical_detail(
                Uuid::new_v4(),
                crate::domain::ErrorCode::HistoryWriteFailed,
                "writing final manifest",
                &e,
            );
        }
    }

    let job_status = match status {
        FinalStatus::Succeeded => JobStatus::Succeeded,
        FinalStatus::Failed => JobStatus::Failed,
        FinalStatus::Cancelled => JobStatus::Cancelled,
    };
    for artifact in &artifacts {
        sink.artifact(seq.fetch_add(1, Ordering::SeqCst), artifact);
    }
    let result = JobResult {
        job_id,
        status: job_status,
        started_at,
        finished_at,
        elapsed_ms,
        engine: engine_identity,
        artifacts,
        metrics,
        warnings,
        error,
    };
    sink.state(seq.fetch_add(1, Ordering::SeqCst), job_status);
    sink.completed(seq.fetch_add(1, Ordering::SeqCst), &result);
    result
}
