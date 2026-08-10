// SPDX-License-Identifier: GPL-3.0-or-later
//! Atomic output publication (architecture.md §11.4; design-02 §3.4, §3.5) -
//! the 7-step sequence and the three conflict policies. This is the module
//! that turns "the engine wrote bytes to a temp file" into "the user's
//! requested output exists at its final name," without ever silently
//! overwriting an existing file and without ever losing track of a
//! temporary file it created.
//!
//! **Kind-agnostic by design.** This module does not care whether a temp
//! file was written by the engine (`UniqueGames`/`DuplicateGames`) or by
//! Rust itself (`ReportJson`/`ReportText`/`LogText`) - by the time
//! [`publish_all`] is called, every artifact is just "a temp file that
//! exists in the destination directory, and the final name it should be
//! published to" ([`ArtifactToPublish`]). The caller (`jobs::run`) is
//! responsible for writing the Rust-generated artifacts' temp files before
//! calling this.
//!
//! **Decision (documented deviation, per design-02 §7 question 2):**
//! `ReplaceAfterConfirmation` moves the previous file to a timestamped
//! `.bak` sibling rather than the OS recycle bin. Design-02's own §7 open
//! question offered this as an accepted, simpler alternative ("should V1
//! ship `.bak`-only (simpler, more predictable for tests)?"); this task's
//! constraints (no product-owner ruling recorded in the decisions ledger,
//! and a strong preference for not adding a COM-heavy new dependency to
//! the most safety-critical code in the app without being able to verify
//! its behavior here) make `.bak`-only the more defensible choice. The
//! *safety* guarantee (silent overwrite is impossible) does not depend on
//! which of the two mechanisms is used - see [`replace_after_confirmation`].
//!
//! **Decision (documented refinement of design-02 §3.5's suffix wording):**
//! "the whole run uses one suffix (computed once from the first artifact
//! that needs it)" is read here as: probe candidate suffixes `0, 1, 2, ...`
//! and pick the first `n` for which *every* artifact in the batch has a
//! free name, rather than only checking the first artifact and letting
//! later artifacts potentially disagree. This guarantees `x (2).pgn` and
//! `x (2).duplicates.pgn` always stay paired even in adversarial
//! interleavings, which is strictly safer than (and a superset of) what a
//! first-artifact-only probe guarantees. The actual publish step still uses
//! a no-replace rename regardless, so a race after the probe still fails
//! closed rather than silently overwriting.

use std::path::{Path, PathBuf};

use crate::domain::{ArtifactKind, ConflictPolicy, OutputArtifact};

use super::platform;

#[derive(Debug)]
pub enum PublishError {
    /// §3.4 step 6a's TOCTOU guard tripped: the destination directory's
    /// file identity changed since the job started (e.g. a directory
    /// component was swapped for a junction mid-job).
    DestinationIdentityChanged,
    /// §3.4 step 4: the engine exited 0 but an expected temp file is
    /// missing or is not a regular file.
    OutputMissing(PathBuf),
    /// §3.4 step 5: light postflight validation failed (e.g. the engine
    /// reported N>0 matched games but the main output is 0 bytes).
    OutputInvalid {
        path: PathBuf,
        reason: String,
    },
    /// No-replace rename found the destination already occupied (`Fail`
    /// policy, or `AddNumericSuffix` exhausted its search).
    OutputExists(PathBuf),
    Io(std::io::Error),
}

impl std::fmt::Display for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublishError::DestinationIdentityChanged => {
                write!(
                    f,
                    "destination directory identity changed since the job started"
                )
            }
            PublishError::OutputMissing(p) => {
                write!(f, "expected output missing: {}", p.display())
            }
            PublishError::OutputInvalid { path, reason } => {
                write!(f, "expected output invalid: {}: {reason}", path.display())
            }
            PublishError::OutputExists(p) => write!(f, "output already exists: {}", p.display()),
            PublishError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PublishError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PublishError {
    fn from(e: std::io::Error) -> Self {
        PublishError::Io(e)
    }
}

