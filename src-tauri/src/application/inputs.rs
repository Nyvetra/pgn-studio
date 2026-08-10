// SPDX-License-Identifier: GPL-3.0-or-later
//! `inspect_inputs` (design-02 §4.1): lightweight, non-blocking, per-path
//! filesystem probing for the Files screen - existence/readability/
//! extension checks and an *optional* streamed SHA-256, run on a bounded
//! pool (design-02 §2.6: "hashing in `inspect_inputs`... max 2 concurrent
//! hashers... never takes the [single-flight job] slot").
//!
//! Deliberately separate from `filesystem::validate::validate_job`: this is
//! a *display* probe (never blocks the Files screen on a slow network
//! drive for long, never errors the whole call because one file is bad),
//! not the authoritative, blocking validation pipeline `start_job`
//! re-runs internally regardless of what this reported.
//!
//! Also owns `scan_input_directory` ("Add Folder", architecture.md §13.2):
//! walks a user-picked folder (`filesystem::folder_scan`) and feeds the
//! matched paths through this same [`inspect_inputs`], so the Files screen
//! sees identical size/readability/warning data whether a file arrived via
//! "Add Files" or "Add Folder" - see [`scan_input_directory`]'s own doc
//! comment.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::Semaphore;

use crate::domain::PublicError;
use crate::errors;
use crate::filesystem::folder_scan;

use super::run_blocking;

/// Design-02 §2.6, cited verbatim in this module's doc comment.
const MAX_CONCURRENT_HASHERS: usize = 2;

/// One `inspect_inputs` result row (design-02 §4.1: "per path: { path,
/// displayName, sizeBytes, modifiedAt?, isReadable, extensionOk,
/// warnings[] } / optional sha256 only when settings.hashInputs is on").
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InputInspectionDto {
    pub path: String,
    pub display_name: String,
    pub size_bytes: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub is_readable: bool,
    pub extension_ok: bool,
    pub sha256: Option<String>,
    pub warnings: Vec<String>,
}

/// Inspects every path in `paths` concurrently (bounded to
/// [`MAX_CONCURRENT_HASHERS`] when `hash_inputs` is set), never failing the
/// whole batch because one entry is bad - problems become `warnings`/
/// `isReadable: false` on that entry alone, matching `validate_job`'s own
/// "errors block, nothing else does" split applied at the single-file
/// level.
pub async fn inspect_inputs(paths: Vec<String>, hash_inputs: bool) -> Vec<InputInspectionDto> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_HASHERS));
    let tasks: Vec<_> = paths
        .into_iter()
        .map(|raw_path| {
            let semaphore = semaphore.clone();
            tokio::spawn(inspect_one(raw_path, hash_inputs, semaphore))
        })
        .collect();

    let mut results = Vec::with_capacity(tasks.len());
    for task in tasks {
        results.push(task.await.unwrap_or_else(|_| InputInspectionDto {
            path: String::new(),
            display_name: String::new(),
            size_bytes: None,
            modified_at: None,
            is_readable: false,
            extension_ok: false,
            sha256: None,
            warnings: vec!["an internal error occurred while inspecting this file".to_string()],
        }));
    }
    results
}

// ---------------------------------------------------------------------
// scan_input_directory ("Add Folder", architecture.md §13.2)
// ---------------------------------------------------------------------

/// `scan_input_directory` request options - see
/// `filesystem::folder_scan::ScanOptions` (the internal, non-IPC-facing
/// twin this is converted into) for the full recursion-default rationale.
/// A separate wire type rather than reusing that one directly for the same
/// reason the rest of this codebase keeps `filesystem`/`engine` internals
/// off the IPC boundary (design-02's "no wire type leaks an internal-only
/// shape" convention - see `application::jobs::CommandPreviewDto`'s own
/// doc comment for the identical reasoning applied to
/// `engine::command_compiler::CompiledEngineCommand`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScanInputDirectoryOptions {
    /// Whether to descend into subfolders. The frontend must default this
    /// to `false` and surface it as an explicit "Include subfolders"
    /// control - see `filesystem::folder_scan`'s module doc comment for why
    /// non-recursive is the binding default.
    pub recursive: bool,
    /// The §11.2 "advanced override" for extensionless/non-`.pgn` files.
    pub include_all_extensions: bool,
}

