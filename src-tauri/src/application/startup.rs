// SPDX-License-Identifier: GPL-3.0-or-later
//! One-time application startup sequence: resolves the bundled sidecar and
//! `eco.pgn` resource paths, runs `engine::sidecar::startup_check`
//! (two-gate verify + self-test + Unicode-path probe), sweeps interrupted
//! workspaces left behind by a crash (design-02 §3.3: "Intended to run once
//! at application startup, before any new job can be started"), and loads
//! settings/history - producing the single [`AppContext`] Tauri manages for
//! the rest of the app's life.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};

use crate::engine::sidecar::{self, SidecarLocation};
use crate::persistence::history::JsonHistoryStore;
use crate::persistence::settings::JsonSettingsStore;

use super::context::{AppContext, EngineBundle};

/// `resources/pgn-extract/eco.pgn`'s relative path under both the dev tree
/// and the installed resource directory - `tauri.conf.json`'s
/// `bundle.resources` list (plain string entries, not the `{ src, target }`
/// remapping form) preserves this exact relative path under the resource
/// dir at install time.
const ECO_FILE_RELATIVE_PATH: &str = "resources/pgn-extract/eco.pgn";

fn resolve_eco_file<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    if cfg!(debug_assertions) {
        // Mirrors `engine::sidecar::SidecarLocation::dev_default`'s own
        // pattern exactly: resolve relative to *this crate's* manifest
        // directory at compile time, correct regardless of the process's
        // current working directory under `cargo tauri dev`/`cargo test`.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ECO_FILE_RELATIVE_PATH)
    } else {
        app.path()
            .resource_dir()
            .map(|dir| dir.join(ECO_FILE_RELATIVE_PATH))
            .unwrap_or_else(|_| PathBuf::from(ECO_FILE_RELATIVE_PATH))
    }
}

fn sidecar_location<R: Runtime>(app: &AppHandle<R>) -> SidecarLocation {
    if cfg!(debug_assertions) {
        SidecarLocation::dev_default()
    } else {
        match app.path().resource_dir() {
            Ok(resource_dir) => SidecarLocation::Bundled { resource_dir },
            Err(_) => SidecarLocation::dev_default(),
        }
    }
}

/// Runs the full startup sequence and builds the [`AppContext`] every
/// command handler reads. Called once from `lib.rs`'s `run()`, inside
/// `tauri::Builder::setup`, before the window is shown to the user.
///
/// Never panics or fails outright: an engine verification failure is
/// *stored* (`AppContext::engine` becomes `Err`) rather than aborting the
/// process, so the window can still open and the (engine-independent)
/// settings/history commands keep working while engine-dependent commands
/// surface the stored error directly.
pub async fn initialize<R: Runtime>(app: &AppHandle<R>) -> AppContext {
    let location = sidecar_location(app);
    let engine = match sidecar::startup_check(&location).await {
        Ok(result) => Ok(EngineBundle {
            executable: result.engine,
            capabilities: result.capabilities,
        }),
        Err(e) => Err(e),
    };

    let jobs_root = app
        .path()
        .app_cache_dir()
        .map(|dir| dir.join("jobs"))
        .unwrap_or_else(|_| std::env::temp_dir().join("pgn-studio-jobs"));
    // Design-02 §3.3: sweep exactly once, before any job can start, so a
    // leftover temp file from a killed process is never mistaken for a
    // fresh job's output. Best-effort: a sweep failure (e.g. an unreadable
    // jobs_root on first run before it exists) must not block startup.
    match crate::filesystem::workspace::sweep_interrupted_workspaces(&jobs_root) {
        Ok(report) if !report.interrupted_job_ids.is_empty() => {
            tracing::info!(
                component = "application::startup",
                interrupted = report.interrupted_job_ids.len(),
                deleted = report.deleted_paths.len(),
                cleanup_failures = report.cleanup_failures.len(),
                "swept interrupted job workspaces at startup"
            );
        }
        _ => {}
    }

    let eco_file = resolve_eco_file(app);

    let config_dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("pgn-studio-config"));
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("pgn-studio-data"));

    let settings = JsonSettingsStore::load_or_default(config_dir.join("settings.json"));
    let history = JsonHistoryStore::load_or_default(data_dir.join("history").join("index.json"));

    // architecture.md §22.1: resolved the same way every other per-app
    // directory in this function is (a real platform path in release
    // builds, a temp-dir fallback if the platform resolver ever fails) -
    // `observability::init_logging` (called earlier, from `lib.rs::run`,
    // before this function - see its own doc comment for why the ordering
    // matters) already created it; this is only a second, cheap resolution
    // of the same path so `AppContext`/the `clear_logs` command know where
    // to look.
    let log_dir = app
        .path()
        .app_log_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("pgn-studio-logs"));

    AppContext::new(
        engine,
        jobs_root,
        eco_file,
        Box::new(settings),
        Box::new(history),
        log_dir,
    )
}
