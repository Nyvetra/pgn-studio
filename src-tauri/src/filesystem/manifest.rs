// SPDX-License-Identifier: GPL-3.0-or-later
//! The job manifest (architecture.md §11.3, §15.3; design-02 §3.3, §3.4
//! step 7): a draft written *before* spawn (so a crash leaves an
//! enumerable record for the startup sweeper) and a final manifest written
//! *last*, after every listed artifact has been fully published ("its
//! presence implies all listed artifacts were fully published").
//!
//! Two distinct types rather than one `Option`-heavy struct, because they
//! have genuinely different read/write directions:
//! - [`DraftManifest`] is written before spawn and **read back** by the
//!   startup sweeper ([`super::workspace::sweep_interrupted_workspaces`]) -
//!   it derives `Deserialize`.
//! - [`FinalManifest`] is written once, last, and nothing in this codebase
//!   reads it back (that is a future history/persistence feature) - it
//!   derives `Serialize` only. It therefore embeds [`WarningRecord`]/
//!   [`ErrorRecord`] rather than [`crate::domain::JobWarning`]/
//!   [`crate::domain::PublicError`] directly: those two domain types are
//!   deliberately **not** `Deserialize` (see `domain::result`'s doc
//!   comments - it closes a redaction-bypass hole), and a manifest record
//!   type is a reasonable, narrow place to convert out of them via their
//!   public accessors rather than force-fitting non-`Deserialize` types
//!   into a struct that would otherwise want to be round-trippable.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::domain::{
    EngineIdentity, ErrorCode, JobSpec, JobWarning, OutputArtifact, ProcessingMetrics, PublicError,
};

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CriteriaFileRecord {
    pub relative_path: String,
    pub sha256: String,
}

/// Written to `<ws>/manifest.draft.json` before the engine is spawned
/// (design-02 §3.3). Only `.pgnstudio-tmp-*`-named paths belong in
/// `temp_outputs` - "nothing else is ever swept" (§3.3) - callers must not
/// add arbitrary paths here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftManifest {
    pub schema_version: u32,
    pub job_id: Uuid,
    pub spec: JobSpec,
    pub argv: Vec<String>,
    pub criteria_files: Vec<CriteriaFileRecord>,
    pub temp_outputs: Vec<PathBuf>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FinalStatus {
    Succeeded,
    Failed,
    Cancelled,
}

/// `Deserialize`/`Type` (Phase 2a addition): `application::jobs::get_job`
/// reads a completed job's `FinalManifest` back from
/// `<jobs_root>/<job_id>/manifest.json` to answer `get_job` for a
/// non-active job, and returns records of this exact shape to the frontend.
/// This module's own doc comment already anticipated exactly this
/// ("nothing in this codebase reads it back (that is a future
/// history/persistence feature)").
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WarningRecord {
    pub code: ErrorCode,
    pub message: String,
}

impl From<&JobWarning> for WarningRecord {
    fn from(w: &JobWarning) -> Self {
        Self {
            code: w.code(),
            message: w.message().to_string(),
        }
    }
}

/// See [`WarningRecord`]'s doc comment for why this now derives
/// `Deserialize`/`Type` too.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ErrorRecord {
    pub code: ErrorCode,
    pub title: String,
    pub message: String,
    pub remediation: Option<String>,
    pub technical_id: Uuid,
}

impl From<&PublicError> for ErrorRecord {
    fn from(e: &PublicError) -> Self {
        Self {
            code: e.code(),
            title: e.title().to_string(),
            message: e.message().to_string(),
            remediation: e.remediation().map(str::to_string),
            technical_id: e.technical_id(),
        }
    }
}