/// `scan_input_directory` response: the matched files, already run through
/// the exact same [`inspect_inputs`] pipeline "Add Files" uses (so sizes,
/// readability, and warnings are computed identically - never duplicated
/// logic), plus enough truncation/scope metadata for the Files screen to
/// show an honest "found N files..." review before anything is actually
/// added (architecture.md §13.2).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryScanDto {
    pub files: Vec<InputInspectionDto>,
    /// Echoes back what was actually used, so the UI can label the result
    /// accurately ("12 files, including subfolders") without trusting its
    /// own possibly-stale local state.
    pub recursive: bool,
    pub directories_scanned: u64,
    pub truncated: bool,
    pub truncation_notes: Vec<String>,
}

/// Scans `directory` for candidate `.pgn` inputs (architecture.md §13.2
/// "Add Folder") and inspects every match through the same
/// [`inspect_inputs`] path "Add Files" already uses, so the Files screen
/// never sees two different notions of "size"/"readable"/"warnings" for
/// the same file depending on how it was added.
///
/// The walk itself (`filesystem::folder_scan::scan_pgn_directory`) is
/// synchronous/blocking, so it runs via [`run_blocking`]
/// (architecture.md §19.4: filesystem scanning must not run on the
/// async/UI-adjacent thread) - matching `application::jobs::validate_job`'s
/// own precedent for wrapping a pure, synchronous `filesystem::` function.
pub async fn scan_input_directory(
    directory: String,
    options: ScanInputDirectoryOptions,
    hash_inputs: bool,
) -> Result<DirectoryScanDto, PublicError> {
    let root = PathBuf::from(&directory);
    let root_for_error = root.clone();
    let scan_options = folder_scan::ScanOptions {
        recursive: options.recursive,
        include_all_extensions: options.include_all_extensions,
    };

    let outcome = run_blocking(move || folder_scan::scan_pgn_directory(&root, &scan_options))
        .await?
        .map_err(|e| errors::directory_not_readable_io(&root_for_error, &e))?;

    let paths: Vec<String> = outcome
        .files
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let files = inspect_inputs(paths, hash_inputs).await;

    Ok(DirectoryScanDto {
        files,
        recursive: options.recursive,
        directories_scanned: outcome.directories_visited,
        truncated: outcome.truncated,
        truncation_notes: outcome.truncation_notes,
    })
}

async fn inspect_one(
    raw_path: String,
    hash_inputs: bool,
    semaphore: Arc<Semaphore>,
) -> InputInspectionDto {
    let path = PathBuf::from(&raw_path);
    let display_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw_path.clone());
    let mut warnings = Vec::new();

    if !path.is_absolute() {
        warnings.push("path is not absolute".to_string());
    }

    let metadata_path = path.clone();
    let metadata = tokio::task::spawn_blocking(move || std::fs::metadata(&metadata_path)).await;

    let (size_bytes, modified_at, is_readable) = match metadata {
        Ok(Ok(meta)) if meta.is_file() => {
            let modified = meta.modified().ok().map(DateTime::<Utc>::from);
            (Some(meta.len()), modified, true)
        }
        Ok(Ok(_)) => {
            warnings.push("not a regular file".to_string());
            (None, None, false)
        }
        Ok(Err(e)) => {
            warnings.push(format!(
                "could not be read ({})",
                errors::classify_io_error(&e)
            ));
            (None, None, false)
        }
        Err(_) => {
            warnings.push("an internal error occurred reading file metadata".to_string());
            (None, None, false)
        }
    };

    let extension_ok = path
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("pgn"))
        .unwrap_or(false);
    if !extension_ok {
        warnings.push("file does not have a .pgn extension".to_string());
    }

    let sha256 = if hash_inputs && is_readable {
        let _permit = semaphore.acquire_owned().await.ok();
        hash_file(&path).await
    } else {
        None
    };

    InputInspectionDto {
        path: raw_path,
        display_name,
        size_bytes,
        modified_at,
        is_readable,
        extension_ok,
        sha256,
        warnings,
    }
}

