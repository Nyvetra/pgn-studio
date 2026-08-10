// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-job workspace creation and the startup interrupted-workspace sweeper
//! (architecture.md §11.3; design-02 §3.3).
//!
//! ```text
//! <app-cache>/jobs/<job-uuid>/
//! ├── criteria/
//! ├── logs/
//! ├── engine/                (reserved; empty in V1)
//! ├── manifest.draft.json    (written BEFORE spawn)
//! └── virtual.tmp            (engine-created iff -Z; CWD == workspace root)
//! ```
//!
//! Sources are referenced in place - this module never copies input PGNs.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::manifest::{DraftManifest, FinalManifest};

/// Handle to a created per-job workspace: just the resolved paths inside
/// it. Cheap to construct/clone; holds no open resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobWorkspace {
    root: PathBuf,
}

impl JobWorkspace {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn criteria_dir(&self) -> PathBuf {
        self.root.join("criteria")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn engine_dir(&self) -> PathBuf {
        self.root.join("engine")
    }

    pub fn engine_log_path(&self) -> PathBuf {
        self.logs_dir().join("engine.log")
    }

    pub fn manifest_draft_path(&self) -> PathBuf {
        self.root.join("manifest.draft.json")
    }

    pub fn manifest_final_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    /// CWD == workspace root (Decision D-7), so this is exactly where the
    /// engine's own `-Z` mode creates `virtual.tmp` (a relative path from
    /// its perspective).
    pub fn virtual_tmp_path(&self) -> PathBuf {
        self.root.join("virtual.tmp")
    }
}

/// The deterministic workspace path for a job id, without creating
/// anything - useful for [`super::validate::ValidationLayout::workspace_root`],
/// which must be computed before a job is actually started.
pub fn workspace_root_for(jobs_root: &Path, job_id: Uuid) -> PathBuf {
    jobs_root.join(job_id.to_string())
}

/// Creates the per-job workspace directory tree (design-02 §3.3). Uses
/// `create_dir_all`, so it is safe to call even if some of the tree already
/// exists (it never truncates/clears an existing directory).
pub fn create_job_workspace(jobs_root: &Path, job_id: Uuid) -> io::Result<JobWorkspace> {
    let root = workspace_root_for(jobs_root, job_id);
    std::fs::create_dir_all(root.join("criteria"))?;
    std::fs::create_dir_all(root.join("logs"))?;
    std::fs::create_dir_all(root.join("engine"))?;
    Ok(JobWorkspace { root })
}

/// Writes the draft manifest (design-02 §3.3: "written BEFORE spawn"). Not
/// itself published via an atomic rename - only the *promotion* to the
/// final manifest (`workspace.manifest_final_path()`) needs that guarantee;
/// the draft's only job is to exist, readable, before the engine starts.
pub fn write_draft_manifest(workspace: &JobWorkspace, draft: &DraftManifest) -> io::Result<()> {
    let content = serde_json::to_vec_pretty(draft)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(workspace.manifest_draft_path(), content)
}

/// Writes the completed manifest **last** (design-02 §3.4 step 7):
/// overwrites `manifest.draft.json` with the final content, `fsync`s it,
/// then atomically renames it to `manifest.json`. A plain
/// `std::fs::rename` is correct here (unlike published destination
/// artifacts, this is an internal workspace bookkeeping file with no
/// conflict-policy concept - each job owns a fresh workspace, so
/// `manifest.json` never already exists at this point).
///
/// "Manifest written last, so its presence implies all listed artifacts
/// were fully published": callers must not call this until every artifact
/// in `manifest.artifacts` has already been renamed to its final
/// destination path.
pub fn write_final_manifest(workspace: &JobWorkspace, manifest: &FinalManifest) -> io::Result<()> {
    let content = serde_json::to_vec_pretty(manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let draft_path = workspace.manifest_draft_path();
    {
        let mut file = std::fs::File::create(&draft_path)?;
        file.write_all(&content)?;
        file.sync_all()?;
    }
    std::fs::rename(&draft_path, workspace.manifest_final_path())
}

/// The result of a startup sweep (design-02 §3.3): every interrupted
/// workspace found, every temp file actually deleted, and every temp file
/// a deletion attempt failed on (never silently claimed as cleaned up -
/// architecture.md §18.3).
#[derive(Debug, Clone, Default)]
pub struct SweepReport {
    pub interrupted_job_ids: Vec<Uuid>,
    pub deleted_paths: Vec<PathBuf>,
    pub cleanup_failures: Vec<PathBuf>,
}

/// Only names matching this prefix are ever swept, even if a (corrupted or
/// hand-edited) draft manifest lists something else - "nothing else is
/// ever swept" (design-02 §3.3) is enforced here, not just at write time.
const SWEPT_TEMP_PREFIX: &str = ".pgnstudio-tmp-";

fn is_swept_temp_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with(SWEPT_TEMP_PREFIX))
        .unwrap_or(false)
}

