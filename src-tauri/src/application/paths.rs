// SPDX-License-Identifier: GPL-3.0-or-later
//! Path allowlisting for `reveal_path`/`open_path` (design-02 §4.1: "only
//! paths that appear in job history or artifacts (allowlist check) — never
//! arbitrary user-supplied paths").

use std::path::{Path, PathBuf};

use crate::domain::PublicError;
use crate::errors;
use crate::filesystem::identity;

use super::context::AppContext;

/// Resolves `requested` against `ctx`'s allowlist (persisted history's
/// input/artifact paths, plus the active job's own inputs/artifacts-so-far,
/// see `AppContext::known_paths`), using file-identity comparison rather
/// than string equality for the same reason `filesystem::identity` uses it
/// everywhere else in this codebase (design-02 §3.1): a case-different or
/// 8.3-short-name spelling of an allowed path must not slip past a naive
/// string check in either direction (falsely rejected *or* falsely
/// accepted).
///
/// Returns the recorded allowed path on success - never the caller-supplied
/// string verbatim, since `identity::is_same_file` only establishes
/// *whether* two paths name the same file, not which spelling to trust; the
/// path this crate itself recorded is what gets handed to the dialog/opener
/// plugin.
pub fn resolve_allowed_path(ctx: &AppContext, requested: &Path) -> Result<PathBuf, PublicError> {
    for known in ctx.known_paths() {
        if identity::is_same_file(requested, &known).unwrap_or(false) {
            return Ok(known);
        }
    }
    Err(errors::path_not_allowed(requested))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::context::LiveJobSnapshot;
    use crate::domain::{ArtifactKind, JobStatus, OutputArtifact};
    use crate::persistence::history::{HistoryEntryInput, JobSummaryDto};
    use crate::persistence::settings::JsonSettingsStore;
    use uuid::Uuid;

    fn test_context(tmp: &std::path::Path) -> AppContext {
        AppContext::new(
            Err(crate::errors::engine_missing(std::path::Path::new(
                "nowhere",
            ))),
            tmp.join("jobs"),
            tmp.join("eco.pgn"),
            Box::new(JsonSettingsStore::load_or_default(
                tmp.join("settings.json"),
            )),
            Box::new(
                crate::persistence::history::JsonHistoryStore::load_or_default(
                    tmp.join("history.json"),
                ),
            ),
            tmp.join("logs"),
        )
    }

    #[test]
    fn rejects_a_path_not_in_history_or_live_job() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_context(tmp.path());
        let stray = tmp.path().join("not-tracked.pgn");
        std::fs::write(&stray, b"x").unwrap();
        let err = resolve_allowed_path(&ctx, &stray).unwrap_err();
        assert_eq!(err.code(), crate::domain::ErrorCode::InvalidJobSpec);
    }

    #[test]
    fn accepts_a_path_recorded_in_persisted_history() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_context(tmp.path());
        let artifact = tmp.path().join("out.pgn");
        std::fs::write(&artifact, b"x").unwrap();
        ctx.history.record_completed(
            HistoryEntryInput {
                summary: JobSummaryDto {
                    job_id: Uuid::new_v4(),
                    name: "job".to_string(),
                    status: JobStatus::Succeeded,
                    started_at: chrono::Utc::now(),
                    finished_at: Some(chrono::Utc::now()),
                    app_version: "0.1.0".to_string(),
                    engine_version: "v26-06".to_string(),
                    error_code: None,
                },
                input_paths: vec![],
                artifact_paths: vec![artifact.clone()],
            },
            50,
        );
        assert_eq!(resolve_allowed_path(&ctx, &artifact).unwrap(), artifact);
    }

    #[test]
    fn accepts_a_path_from_the_currently_active_job() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_context(tmp.path());
        let live_artifact = tmp.path().join("live-out.pgn");
        std::fs::write(&live_artifact, b"x").unwrap();
        *ctx.live_job.lock().unwrap() = Some(LiveJobSnapshot {
            job_id: Uuid::new_v4(),
            name: "running job".to_string(),
            status: JobStatus::Running,
            started_at: chrono::Utc::now(),
            metrics: crate::domain::ProcessingMetrics {
                input_files: 0,
                input_bytes: 0,
                processed_games: None,
                input_games: None,
                output_games: None,
                duplicate_games: None,
                broken_games: None,
                output_bytes: None,
            },
            artifacts: vec![OutputArtifact {
                kind: ArtifactKind::UniqueGames,
                path: live_artifact.clone(),
                size_bytes: 1,
            }],
            warnings: vec![],
            input_paths: vec![],
        });
        assert_eq!(
            resolve_allowed_path(&ctx, &live_artifact).unwrap(),
            live_artifact
        );
    }
}