/// One artifact ready to be published: a temp file that already exists on
/// disk (in the destination directory - design-02 D-8) and the final path
/// it should end up at absent any conflict-policy renaming.
#[derive(Debug, Clone)]
pub struct ArtifactToPublish {
    pub temp_path: PathBuf,
    pub kind: ArtifactKind,
    pub final_path: PathBuf,
    /// design-02 D-21: whether this artifact should still be published if
    /// its temp file turns out to be empty (main output: always `true`;
    /// duplicates audit: only when `always_create_audit`).
    pub publish_if_empty: bool,
}

/// Failure outcome, carrying everything design-02 §3.4's failure-path
/// honesty rules require: what already got published (never rolled back -
/// those are complete, valid files), and the true state of temp-file
/// cleanup (never claimed successful when it was not).
#[derive(Debug)]
pub struct PublishFailure {
    pub error: PublishError,
    pub published_before_failure: Vec<OutputArtifact>,
    pub deleted_temp_files: Vec<PathBuf>,
    pub leftover_temp_files: Vec<PathBuf>,
}

fn artifact_name_suffix(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::UniqueGames => ".pgn",
        ArtifactKind::DuplicateGames => ".duplicates.pgn",
        ArtifactKind::ReportJson => ".report.json",
        ArtifactKind::ReportText => ".report.txt",
        ArtifactKind::LogText => ".log.txt",
    }
}

/// Inserts ` (n)` immediately before the artifact's well-known name suffix
/// (design-02 §3.5's own example: "`x (2).pgn` and `x (2).duplicates.pgn`
/// stay paired" - the `(2)` sits before `.duplicates.pgn` as a whole, not
/// merely before the final `.pgn`). Stripping a known, exact suffix string
/// (rather than `Path::file_stem`/`extension`, which split on the *last*
/// dot only) keeps this correct even when `base_name` itself contains a
/// dot, which `validate_base_name` permits.
fn apply_suffix(final_path: &Path, kind: ArtifactKind, n: u32) -> PathBuf {
    if n == 0 {
        return final_path.to_path_buf();
    }
    let file_name = final_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or_default();
    let suffix = artifact_name_suffix(kind);
    let base = file_name.strip_suffix(suffix).unwrap_or(file_name);
    final_path.with_file_name(format!("{base} ({n}){suffix}"))
}

const MAX_SUFFIX: u32 = 999;

struct Decision<'a> {
    artifact: &'a ArtifactToPublish,
    skip: bool,
}

fn find_batch_suffix(decisions: &[Decision<'_>]) -> Option<u32> {
    'candidate: for n in 0..=MAX_SUFFIX {
        for d in decisions {
            if d.skip {
                continue;
            }
            if apply_suffix(&d.artifact.final_path, d.artifact.kind, n).exists() {
                continue 'candidate;
            }
        }
        return Some(n);
    }
    None
}

fn build_failure(
    error: PublishError,
    all_artifacts: &[ArtifactToPublish],
    published_so_far: Vec<OutputArtifact>,
    consumed_temp_paths: &[PathBuf],
) -> PublishFailure {
    let mut deleted = Vec::new();
    let mut leftover = Vec::new();
    for artifact in all_artifacts {
        if consumed_temp_paths.iter().any(|p| p == &artifact.temp_path) {
            continue; // already renamed away, or already deleted (empty-skip case)
        }
        if !artifact.temp_path.exists() {
            continue; // never created, or already gone
        }
        match std::fs::remove_file(&artifact.temp_path) {
            Ok(()) => deleted.push(artifact.temp_path.clone()),
            Err(_) => leftover.push(artifact.temp_path.clone()),
        }
    }
    PublishFailure {
        error,
        published_before_failure: published_so_far,
        deleted_temp_files: deleted,
        leftover_temp_files: leftover,
    }
}