/// Scans `jobs_root` for workspaces left behind by a crash (design-02
/// §3.3): a workspace directory whose `manifest.draft.json` exists but
/// whose `manifest.json` (the terminal manifest, written last on every
/// normal exit path - success, failure, *and* cancellation) does not.
/// Deletes the temp outputs the draft recorded, plus `virtual.tmp`, and
/// reports the job as interrupted.
///
/// Intended to run once at application startup, before any new job can be
/// started (so a leftover temp file from a killed process is never
/// mistaken for a fresh job's output).
pub fn sweep_interrupted_workspaces(jobs_root: &Path) -> io::Result<SweepReport> {
    let mut report = SweepReport::default();
    if !jobs_root.is_dir() {
        return Ok(report);
    }
    for entry in std::fs::read_dir(jobs_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let workspace_root = entry.path();
        if workspace_root.join("manifest.json").exists() {
            continue; // reached a terminal state normally - nothing to do
        }
        let draft_path = workspace_root.join("manifest.draft.json");
        if !draft_path.is_file() {
            continue; // not a recognizable job workspace - leave it alone
        }
        let job_id = match entry
            .file_name()
            .to_str()
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(id) => id,
            None => continue,
        };
        report.interrupted_job_ids.push(job_id);

        if let Ok(bytes) = std::fs::read(&draft_path) {
            if let Ok(draft) = serde_json::from_slice::<DraftManifest>(&bytes) {
                for temp_path in &draft.temp_outputs {
                    if !is_swept_temp_name(temp_path) {
                        continue;
                    }
                    sweep_one(temp_path, &mut report);
                }
            }
        }

        let virtual_tmp = workspace_root.join("virtual.tmp");
        sweep_one(&virtual_tmp, &mut report);
    }
    Ok(report)
}

