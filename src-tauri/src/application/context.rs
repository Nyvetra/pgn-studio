// SPDX-License-Identifier: GPL-3.0-or-later
//! [`AppContext`]: the single piece of Tauri-managed state every command
//! handler reads (architecture.md §7.1's application/orchestration layer).
//!
//! Holds the verified engine bundle (or the startup failure that prevents
//! one from existing - engine-dependent commands surface that failure
//! directly rather than the app refusing to start at all), the
//! single-flight job guard (`jobs::AppState`, unchanged from Phase 1b), the
//! settings/history stores (behind their small traits, per the task's
//! "Phase 6 can swap it" instruction), and the small amount of live-job
//! bookkeeping `get_job`/`reveal_path`/`open_path` need while a job is
//! `Running`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{
    EngineCapabilities, JobStatus, JobWarning, OutputArtifact, ProcessingMetrics, PublicError,
};
use crate::engine::EngineExecutable;
use crate::persistence::history::HistoryStore;
use crate::persistence::settings::SettingsStore;

/// A verified sidecar plus its (probed, not merely static) capabilities -
/// everything `engine::sidecar::startup_check` produces, held for the life
/// of the app.
pub struct EngineBundle {
    pub executable: EngineExecutable,
    pub capabilities: EngineCapabilities,
}

/// Best-effort snapshot of the currently `Running`/`Cancelling` job,
/// updated by [`super::events::TauriJobEventSink`] on every callback so
/// `get_job` can answer for the active job without waiting for it to finish
/// (design-02 §4.2: `get_job` is also how the frontend reconciles state
/// after a reload, so it must work whether or not the job in question has
/// completed yet).
#[derive(Debug, Clone)]
pub struct LiveJobSnapshot {
    pub job_id: Uuid,
    pub name: String,
    pub status: JobStatus,
    pub started_at: DateTime<Utc>,
    pub metrics: ProcessingMetrics,
    pub artifacts: Vec<OutputArtifact>,
    pub warnings: Vec<JobWarning>,
    /// Recorded at acceptance time so the active job's own sources are
    /// reveal/open-able before it finishes (they are also "job history" in
    /// the sense that matters for the allowlist - the user just selected
    /// them - even though the job has not reached a terminal state yet).
    pub input_paths: Vec<PathBuf>,
}

/// Tauri-managed application state (`app.manage(AppContext::new(...))`,
/// read via `tauri::State<'_, AppContext>` in every command handler).
pub struct AppContext {
    /// `Err` iff the startup sidecar verification failed (design-02 §5.1:
    /// `ENGINE_MISSING`/`ENGINE_TAMPERED`/`ENGINE_START_FAILED`). Stored
    /// rather than causing the whole app to abort so the window can still
    /// open and show the user *why* the engine is unavailable (e.g. "the
    /// bundled engine failed verification - reinstall") instead of the
    /// process silently exiting before any UI exists.
    pub engine: Result<EngineBundle, PublicError>,
    pub jobs: crate::jobs::AppState,
    pub jobs_root: PathBuf,
    pub eco_file: PathBuf,
    pub settings: Box<dyn SettingsStore>,
    pub history: Box<dyn HistoryStore>,
    pub live_job: Mutex<Option<LiveJobSnapshot>>,
}

impl AppContext {
    pub fn new(
        engine: Result<EngineBundle, PublicError>,
        jobs_root: PathBuf,
        eco_file: PathBuf,
        settings: Box<dyn SettingsStore>,
        history: Box<dyn HistoryStore>,
    ) -> Self {
        Self {
            engine,
            jobs: crate::jobs::AppState::new(),
            jobs_root,
            eco_file,
            settings,
            history,
            live_job: Mutex::new(None),
        }
    }

    /// A defensive clone of the engine bundle's pieces for the common
    /// "borrow both at once" shape `RunJobContext`/`ValidationLayout`
    /// need, or the stored startup error if the engine never verified.
    pub fn engine_bundle(&self) -> Result<&EngineBundle, PublicError> {
        self.engine.as_ref().map_err(Clone::clone)
    }

    /// The allowlist `reveal_path`/`open_path` check against (design-02
    /// §4.1: "only paths present in history/artifacts"): every path known
    /// from persisted history, plus the currently active job's own inputs/
    /// artifacts-so-far (which are not in history yet - it has not
    /// completed).
    pub fn known_paths(&self) -> HashSet<PathBuf> {
        let mut set: HashSet<PathBuf> = self.history.known_paths().into_iter().collect();
        if let Some(live) = self
            .live_job
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            set.extend(live.artifacts.iter().map(|a| a.path.clone()));
            set.extend(live.input_paths.iter().cloned());
        }
        set
    }
}