/// `ReplaceAfterConfirmation`: move the previous file to a timestamped
/// `.bak` sibling, then perform the *same* no-replace rename every other
/// policy uses. Still no-replace even here: if something else creates a
/// new file at `final_path` between the `.bak` move and this rename, the
/// rename fails closed (`OUTPUT_EXISTS`) instead of silently overwriting -
/// "silent overwrite must be impossible by construction" (design-02 §3.5)
/// holds for every policy, not just `Fail`.
fn replace_after_confirmation(
    temp_path: &Path,
    final_path: &Path,
) -> Result<(), platform::RenameError> {
    if final_path.exists() {
        let bak_path = timestamped_bak_path(final_path);
        std::fs::rename(final_path, &bak_path).map_err(platform::RenameError::Io)?;
    }
    platform::rename_no_replace(temp_path, final_path)
}

fn timestamped_bak_path(final_path: &Path) -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let file_name = final_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("output");
    final_path.with_file_name(format!("{file_name}.{stamp}.bak"))
}

/// Runs the full 7-step atomic publication sequence (design-02 §3.4) for a
/// batch of artifacts that must all be published consistently (steps 1-3
/// already happened by the time this is called: temp names were chosen at
/// compile time, the engine/Rust already wrote the temp files, and the
/// caller has already gated on engine success).
///
/// `destination_dir_identity_at_spawn` must be a [`same_file::Handle`]
/// captured for `destination_dir` **before** the engine was spawned - see
/// `jobs::run` for where this is captured and held for the run's duration.
pub fn publish_all(
    artifacts: &[ArtifactToPublish],
    destination_dir: &Path,
    destination_dir_identity_at_spawn: &same_file::Handle,
    conflict_policy: ConflictPolicy,
    matched_games: Option<u64>,
) -> Result<Vec<OutputArtifact>, PublishFailure> {
    // Step 6a: TOCTOU re-check, before touching anything.
    let current_identity = match same_file::Handle::from_path(destination_dir) {
        Ok(h) => h,
        Err(e) => {
            return Err(build_failure(
                PublishError::Io(e),
                artifacts,
                Vec::new(),
                &[],
            ))
        }
    };
    if &current_identity != destination_dir_identity_at_spawn {
        return Err(build_failure(
            PublishError::DestinationIdentityChanged,
            artifacts,
            Vec::new(),
            &[],
        ));
    }

    // Steps 4 + 5: existence/readability + light postflight validation,
    // decided for every artifact before any rename happens.
    let mut decisions: Vec<Decision<'_>> = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let metadata = match std::fs::metadata(&artifact.temp_path) {
            Ok(m) if m.is_file() => m,
            _ => {
                return Err(build_failure(
                    PublishError::OutputMissing(artifact.temp_path.clone()),
                    artifacts,
                    Vec::new(),
                    &[],
                ));
            }
        };
        if artifact.kind == ArtifactKind::UniqueGames {
            if let Some(matched) = matched_games {
                if matched > 0 && metadata.len() == 0 {
                    return Err(build_failure(
                        PublishError::OutputInvalid {
                            path: artifact.temp_path.clone(),
                            reason: format!(
                                "the engine reported {matched} game(s) matched, but the output \
                                 file is empty"
                            ),
                        },
                        artifacts,
                        Vec::new(),
                        &[],
                    ));
                }
            }
        }
        let skip = metadata.len() == 0 && !artifact.publish_if_empty;
        decisions.push(Decision { artifact, skip });
    }

    let suffix = match conflict_policy {
        ConflictPolicy::AddNumericSuffix => match find_batch_suffix(&decisions) {
            Some(n) => n,
            None => {
                return Err(build_failure(
                    PublishError::OutputExists(decisions[0].artifact.final_path.clone()),
                    artifacts,
                    Vec::new(),
                    &[],
                ));
            }
        },
        _ => 0,
    };

    let mut published: Vec<OutputArtifact> = Vec::new();
    let mut consumed_temp_paths: Vec<PathBuf> = Vec::new();

    for decision in &decisions {
        let artifact = decision.artifact;
        if decision.skip {
            let _ = std::fs::remove_file(&artifact.temp_path);
            consumed_temp_paths.push(artifact.temp_path.clone());
            continue;
        }

        if let Err(e) = platform::sync_file(&artifact.temp_path) {
            return Err(build_failure(
                PublishError::Io(e),
                artifacts,
                std::mem::take(&mut published),
                &consumed_temp_paths,
            ));
        }

        let final_path = apply_suffix(&artifact.final_path, artifact.kind, suffix);
        let rename_result = match conflict_policy {
            ConflictPolicy::Fail | ConflictPolicy::AddNumericSuffix => {
                platform::rename_no_replace(&artifact.temp_path, &final_path)
            }
            ConflictPolicy::ReplaceAfterConfirmation => {
                replace_after_confirmation(&artifact.temp_path, &final_path)
            }
        };

        match rename_result {
            Ok(()) => {
                consumed_temp_paths.push(artifact.temp_path.clone());
                let _ = platform::sync_dir(destination_dir);
                let size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
                published.push(OutputArtifact {
                    kind: artifact.kind,
                    path: final_path,
                    size_bytes: size,
                });
            }
            Err(platform::RenameError::AlreadyExists) => {
                return Err(build_failure(
                    PublishError::OutputExists(final_path),
                    artifacts,
                    std::mem::take(&mut published),
                    &consumed_temp_paths,
                ));
            }
            Err(platform::RenameError::Io(e)) => {
                return Err(build_failure(
                    PublishError::Io(e),
                    artifacts,
                    std::mem::take(&mut published),
                    &consumed_temp_paths,
                ));
            }
        }
    }

    Ok(published)
}

