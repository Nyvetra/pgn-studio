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
}
