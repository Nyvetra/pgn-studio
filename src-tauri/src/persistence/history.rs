// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded job history (architecture.md §15.2; design-02 §4.1
//! `list_recent_jobs`/`get_job`/`delete_job_history`).
//!
//! architecture.md §15.2 permits either a bounded collection of JSON
//! manifests or a Rust-owned SQLite database for V1; this module is the
//! former. It deliberately does **not** duplicate full job results: the
//! authoritative per-job record is the workspace's own
//! `filesystem::manifest::FinalManifest` (`<jobs_root>/<job_id>/manifest.json`,
//! already written by `jobs::run_job`) - this store is only the bounded,
//! fast-to-scan *index* `list_recent_jobs` needs, matching
//! architecture.md §15.2's own suggested SQL schema, where the `jobs` table
//! stores a `manifest_path` pointer rather than inline content.
//!
//! `HistoryStore` is a small trait (task ask: "Keep the storage layer small
//! and behind a trait so Phase 6 can swap it") - a future SQLite
//! implementation only needs to satisfy this same interface.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::domain::{ErrorCode, JobStatus};

/// One row of `list_recent_jobs` (architecture.md §15.2's suggested `jobs`
/// table, projected to JSON; design-02 §4.1 names the command but not this
/// DTO's field list).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JobSummaryDto {
    pub job_id: Uuid,
    pub name: String,
    pub status: JobStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub app_version: String,
    pub engine_version: String,
    pub error_code: Option<ErrorCode>,
}

