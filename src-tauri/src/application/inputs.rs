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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use specta::Type;
use tokio::sync::Semaphore;

use crate::errors;

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
}