fn sweep_one(path: &Path, report: &mut SweepReport) {
    if !path.exists() {
        return;
    }
    match std::fs::remove_file(path) {
        Ok(()) => report.deleted_paths.push(path.to_path_buf()),
        Err(_) => report.cleanup_failures.push(path.to_path_buf()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filesystem::manifest::MANIFEST_SCHEMA_VERSION;
    use chrono::Utc;

    fn sample_spec() -> crate::domain::JobSpec {
        crate::domain::JobSpec {
            schema_version: crate::domain::CURRENT_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            name: "sweep-test".to_string(),
            inputs: vec![],
            output: crate::domain::OutputPlan {
                directory: PathBuf::from(r"C:\dest"),
                base_name: "out".to_string(),
                unique_games: true,
                duplicate_games: crate::domain::DuplicateOutput::None,
                log_file: false,
                manifest: false,
                always_create_audit: false,
                conflict_policy: crate::domain::ConflictPolicy::Fail,
                confirmed_replace: false,
            },
            operations: crate::domain::OperationPlan {
                mode: crate::domain::JobMode::Process,
                duplicates: crate::domain::DuplicatePolicy::None,
                cleanup: crate::domain::CleanupOptions {
                    remove_comments: false,
                    remove_variations: false,
                    remove_nags: false,
                    remove_move_numbers: false,
                    remove_results: false,
                    remove_tags: vec![],
                    reject_bad_results: false,
                    fix_result_tags: false,
                },
                broken: crate::domain::BrokenOutput::Discard,
                eco: crate::domain::EcoOptions { enabled: false },
                output_notation: crate::domain::OutputNotation::San,
                check_file: None,
            },
            filters: crate::domain::FilterPlan {
                tag_rules: vec![],
                move_bounds: None,
                checkmate_only: false,
                setup_policy: crate::domain::SetupPolicy::Any,
                fen_pattern: None,
                textual_variations: vec![],
                advanced_args: vec![],
            },
            runtime: crate::domain::RuntimeOptions {
                use_external_duplicate_table: false,
                count_output_games: true,
            },
        }
    }

    #[test]
    fn create_job_workspace_makes_the_full_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let job_id = Uuid::new_v4();
        let ws = create_job_workspace(tmp.path(), job_id).unwrap();
        assert!(ws.criteria_dir().is_dir());
        assert!(ws.logs_dir().is_dir());
        assert!(ws.engine_dir().is_dir());
        assert_eq!(ws.root(), tmp.path().join(job_id.to_string()));
    }

    #[test]
    fn sweep_deletes_leftover_temp_outputs_and_marks_interrupted() {
        let tmp = tempfile::tempdir().unwrap();
        let job_id = Uuid::new_v4();
        let ws = create_job_workspace(tmp.path(), job_id).unwrap();

        let dest = tmp.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        let leftover_unique = dest.join(".pgnstudio-tmp-abcabcabcabc-unique.pgn");
        std::fs::write(&leftover_unique, b"partial").unwrap();
        let virtual_tmp = ws.virtual_tmp_path();
        std::fs::write(&virtual_tmp, b"hash table").unwrap();

        let draft = DraftManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            job_id,
            spec: sample_spec(),
            argv: vec!["-s".to_string(), "--summary".to_string()],
            criteria_files: vec![],
            temp_outputs: vec![leftover_unique.clone()],
            created_at: Utc::now(),
        };
        write_draft_manifest(&ws, &draft).unwrap();

        let report = sweep_interrupted_workspaces(tmp.path()).unwrap();
        assert_eq!(report.interrupted_job_ids, vec![job_id]);
        assert!(!leftover_unique.exists());
        assert!(!virtual_tmp.exists());
        assert!(report.deleted_paths.contains(&leftover_unique));
        assert!(report.deleted_paths.contains(&virtual_tmp));
    }

    #[test]
    fn sweep_ignores_workspace_with_final_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let job_id = Uuid::new_v4();
        let ws = create_job_workspace(tmp.path(), job_id).unwrap();
        std::fs::write(ws.manifest_final_path(), b"{}").unwrap();

        let dest = tmp.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        let survivor = dest.join(".pgnstudio-tmp-shouldsurvive-unique.pgn");
        std::fs::write(&survivor, b"do not touch").unwrap();
        let draft = DraftManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            job_id,
            spec: sample_spec(),
            argv: vec![],
            criteria_files: vec![],
            temp_outputs: vec![survivor.clone()],
            created_at: Utc::now(),
        };
        write_draft_manifest(&ws, &draft).unwrap();

        let report = sweep_interrupted_workspaces(tmp.path()).unwrap();
        assert!(report.interrupted_job_ids.is_empty());
        assert!(survivor.exists(), "a completed job's files must survive");
    }

    #[test]
    fn sweep_never_deletes_a_path_outside_the_swept_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let job_id = Uuid::new_v4();
        let ws = create_job_workspace(tmp.path(), job_id).unwrap();

        // A maliciously/accidentally corrupted draft naming a real file
        // that does NOT match the .pgnstudio-tmp- prefix must never be
        // deleted, even though it is listed in temp_outputs.
        let precious = tmp.path().join("not-a-temp-file.pgn");
        std::fs::write(&precious, b"precious").unwrap();
        let draft = DraftManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            job_id,
            spec: sample_spec(),
            argv: vec![],
            criteria_files: vec![],
            temp_outputs: vec![precious.clone()],
            created_at: Utc::now(),
        };
        write_draft_manifest(&ws, &draft).unwrap();

        let report = sweep_interrupted_workspaces(tmp.path()).unwrap();
        assert!(precious.exists());
        assert!(report.deleted_paths.is_empty());
    }
}