/// Written to `<ws>/manifest.draft.json` then atomically promoted to
/// `<ws>/manifest.json` **last**, after every artifact in `artifacts` has
/// already been published (design-02 §3.4 step 7).
///
/// `Deserialize` (Phase 2a addition): `application::jobs::get_job` reads
/// this back to answer `get_job` for a job that is no longer active - see
/// [`WarningRecord`]'s doc comment. Never re-serialized to
/// `manifest.json` (that write path is still `write_final_manifest`'s
/// alone) - this is a read-only consumer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalManifest {
    pub schema_version: u32,
    pub job_id: Uuid,
    pub spec: JobSpec,
    pub argv: Vec<String>,
    pub criteria_files: Vec<CriteriaFileRecord>,
    pub status: FinalStatus,
    pub engine: EngineIdentity,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub artifacts: Vec<OutputArtifact>,
    pub metrics: ProcessingMetrics,
    pub warnings: Vec<WarningRecord>,
    pub error: Option<ErrorRecord>,
    /// Temp files deleted as part of failure/cancellation cleanup (design-02
    /// §2.5 step 7, §3.4 failure path).
    pub deleted_temp_files: Vec<PathBuf>,
    /// Temp files that a cleanup attempt could **not** delete - named
    /// explicitly rather than the cleanup being silently claimed as
    /// complete (architecture.md §18.3's binding honesty rule).
    pub leftover_temp_files: Vec<PathBuf>,
}