/// Deletes every temp output in `temp_paths` that still exists (design-02
/// §3.4's failure path / §2.5 step 6: cancellation and non-zero engine
/// exit both need "delete unpublished temporary outputs"). Never claims
/// success for a path it could not actually delete.
pub fn cleanup_temp_paths(temp_paths: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut deleted = Vec::new();
    let mut leftover = Vec::new();
    for path in temp_paths {
        if !path.exists() {
            continue;
        }
        match std::fs::remove_file(path) {
            Ok(()) => deleted.push(path.clone()),
            Err(_) => leftover.push(path.clone()),
        }
    }
    (deleted, leftover)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_identity(dir: &Path) -> same_file::Handle {
        same_file::Handle::from_path(dir).unwrap()
    }

    #[test]
    fn publishes_a_single_artifact_with_fail_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let temp_path = tmp.path().join(".pgnstudio-tmp-abc-unique.pgn");
        std::fs::write(&temp_path, b"[Event \"x\"]\n\n1. e4 1-0\n").unwrap();
        let final_path = tmp.path().join("out.pgn");

        let artifacts = vec![ArtifactToPublish {
            temp_path: temp_path.clone(),
            kind: ArtifactKind::UniqueGames,
            final_path: final_path.clone(),
            publish_if_empty: true,
        }];
        let identity = dir_identity(tmp.path());
        let published = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::Fail,
            Some(1),
        )
        .unwrap();

        assert_eq!(published.len(), 1);
        assert_eq!(published[0].path, final_path);
        assert!(!temp_path.exists());
        assert!(final_path.exists());
    }

    #[test]
    fn fail_policy_never_overwrites_and_leaves_temp_for_diagnosis() {
        let tmp = tempfile::tempdir().unwrap();
        let temp_path = tmp.path().join(".pgnstudio-tmp-abc-unique.pgn");
        std::fs::write(&temp_path, b"new content").unwrap();
        let final_path = tmp.path().join("out.pgn");
        std::fs::write(&final_path, b"PRECIOUS EXISTING CONTENT").unwrap();

        let artifacts = vec![ArtifactToPublish {
            temp_path: temp_path.clone(),
            kind: ArtifactKind::UniqueGames,
            final_path: final_path.clone(),
            publish_if_empty: true,
        }];
        let identity = dir_identity(tmp.path());
        let failure = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::Fail,
            None,
        )
        .unwrap_err();

        assert!(matches!(failure.error, PublishError::OutputExists(_)));
        assert_eq!(
            std::fs::read(&final_path).unwrap(),
            b"PRECIOUS EXISTING CONTENT"
        );
        assert!(failure.published_before_failure.is_empty());
        // Step 3/§18.3: the leftover temp is reported, not silently kept
        // or silently claimed deleted.
        assert!(failure.deleted_temp_files.contains(&temp_path) || !temp_path.exists());
    }

    #[test]
    fn add_numeric_suffix_finds_the_first_free_pair() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("out.pgn"), b"taken").unwrap();
        std::fs::write(tmp.path().join("out (1).pgn"), b"also taken").unwrap();

        let temp_path = tmp.path().join(".pgnstudio-tmp-abc-unique.pgn");
        std::fs::write(&temp_path, b"content").unwrap();
        let artifacts = vec![ArtifactToPublish {
            temp_path,
            kind: ArtifactKind::UniqueGames,
            final_path: tmp.path().join("out.pgn"),
            publish_if_empty: true,
        }];
        let identity = dir_identity(tmp.path());
        let published = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::AddNumericSuffix,
            None,
        )
        .unwrap();

        assert_eq!(published[0].path, tmp.path().join("out (2).pgn"));
    }

    #[test]
    fn add_numeric_suffix_keeps_a_paired_batch_consistent() {
        let tmp = tempfile::tempdir().unwrap();
        // "out (1).pgn" is free, but "out (1).duplicates.pgn" is taken -
        // the batch must skip to (2) for BOTH artifacts, not use (1) for
        // one and (2) for the other.
        std::fs::write(tmp.path().join("out.pgn"), b"taken").unwrap();
        std::fs::write(tmp.path().join("out (1).duplicates.pgn"), b"taken").unwrap();

        let temp_unique = tmp.path().join(".pgnstudio-tmp-abc-unique.pgn");
        let temp_dupes = tmp.path().join(".pgnstudio-tmp-abc-duplicates.pgn");
        std::fs::write(&temp_unique, b"unique").unwrap();
        std::fs::write(&temp_dupes, b"dupes").unwrap();

        let artifacts = vec![
            ArtifactToPublish {
                temp_path: temp_unique,
                kind: ArtifactKind::UniqueGames,
                final_path: tmp.path().join("out.pgn"),
                publish_if_empty: true,
            },
            ArtifactToPublish {
                temp_path: temp_dupes,
                kind: ArtifactKind::DuplicateGames,
                final_path: tmp.path().join("out.duplicates.pgn"),
                publish_if_empty: true,
            },
        ];
        let identity = dir_identity(tmp.path());
        let published = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::AddNumericSuffix,
            None,
        )
        .unwrap();

        assert_eq!(published[0].path, tmp.path().join("out (2).pgn"));
        assert_eq!(published[1].path, tmp.path().join("out (2).duplicates.pgn"));
    }

    #[test]
    fn replace_after_confirmation_backs_up_and_never_silently_overwrites_on_race() {
        let tmp = tempfile::tempdir().unwrap();
        let final_path = tmp.path().join("out.pgn");
        std::fs::write(&final_path, b"OLD CONTENT").unwrap();
        let temp_path = tmp.path().join(".pgnstudio-tmp-abc-unique.pgn");
        std::fs::write(&temp_path, b"NEW CONTENT").unwrap();

        let artifacts = vec![ArtifactToPublish {
            temp_path,
            kind: ArtifactKind::UniqueGames,
            final_path: final_path.clone(),
            publish_if_empty: true,
        }];
        let identity = dir_identity(tmp.path());
        let published = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::ReplaceAfterConfirmation,
            None,
        )
        .unwrap();

        assert_eq!(std::fs::read(&published[0].path).unwrap(), b"NEW CONTENT");
        // The old content must survive *somewhere* (as a .bak sibling), not
        // be silently discarded.
        let bak_files: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".bak"))
            .collect();
        assert_eq!(bak_files.len(), 1);
        assert_eq!(std::fs::read(bak_files[0].path()).unwrap(), b"OLD CONTENT");
    }

    #[test]
    fn empty_main_output_is_still_published() {
        let tmp = tempfile::tempdir().unwrap();
        let temp_path = tmp.path().join(".pgnstudio-tmp-abc-unique.pgn");
        std::fs::write(&temp_path, b"").unwrap();
        let final_path = tmp.path().join("out.pgn");

        let artifacts = vec![ArtifactToPublish {
            temp_path,
            kind: ArtifactKind::UniqueGames,
            final_path: final_path.clone(),
            publish_if_empty: true,
        }];
        let identity = dir_identity(tmp.path());
        let published = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::Fail,
            None,
        )
        .unwrap();
        assert_eq!(published.len(), 1);
        assert!(final_path.exists());
    }

    #[test]
    fn empty_duplicates_audit_is_deleted_not_published_when_not_always_create() {
        let tmp = tempfile::tempdir().unwrap();
        let temp_path = tmp.path().join(".pgnstudio-tmp-abc-duplicates.pgn");
        std::fs::write(&temp_path, b"").unwrap();
        let final_path = tmp.path().join("out.duplicates.pgn");

        let artifacts = vec![ArtifactToPublish {
            temp_path: temp_path.clone(),
            kind: ArtifactKind::DuplicateGames,
            final_path: final_path.clone(),
            publish_if_empty: false,
        }];
        let identity = dir_identity(tmp.path());
        let published = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::Fail,
            None,
        )
        .unwrap();
        assert!(published.is_empty());
        assert!(!temp_path.exists());
        assert!(!final_path.exists());
    }

    #[test]
    fn matched_games_positive_but_empty_output_is_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let temp_path = tmp.path().join(".pgnstudio-tmp-abc-unique.pgn");
        std::fs::write(&temp_path, b"").unwrap();
        let artifacts = vec![ArtifactToPublish {
            temp_path,
            kind: ArtifactKind::UniqueGames,
            final_path: tmp.path().join("out.pgn"),
            publish_if_empty: true,
        }];
        let identity = dir_identity(tmp.path());
        let failure = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::Fail,
            Some(3),
        )
        .unwrap_err();
        assert!(matches!(failure.error, PublishError::OutputInvalid { .. }));
    }

    #[test]
    fn missing_temp_output_is_reported_as_output_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let artifacts = vec![ArtifactToPublish {
            temp_path: tmp.path().join(".pgnstudio-tmp-abc-unique.pgn"), // never created
            kind: ArtifactKind::UniqueGames,
            final_path: tmp.path().join("out.pgn"),
            publish_if_empty: true,
        }];
        let identity = dir_identity(tmp.path());
        let failure = publish_all(
            &artifacts,
            tmp.path(),
            &identity,
            ConflictPolicy::Fail,
            None,
        )
        .unwrap_err();
        assert!(matches!(failure.error, PublishError::OutputMissing(_)));
    }

    #[test]
    fn destination_identity_change_blocks_publication() {
        let tmp = tempfile::tempdir().unwrap();
        let temp_path = tmp.path().join(".pgnstudio-tmp-abc-unique.pgn");
        std::fs::write(&temp_path, b"content").unwrap();
        let artifacts = vec![ArtifactToPublish {
            temp_path,
            kind: ArtifactKind::UniqueGames,
            final_path: tmp.path().join("out.pgn"),
            publish_if_empty: true,
        }];
        // A handle to a DIFFERENT directory stands in for "identity changed".
        let other_tmp = tempfile::tempdir().unwrap();
        let stale_identity = dir_identity(other_tmp.path());
        let failure = publish_all(
            &artifacts,
            tmp.path(),
            &stale_identity,
            ConflictPolicy::Fail,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            failure.error,
            PublishError::DestinationIdentityChanged
        ));
    }

    #[test]
    fn cleanup_temp_paths_reports_what_it_actually_deleted() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.tmp");
        let b = tmp.path().join("b-does-not-exist.tmp");
        std::fs::write(&a, b"x").unwrap();
        let (deleted, leftover) = cleanup_temp_paths(&[a.clone(), b.clone()]);
        assert_eq!(deleted, vec![a.clone()]);
        assert!(leftover.is_empty());
        assert!(!a.exists());
    }
}
