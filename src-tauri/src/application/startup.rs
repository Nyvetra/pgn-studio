// SPDX-License-Identifier: GPL-3.0-or-later
//! One-time application startup sequence: resolves the bundled sidecar and
//! `eco.pgn` resource paths, runs `engine::sidecar::startup_check`
//! (two-gate verify + self-test + Unicode-path probe), sweeps interrupted
//! workspaces left behind by a crash (design-02 §3.3: "Intended to run once
//! at application startup, before any new job can be started"), and loads
//! settings/history - producing the single [`AppContext`] Tauri manages for
//! the rest of the app's life.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, Runtime};

use crate::engine::sidecar::{self, SidecarLocation};
use crate::filesystem::eco_merge;
use crate::persistence::history::JsonHistoryStore;
use crate::persistence::settings::JsonSettingsStore;

use super::context::{AppContext, EngineBundle};

/// `resources/pgn-extract/eco.pgn`'s relative path under both the dev tree
/// and the installed resource directory - `tauri.conf.json`'s
/// `bundle.resources` list (plain string entries, not the `{ src, target }`
/// remapping form) preserves this exact relative path under the resource
/// dir at install time.
const ECO_FILE_RELATIVE_PATH: &str = "resources/pgn-extract/eco.pgn";

/// PGN Studio's own generated supplement (`scripts/build-eco-supplement.mjs`),
/// bundled next to the third-party `eco.pgn` but deliberately in its own
/// directory so the two datasets never look like one file - see
/// `filesystem::eco_merge` and `resources/eco-supplement/SOURCE.json`.
const ECO_SUPPLEMENT_RELATIVE_PATH: &str = "resources/eco-supplement/eco-supplement.pgn";

/// Resolves a bundled resource under both the dev tree and the installed
/// resource directory. Mirrors `engine::sidecar::SidecarLocation::dev_default`'s
/// own pattern exactly: in debug builds resolve relative to *this crate's*
/// manifest directory at compile time, correct regardless of the process's
/// current working directory under `cargo tauri dev`/`cargo test`.
///
/// `tauri.conf.json`'s `bundle.resources` list (plain string entries, not
/// the `{ src, target }` remapping form) preserves each relative path
/// verbatim under the resource dir at install time.
fn resolve_resource<R: Runtime>(app: &AppHandle<R>, relative: &str) -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
    } else {
        app.path()
            .resource_dir()
            .map(|dir| dir.join(relative))
            .unwrap_or_else(|_| PathBuf::from(relative))
    }
}

/// Resolves the ECO file the engine's `-e` option will be given: the
/// bundled `eco.pgn` concatenated with PGN Studio's supplement, cached
/// under `<app-cache>/eco/`.
///
/// Degrades to the bundled `eco.pgn` alone if the supplement is missing or
/// the merge fails, so ECO classification can never be *worse* than it was
/// before the supplement existed. See `filesystem::eco_merge` for why a
/// merged file (rather than two `-e` flags) is the only option.
fn resolve_eco_file<R: Runtime>(app: &AppHandle<R>, cache_root: &Path) -> PathBuf {
    let bundled = resolve_resource(app, ECO_FILE_RELATIVE_PATH);
    let supplement = resolve_resource(app, ECO_SUPPLEMENT_RELATIVE_PATH);

    let choice = eco_merge::resolve_eco_file(&bundled, &supplement, cache_root);
    if choice.merged {
        tracing::info!(
            component = "application::startup",
            path = %choice.path.display(),
            "using the merged ECO classification file"
        );
    } else {
        tracing::warn!(
            component = "application::startup",
            path = %choice.path.display(),
            reason = choice.note.as_deref().unwrap_or("unknown"),
            "falling back to the bundled eco.pgn without the supplement"
        );
    }
    choice.path
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

    // Resolved once and shared: `jobs_root` keeps its original fallback
    // path verbatim (a leftover workspace from a previous crash must stay
    // findable at the exact location the sweep has always looked), while
    // the merged ECO file gets a sibling directory under the same root.
    let cache_dir = app.path().app_cache_dir().ok();
    let jobs_root = cache_dir
        .clone()
        .map(|dir| dir.join("jobs"))
        .unwrap_or_else(|| std::env::temp_dir().join("pgn-studio-jobs"));
    let eco_cache_root = cache_dir.unwrap_or_else(|| std::env::temp_dir().join("pgn-studio-cache"));
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

    let eco_file = resolve_eco_file(app, &eco_cache_root);

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