async fn hash_file(path: &Path) -> Option<String> {
    crate::engine::sidecar::hash_file_streaming(path).await.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_file_is_reported_unreadable_with_a_warning_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist.pgn");
        let results = inspect_inputs(vec![missing.to_string_lossy().into_owned()], false).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_readable);
        assert!(!results[0].warnings.is_empty());
    }

    #[tokio::test]
    async fn valid_pgn_file_is_readable_with_no_warnings_and_correct_size() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.pgn");
        std::fs::write(&path, b"[Event \"x\"]\n\n1. e4 e5 1-0\n").unwrap();
        let expected_len = std::fs::metadata(&path).unwrap().len();
        let results = inspect_inputs(vec![path.to_string_lossy().into_owned()], false).await;
        assert!(results[0].is_readable);
        assert!(results[0].extension_ok);
        assert_eq!(results[0].size_bytes, Some(expected_len));
        assert!(results[0].sha256.is_none(), "hashing was not requested");
    }

    #[tokio::test]
    async fn non_pgn_extension_is_flagged_but_still_readable() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.txt");
        std::fs::write(&path, b"not pgn").unwrap();
        let results = inspect_inputs(vec![path.to_string_lossy().into_owned()], false).await;
        assert!(results[0].is_readable);
        assert!(!results[0].extension_ok);
        assert!(results[0].warnings.iter().any(|w| w.contains(".pgn")));
    }

    #[tokio::test]
    async fn hashing_is_populated_only_when_requested() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.pgn");
        std::fs::write(&path, b"content").unwrap();
        let results = inspect_inputs(vec![path.to_string_lossy().into_owned()], true).await;
        assert!(results[0].sha256.is_some());
        assert_eq!(results[0].sha256.as_deref().unwrap().len(), 64);
    }

    #[tokio::test]
    async fn preserves_input_order_and_count_across_a_mixed_batch() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.pgn");
        let b = tmp.path().join("does-not-exist.pgn");
        let c = tmp.path().join("c.pgn");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&c, b"y").unwrap();
        let results = inspect_inputs(
            vec![
                a.to_string_lossy().into_owned(),
                b.to_string_lossy().into_owned(),
                c.to_string_lossy().into_owned(),
            ],
            false,
        )
        .await;
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].path, a.to_string_lossy());
        assert_eq!(results[1].path, b.to_string_lossy());
        assert_eq!(results[2].path, c.to_string_lossy());
        assert!(results[0].is_readable);
        assert!(!results[1].is_readable);
        assert!(results[2].is_readable);
    }

    fn scan_options(recursive: bool, include_all_extensions: bool) -> ScanInputDirectoryOptions {
        ScanInputDirectoryOptions {
            recursive,
            include_all_extensions,
        }
    }

    #[tokio::test]
    async fn scan_input_directory_reuses_inspect_inputs_for_size_and_readability() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.pgn");
        std::fs::write(&path, b"[Event \"x\"]\n\n1. e4 e5 1-0\n").unwrap();
        let expected_len = std::fs::metadata(&path).unwrap().len();

        let result = scan_input_directory(
            tmp.path().to_string_lossy().into_owned(),
            scan_options(false, false),
            false,
        )
        .await
        .unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].is_readable);
        assert_eq!(result.files[0].size_bytes, Some(expected_len));
        assert!(!result.truncated);
        assert!(!result.recursive);
    }

    #[tokio::test]
    async fn scan_input_directory_echoes_the_recursive_flag_it_actually_used() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.pgn"), b"x").unwrap();

        let non_recursive = scan_input_directory(
            tmp.path().to_string_lossy().into_owned(),
            scan_options(false, false),
            false,
        )
        .await
        .unwrap();
        assert_eq!(non_recursive.files.len(), 0);
        assert!(!non_recursive.recursive);

        let recursive = scan_input_directory(
            tmp.path().to_string_lossy().into_owned(),
            scan_options(true, false),
            false,
        )
        .await
        .unwrap();
        assert_eq!(recursive.files.len(), 1);
        assert!(recursive.recursive);
    }

    #[tokio::test]
    async fn scan_input_directory_on_a_missing_folder_is_a_reported_error_not_a_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        let err = scan_input_directory(
            missing.to_string_lossy().into_owned(),
            scan_options(false, false),
            false,
        )
        .await
        .unwrap_err();
        assert_eq!(err.code(), crate::domain::ErrorCode::InputNotReadable);
    }
}