/// Parses and re-validates a previously-exported job manifest file's raw
/// bytes as **untrusted input** (architecture.md §16.1's threat model:
/// "malicious filenames or paths" - a hand-edited or foreign-tool-produced
/// file with this name must never be trusted merely because it happens to
/// parse as JSON). "Save Job" (architecture.md §13.7) writes exactly this
/// shape via `application::jobs::export_job_manifest`; this is the
/// corresponding untrusted-read side - used directly by this module's own
/// round-trip test (`filesystem::export`'s
/// `exported_manifest_round_trips_through_revalidation`), and is the seam a
/// future "load a saved job" UI would call before ever handing the
/// recovered [`JobSpec`] to `validate_job`/the Review screen (architecture.md
/// §4.4: "Users can later rerun a compatible manifest after reviewing it" -
/// "reviewing" presupposes the spec was already sanity-checked, not blindly
/// trusted).
///
/// Rejects, rather than best-effort-recovering from:
/// - malformed/non-JSON bytes, or JSON that does not match this shape;
/// - any `schemaVersion` other than [`MANIFEST_SCHEMA_VERSION`] - "reject
///   unknown schema versions" is a binding instruction, not merely "try to
///   read it anyway";
/// - an embedded `spec.schemaVersion` other than
///   [`crate::domain::CURRENT_SCHEMA_VERSION`] (defense in depth: the outer
///   manifest envelope and the inner job spec are versioned independently,
///   and both must be understood before either is trusted);
/// - a structurally empty/invalid spec (no inputs, or an empty output base
///   name).
///
/// What this function does **not** do: re-check that the spec's input/
/// output *paths* still exist or are still writable on this machine - a
/// saved file's filesystem facts cannot be known until the normal
/// `validate_job` pipeline runs again (possibly on a different machine, at
/// a later time), so this function only vouches for *shape*, never for the
/// paths still being valid.
pub fn parse_and_revalidate_exported_manifest(bytes: &[u8]) -> Result<FinalManifest, PublicError> {
    let manifest: FinalManifest = serde_json::from_slice(bytes).map_err(|_| {
        crate::errors::invalid_saved_manifest("the file is not a valid PGN Studio job file")
    })?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(crate::errors::invalid_saved_manifest(&format!(
            "unsupported job file version {} (this build understands version {MANIFEST_SCHEMA_VERSION})",
            manifest.schema_version
        )));
    }
    if manifest.spec.schema_version != crate::domain::CURRENT_SCHEMA_VERSION {
        return Err(crate::errors::invalid_saved_manifest(&format!(
            "unsupported job spec version {} (this build understands version {})",
            manifest.spec.schema_version,
            crate::domain::CURRENT_SCHEMA_VERSION
        )));
    }
    if manifest.spec.inputs.is_empty() {
        return Err(crate::errors::invalid_saved_manifest(
            "the job file has no input files",
        ));
    }
    if manifest.spec.output.base_name.trim().is_empty() {
        return Err(crate::errors::invalid_saved_manifest(
            "the job file has no output base name",
        ));
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_record_copies_code_and_message() {
        let w = crate::errors::temp_cleanup_failed(&[PathBuf::from("x")]);
        let record = WarningRecord::from(&w);
        assert_eq!(record.code, ErrorCode::TempCleanupFailed);
        assert_eq!(record.message, w.message());
    }

    #[test]
    fn error_record_copies_every_public_field() {
        let e = crate::errors::output_exists(std::path::Path::new(r"C:\dest\out.pgn"));
        let record = ErrorRecord::from(&e);
        assert_eq!(record.code, ErrorCode::OutputExists);
        assert_eq!(record.title, e.title());
        assert_eq!(record.message, e.message());
        assert_eq!(record.technical_id, e.technical_id());
    }

    /// Mirrors `filesystem::workspace::tests::sample_spec` / `filesystem::
    /// validate::tests::minimal_spec` - each test module in this crate
    /// builds its own minimal fixture rather than sharing one, which is the
    /// established local convention (see those modules).
    fn sample_manifest() -> FinalManifest {
        use crate::domain::*;
        let spec = JobSpec {
            schema_version: CURRENT_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            name: "sample".to_string(),
            inputs: vec![InputFile {
                path: PathBuf::from(r"C:\games\a.pgn"),
                display_name: "a.pgn".to_string(),
                priority: 0,
            }],
            output: OutputPlan {
                directory: PathBuf::from(r"C:\dest"),
                base_name: "out".to_string(),
                unique_games: true,
                duplicate_games: DuplicateOutput::None,
                log_file: false,
                manifest: false,
                always_create_audit: false,
                conflict_policy: ConflictPolicy::Fail,
                confirmed_replace: false,
            },
            operations: OperationPlan {
                mode: JobMode::Process,
                duplicates: DuplicatePolicy::None,
                cleanup: CleanupOptions {
                    remove_comments: false,
                    remove_variations: false,
                    remove_nags: false,
                    remove_move_numbers: false,
                    remove_results: false,
                    remove_tags: vec![],
                    reject_bad_results: false,
                    fix_result_tags: false,
                },
                broken: BrokenOutput::Discard,
                eco: EcoOptions { enabled: false },
                output_notation: OutputNotation::San,
                check_file: None,
            },
            filters: FilterPlan {
                tag_rules: vec![],
                move_bounds: None,
                checkmate_only: false,
                setup_policy: SetupPolicy::Any,
                fen_pattern: None,
                textual_variations: vec![],
                advanced_args: vec![],
            },
            runtime: RuntimeOptions {
                use_external_duplicate_table: false,
                count_output_games: true,
            },
        };
        FinalManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            job_id: spec.id,
            spec,
            argv: vec!["-s".to_string()],
            criteria_files: vec![],
            status: FinalStatus::Succeeded,
            engine: EngineIdentity {
                version: "v26-06".to_string(),
                sha256: "a".repeat(64),
                target_triple: "x86_64-pc-windows-msvc".to_string(),
            },
            started_at: Utc::now(),
            finished_at: Utc::now(),
            artifacts: vec![OutputArtifact {
                kind: ArtifactKind::UniqueGames,
                path: PathBuf::from(r"C:\dest\out.pgn"),
                size_bytes: 123,
            }],
            metrics: ProcessingMetrics {
                input_files: 1,
                input_bytes: 10,
                processed_games: Some(1),
                input_games: Some(1),
                output_games: Some(1),
                duplicate_games: Some(0),
                broken_games: None,
                output_bytes: Some(10),
            },
            warnings: vec![],
            error: None,
            deleted_temp_files: vec![],
            leftover_temp_files: vec![],
        }
    }

    #[test]
    fn parse_and_revalidate_accepts_a_well_formed_manifest() {
        let manifest = sample_manifest();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let parsed = parse_and_revalidate_exported_manifest(&bytes).unwrap();
        assert_eq!(parsed.job_id, manifest.job_id);
        assert_eq!(parsed.spec, manifest.spec);
    }

    #[test]
    fn parse_and_revalidate_rejects_malformed_json() {
        let err = parse_and_revalidate_exported_manifest(b"{ not json").unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidJobSpec);
    }

    #[test]
    fn parse_and_revalidate_rejects_an_unknown_manifest_schema_version() {
        let mut value = serde_json::to_value(sample_manifest()).unwrap();
        value["schemaVersion"] = serde_json::json!(999);
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = parse_and_revalidate_exported_manifest(&bytes).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidJobSpec);
        assert!(err.message().contains("999"));
    }

    #[test]
    fn parse_and_revalidate_rejects_an_unknown_embedded_spec_schema_version() {
        let mut value = serde_json::to_value(sample_manifest()).unwrap();
        value["spec"]["schemaVersion"] = serde_json::json!(999);
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = parse_and_revalidate_exported_manifest(&bytes).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidJobSpec);
    }

    #[test]
    fn parse_and_revalidate_rejects_a_spec_with_no_inputs() {
        let mut value = serde_json::to_value(sample_manifest()).unwrap();
        value["spec"]["inputs"] = serde_json::json!([]);
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = parse_and_revalidate_exported_manifest(&bytes).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidJobSpec);
    }

    #[test]
    fn parse_and_revalidate_rejects_an_empty_base_name() {
        let mut value = serde_json::to_value(sample_manifest()).unwrap();
        value["spec"]["output"]["baseName"] = serde_json::json!("");
        let bytes = serde_json::to_vec(&value).unwrap();
        let err = parse_and_revalidate_exported_manifest(&bytes).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidJobSpec);
    }

    /// "Save Job" end-to-end (architecture.md §13.7): the exact bytes
    /// `export_job_manifest` would write via `filesystem::export::
    /// write_export_file_atomically`, read back off disk and re-validated
    /// as untrusted input, round-trip to an equivalent, fully re-runnable
    /// `JobSpec` - proving the export format and the re-validation function
    /// actually agree with each other, not just that each looks right in
    /// isolation.
    #[test]
    fn exported_manifest_round_trips_through_write_and_revalidation() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = sample_manifest();
        let bytes = serde_json::to_vec_pretty(&manifest).unwrap();

        let destination = tmp.path().join("saved.pgnstudio-job.json");
        crate::filesystem::export::write_export_file_atomically(&destination, &bytes).unwrap();

        let read_back = std::fs::read(&destination).unwrap();
        let revalidated = parse_and_revalidate_exported_manifest(&read_back).unwrap();

        assert_eq!(revalidated.job_id, manifest.job_id);
        assert_eq!(revalidated.spec, manifest.spec);
        assert_eq!(revalidated.argv, manifest.argv);
        assert_eq!(revalidated.engine, manifest.engine);
        assert_eq!(revalidated.artifacts, manifest.artifacts);
    }

    /// The untrusted-input rule applies just as much to a file that was
    /// genuinely produced by this app's own export path and then tampered
    /// with afterward - re-validation must not special-case "but I wrote
    /// this myself a moment ago".
    #[test]
    fn a_tampered_schema_version_survives_the_write_step_but_is_rejected_on_reread() {
        let tmp = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(sample_manifest()).unwrap();
        value["schemaVersion"] = serde_json::json!(999);
        let bytes = serde_json::to_vec(&value).unwrap();

        let destination = tmp.path().join("tampered.pgnstudio-job.json");
        crate::filesystem::export::write_export_file_atomically(&destination, &bytes).unwrap();

        let read_back = std::fs::read(&destination).unwrap();
        let err = parse_and_revalidate_exported_manifest(&read_back).unwrap_err();
        assert_eq!(err.code(), ErrorCode::InvalidJobSpec);
    }
}