/// What a caller records for one completed job: the summary plus every
/// path that should become reachable through `reveal_path`/`open_path`'s
/// allowlist (task ask: "must only accept paths that appear in job history
/// or artifacts"). Kept separate from [`JobSummaryDto`] so the *wire* type
/// returned to the frontend never needs to carry a bare path list it has no
/// use for.
#[derive(Debug, Clone)]
pub struct HistoryEntryInput {
    pub summary: JobSummaryDto,
    pub input_paths: Vec<PathBuf>,
    pub artifact_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryRecord {
    summary: JobSummaryDto,
    #[serde(default)]
    input_paths: Vec<PathBuf>,
    #[serde(default)]
    artifact_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HistoryIndex {
    #[serde(default)]
    entries: Vec<HistoryRecord>,
}

pub trait HistoryStore: Send + Sync {
    /// Records a completed job, then evicts the oldest entries beyond
    /// `max_entries` (design-02 §3.3: "Workspace retention: bounded by
    /// `maxRecentJobs`... oldest evicted with their logs"). Returns the ids
    /// of any evicted jobs so the caller can delete their now-orphaned
    /// workspaces (this store only owns the index, never a workspace
    /// directory).
    fn record_completed(&self, entry: HistoryEntryInput, max_entries: u32) -> Vec<Uuid>;

    /// Most-recently-started first, capped at `limit`.
    fn list_recent(&self, limit: u32) -> Vec<JobSummaryDto>;

    fn get_summary(&self, job_id: Uuid) -> Option<JobSummaryDto>;

    /// Removes `job_id` from the index. Returns `true` if it was present.
    /// Never touches the workspace directory or published artifacts - see
    /// this trait's own doc comment and design-02 §4.1's `delete_job_history`
    /// contract ("history + workspace; never artifacts"); workspace deletion
    /// is the caller's (`application::jobs`) responsibility.
    fn delete(&self, job_id: Uuid) -> bool;

    /// The union of every input and artifact path recorded across the
    /// whole (bounded) history - the allowlist `reveal_path`/`open_path`
    /// check against.
    fn known_paths(&self) -> Vec<PathBuf>;
}

pub struct JsonHistoryStore {
    path: PathBuf,
    cache: RwLock<HistoryIndex>,
}

fn read_index(path: &Path) -> HistoryIndex {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HistoryIndex::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

impl JsonHistoryStore {
    pub fn load_or_default(path: PathBuf) -> Self {
        let initial = read_index(&path);
        Self {
            path,
            cache: RwLock::new(initial),
        }
    }

    fn persist(&self, index: &HistoryIndex) {
        // A failed history write is degraded crash-recovery/UX, never a
        // reason to fail the job that already completed successfully
        // (design-02 §5.1 HISTORY_WRITE_FAILED: "job itself may still be
        // Succeeded"). Full detail goes to the local log; there is no
        // per-job `PublicError`/`JobWarning` seam left to attach to by the
        // time this runs (the job already finished), so this is the
        // farthest-downstream point that can still record it.
        if let Err(e) = super::write_json_atomic(&self.path, index) {
            crate::errors::log_technical_detail(
                Uuid::new_v4(),
                ErrorCode::HistoryWriteFailed,
                "writing job history index",
                &e,
            );
        }
    }
}

impl HistoryStore for JsonHistoryStore {
    fn record_completed(&self, entry: HistoryEntryInput, max_entries: u32) -> Vec<Uuid> {
        let mut guard = self.cache.write().unwrap_or_else(|p| p.into_inner());
        guard
            .entries
            .retain(|r| r.summary.job_id != entry.summary.job_id);
        guard.entries.push(HistoryRecord {
            summary: entry.summary,
            input_paths: entry.input_paths,
            artifact_paths: entry.artifact_paths,
        });
        guard
            .entries
            .sort_by_key(|r| std::cmp::Reverse(r.summary.started_at));
        let max_entries = max_entries.max(1) as usize;
        let mut evicted = Vec::new();
        while guard.entries.len() > max_entries {
            if let Some(removed) = guard.entries.pop() {
                evicted.push(removed.summary.job_id);
            }
        }
        self.persist(&guard);
        evicted
    }

    fn list_recent(&self, limit: u32) -> Vec<JobSummaryDto> {
        let guard = self.cache.read().unwrap_or_else(|p| p.into_inner());
        guard
            .entries
            .iter()
            .take(limit as usize)
            .map(|r| r.summary.clone())
            .collect()
    }

    fn get_summary(&self, job_id: Uuid) -> Option<JobSummaryDto> {
        let guard = self.cache.read().unwrap_or_else(|p| p.into_inner());
        guard
            .entries
            .iter()
            .find(|r| r.summary.job_id == job_id)
            .map(|r| r.summary.clone())
    }

    fn delete(&self, job_id: Uuid) -> bool {
        let mut guard = self.cache.write().unwrap_or_else(|p| p.into_inner());
        let before = guard.entries.len();
        guard.entries.retain(|r| r.summary.job_id != job_id);
        let removed = guard.entries.len() != before;
        if removed {
            self.persist(&guard);
        }
        removed
    }

    fn known_paths(&self) -> Vec<PathBuf> {
        let guard = self.cache.read().unwrap_or_else(|p| p.into_inner());
        let mut set: HashSet<PathBuf> = HashSet::new();
        for record in &guard.entries {
            set.extend(record.input_paths.iter().cloned());
            set.extend(record.artifact_paths.iter().cloned());
        }
        set.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(job_id: Uuid, started_at: DateTime<Utc>) -> JobSummaryDto {
        JobSummaryDto {
            job_id,
            name: "test job".to_string(),
            status: JobStatus::Succeeded,
            started_at,
            finished_at: Some(started_at),
            app_version: "0.1.0".to_string(),
            engine_version: "v26-06".to_string(),
            error_code: None,
        }
    }

    fn entry(job_id: Uuid, started_at: DateTime<Utc>) -> HistoryEntryInput {
        HistoryEntryInput {
            summary: summary(job_id, started_at),
            input_paths: vec![PathBuf::from(format!(r"C:\in\{job_id}.pgn"))],
            artifact_paths: vec![PathBuf::from(format!(r"C:\out\{job_id}.pgn"))],
        }
    }

    #[test]
    fn record_then_list_recent_returns_newest_first() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonHistoryStore::load_or_default(tmp.path().join("history.json"));
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let t0 = Utc::now();
        store.record_completed(entry(a, t0), 50);
        store.record_completed(entry(b, t0 + chrono::Duration::seconds(1)), 50);
        let recent = store.list_recent(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].job_id, b, "most recently started job comes first");
        assert_eq!(recent[1].job_id, a);
    }

    #[test]
    fn eviction_beyond_max_entries_returns_the_evicted_id_and_drops_it() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonHistoryStore::load_or_default(tmp.path().join("history.json"));
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let t0 = Utc::now();
        store.record_completed(entry(a, t0), 1);
        let evicted = store.record_completed(entry(b, t0 + chrono::Duration::seconds(1)), 1);
        assert_eq!(evicted, vec![a]);
        assert_eq!(store.list_recent(50).len(), 1);
        assert!(store.get_summary(a).is_none());
        assert!(store.get_summary(b).is_some());
    }

    #[test]
    fn recording_the_same_job_id_again_replaces_rather_than_duplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonHistoryStore::load_or_default(tmp.path().join("history.json"));
        let a = Uuid::new_v4();
        let t0 = Utc::now();
        store.record_completed(entry(a, t0), 50);
        let mut second = entry(a, t0);
        second.summary.status = JobStatus::Failed;
        store.record_completed(second, 50);
        let recent = store.list_recent(50);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].status, JobStatus::Failed);
    }

    #[test]
    fn delete_removes_from_index_and_reports_whether_it_existed() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonHistoryStore::load_or_default(tmp.path().join("history.json"));
        let a = Uuid::new_v4();
        store.record_completed(entry(a, Utc::now()), 50);
        assert!(store.delete(a));
        assert!(
            !store.delete(a),
            "deleting again must report false, not panic"
        );
        assert!(store.get_summary(a).is_none());
    }

    #[test]
    fn known_paths_unions_inputs_and_artifacts_across_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonHistoryStore::load_or_default(tmp.path().join("history.json"));
        let a = Uuid::new_v4();
        store.record_completed(entry(a, Utc::now()), 50);
        let known = store.known_paths();
        assert!(known.contains(&PathBuf::from(format!(r"C:\in\{a}.pgn"))));
        assert!(known.contains(&PathBuf::from(format!(r"C:\out\{a}.pgn"))));
    }

    #[test]
    fn state_survives_reload_from_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("history.json");
        let a = Uuid::new_v4();
        {
            let store = JsonHistoryStore::load_or_default(path.clone());
            store.record_completed(entry(a, Utc::now()), 50);
        }
        let reloaded = JsonHistoryStore::load_or_default(path);
        assert!(reloaded.get_summary(a).is_some());
    }

    #[test]
    fn missing_index_file_starts_empty_not_erroring() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonHistoryStore::load_or_default(tmp.path().join("does-not-exist.json"));
        assert!(store.list_recent(50).is_empty());
    }
}
