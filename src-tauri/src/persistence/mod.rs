// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings and job-history storage (architecture.md §15; design-02 §4.1).
//!
//! Both stores are bounded JSON on disk (architecture.md §15.2 explicitly
//! permits this for the MVP instead of SQLite) and are exposed only through
//! small traits (`SettingsStore`, `HistoryStore`) so a later phase can swap
//! in a `rusqlite`-backed implementation without touching `commands/` or
//! `application/` (task instruction: "Keep the storage layer small and
//! behind a trait so Phase 6 can swap it").
//!
//! Neither store ever writes complete PGN content (architecture.md §15.2) -
//! only paths, sizes, timestamps, and the small typed metadata already
//! defined in `domain::`/`commands::dto`.

pub mod history;
pub mod settings;

use std::io;
use std::path::Path;

use serde::Serialize;

/// Shared atomic-write helper (temp file in the same directory, then
/// rename) for the small JSON documents this module owns. Mirrors the
/// discipline `filesystem::workspace::write_final_manifest` already uses
/// for the job manifest, applied here to settings/history instead - same-
/// directory rename keeps it a metadata-only operation on every platform
/// this project supports, and a torn write can never leave a half-written
/// file at the real path (worst case: a leftover `.tmp-*` sibling, never a
/// corrupt `settings.json`/`index.json`).
pub(crate) fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("persistence path has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let tmp_path = parent.join(format!(
        ".pgnstudio-persist-tmp-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    std::fs::write(&tmp_path, &bytes)?;
    // Best-effort fsync of the temp file before the rename, matching
    // `filesystem::workspace::write_final_manifest`'s durability rationale;
    // these documents are tiny so a failed fsync is logged-away rather than
    // fatal (still followed by the rename, which is the atomicity boundary
    // that matters - a settings/history write is not a source-of-truth
    // safety guarantee the way a job manifest is).
    if let Ok(f) = std::fs::File::open(&tmp_path) {
        let _ = f.sync_all();
    }
    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_json_atomic_creates_parent_dirs_and_is_readable_back() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("doc.json");
        write_json_atomic(&path, &serde_json::json!({"a": 1})).unwrap();
        let read_back: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(read_back["a"], 1);
    }

    #[test]
    fn write_json_atomic_leaves_no_tmp_sibling_on_success() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("doc.json");
        write_json_atomic(&path, &serde_json::json!({"a": 1})).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains("pgnstudio-persist-tmp")
            })
            .collect();
        assert!(
            leftovers.is_empty(),
            "no temp file should survive a successful write"
        );
    }
}
