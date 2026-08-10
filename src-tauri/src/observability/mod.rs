// SPDX-License-Identifier: GPL-3.0-or-later
//! Structured local logging (architecture.md §22.1) plus its two safety
//! rails: bounded retention (file count *and* total size) and a "Clear
//! Logs" action.
//!
//! **Binding: architecture.md §22.3 - no telemetry in Version 1.** This
//! module writes newline-delimited JSON to a local file under the
//! platform's app-log directory and nowhere else. There is no analytics
//! SDK, no crash-upload SDK, and no remote error collector anywhere in this
//! crate or its dependency tree that this module (or anything else in the
//! app) calls into - see this crate's Phase 6 report for the verification
//! sweep (`cargo tree`, plus a source grep for HTTP client crates/`fetch`/
//! `XMLHttpRequest` across both the Rust and TypeScript trees). Every field
//! [`init_logging`] attaches to the subscriber stays on this machine.
//!
//! Every `tracing::*!` call site elsewhere in this crate already carries
//! `timestamp` (subscriber-stamped) and `level` (the macro itself) for
//! free; this module's job is to (1) actually install a subscriber so those
//! events are captured at all - see `errors::log_technical_detail`'s own
//! doc comment, written when no subscriber existed yet - and (2) keep the
//! result bounded and user-clearable.

use std::path::{Path, PathBuf};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Every log file this module writes is named `pgn-studio.<date>.<n>` by
/// `tracing_appender::rolling::daily`'s own convention, given this prefix.
/// [`enforce_retention`] and [`clear_logs`] only ever touch files starting
/// with this exact prefix in the target directory - never anything else
/// that might live alongside them - mirroring
/// `filesystem::workspace`'s "only ever touch what we recognize" discipline
/// for its own swept temp files.
const LOG_FILE_PREFIX: &str = "pgn-studio.log";

/// architecture.md §22.1: "Default retention: a bounded number of files and
/// total size." Two weeks of daily rotation by default.
pub const DEFAULT_MAX_LOG_FILES: usize = 14;
/// 50 MiB combined, regardless of file count - whichever bound is tighter
/// wins (see [`enforce_retention`]).
pub const DEFAULT_MAX_LOG_BYTES: u64 = 50 * 1024 * 1024;

/// Installs the process-global `tracing` subscriber: structured JSON to a
/// daily-rotating file under `log_dir` (created if missing), written on a
/// background thread (`tracing_appender::non_blocking`) so no command
/// handler or engine-output-draining loop ever blocks on log I/O
/// (architecture.md §19.4). Debug builds additionally echo events to
/// stderr for local development; release builds write to the file only.
///
/// Retention is enforced once, up front, before the new file is even
/// opened - the same "sweep before anything new can be mistaken for
/// current" ordering `filesystem::workspace::sweep_interrupted_workspaces`
/// uses at startup.
///
/// Returns the [`WorkerGuard`] the caller must keep alive for the whole
/// process lifetime - dropping it flushes and stops the background writer
/// (`tracing_appender`'s own documented contract). `lib.rs::run()` holds it
/// via `tauri::App::manage`, the same mechanism that keeps [`crate::application::AppContext`]
/// alive for exactly as long as the app runs.
///
/// Never panics and never blocks app startup on a logging failure - a
/// directory that cannot be created, or a subscriber that is somehow
/// already installed (never happens in normal operation; only a
/// theoretical concern if this were ever called twice), is reported to
/// stderr and otherwise ignored. Logging must never be the reason the app
/// fails to start.
pub fn init_logging(log_dir: &Path) -> Option<WorkerGuard> {
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        eprintln!(
            "pgn-studio: could not create log directory {} ({e}); logging is disabled for this run",
            log_dir.display()
        );
        return None;
    }

    enforce_retention(log_dir, DEFAULT_MAX_LOG_FILES, DEFAULT_MAX_LOG_BYTES);

    let file_appender = tracing_appender::rolling::daily(log_dir, LOG_FILE_PREFIX);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    let registry = tracing_subscriber::registry().with(file_layer);

    #[cfg(debug_assertions)]
    let registry = registry.with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true),
    );

    if registry.try_init().is_err() {
        eprintln!("pgn-studio: a tracing subscriber was already active; logging continues on it");
    }

    Some(guard)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetentionReport {
    pub deleted: Vec<PathBuf>,
    pub deletion_failures: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClearLogsReport {
    pub deleted: Vec<PathBuf>,
    pub deletion_failures: Vec<PathBuf>,
}

/// Every file in `log_dir` whose name starts with [`LOG_FILE_PREFIX`],
/// paired with its size. Anything that does not match the prefix, or that
/// cannot be stat-ed, is silently excluded - this function only ever
/// reports on files it is prepared to manage, never treats an unreadable
/// entry as an error that should block the caller.
fn list_log_files(log_dir: &Path) -> Vec<(PathBuf, u64)> {
    let Ok(entries) = std::fs::read_dir(log_dir) else {
        return Vec::new();
    };
    let mut files: Vec<(PathBuf, u64)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| e.file_name().to_string_lossy().starts_with(LOG_FILE_PREFIX))
        .map(|e| {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            (e.path(), size)
        })
        .collect();
    // Oldest first: log file names embed a `YYYY-MM-DD` date suffix
    // (`tracing_appender::rolling::daily`'s own naming), which sorts
    // lexicographically == chronologically. This is a pure path-string
    // sort deliberately, not a filesystem-metadata sort - modification
    // time can be altered by copying/backup tools in a way the embedded
    // date cannot.
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// Deletes the oldest log files until **both** bounds hold: at most
/// `max_files` remain, and their combined size is at most
/// `max_total_bytes` (architecture.md §22.1: "a bounded number of files
/// and total size" - both, not either). Deletion failures (e.g. a file
/// held open elsewhere) are recorded, never silently swallowed
/// (architecture.md §18.3's honesty rule, applied here to log housekeeping
/// rather than a job's own temp files) - and a failure on one file does not
/// stop the sweep from still trying the next-oldest one.
pub fn enforce_retention(
    log_dir: &Path,
    max_files: usize,
    max_total_bytes: u64,
) -> RetentionReport {
    let mut report = RetentionReport::default();
    let files = list_log_files(log_dir);

    let mut remaining = files.len();
    let mut total: u64 = files.iter().map(|(_, size)| size).sum();

    for (path, size) in files {
        if remaining <= max_files && total <= max_total_bytes {
            break;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                report.deleted.push(path);
                remaining -= 1;
                total = total.saturating_sub(size);
            }
            Err(_) => report.deletion_failures.push(path),
        }
    }
    report
}

/// architecture.md §22.1: "Provide 'Clear Logs.'" Deletes every recognized
/// log file in `log_dir` unconditionally - the user-initiated, full-clear
/// counterpart to [`enforce_retention`]'s automatic, bounded sweep. Never
/// touches a file outside [`LOG_FILE_PREFIX`], even though `log_dir` is
/// otherwise exclusively owned by this module (defense in depth, matching
/// this whole module's "never delete what we don't recognize" discipline).
pub fn clear_logs(log_dir: &Path) -> ClearLogsReport {
    let mut report = ClearLogsReport::default();
    for (path, _) in list_log_files(log_dir) {
        match std::fs::remove_file(&path) {
            Ok(()) => report.deleted.push(path),
            Err(_) => report.deletion_failures.push(path),
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str, bytes: usize) {
        std::fs::write(dir.join(name), vec![b'x'; bytes]).unwrap();
    }

    #[test]
    fn enforce_retention_leaves_a_directory_under_both_bounds_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "pgn-studio.log.2026-08-01", 10);
        touch(tmp.path(), "pgn-studio.log.2026-08-02", 10);
        let report = enforce_retention(tmp.path(), 10, 10_000);
        assert!(report.deleted.is_empty());
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 2);
    }

    #[test]
    fn enforce_retention_deletes_oldest_first_beyond_the_file_count_bound() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "pgn-studio.log.2026-08-01", 10);
        touch(tmp.path(), "pgn-studio.log.2026-08-02", 10);
        touch(tmp.path(), "pgn-studio.log.2026-08-03", 10);
        let report = enforce_retention(tmp.path(), 2, 10_000);
        assert_eq!(
            report.deleted,
            vec![tmp.path().join("pgn-studio.log.2026-08-01")]
        );
        assert!(tmp.path().join("pgn-studio.log.2026-08-02").exists());
        assert!(tmp.path().join("pgn-studio.log.2026-08-03").exists());
    }

    #[test]
    fn enforce_retention_deletes_oldest_first_beyond_the_total_size_bound() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "pgn-studio.log.2026-08-01", 100);
        touch(tmp.path(), "pgn-studio.log.2026-08-02", 100);
        // Bound only allows one 100-byte file to remain.
        let report = enforce_retention(tmp.path(), 10, 150);
        assert_eq!(
            report.deleted,
            vec![tmp.path().join("pgn-studio.log.2026-08-01")]
        );
        assert!(tmp.path().join("pgn-studio.log.2026-08-02").exists());
    }

    #[test]
    fn enforce_retention_never_touches_a_file_outside_the_recognized_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "unrelated-file.txt", 100);
        let report = enforce_retention(tmp.path(), 0, 0);
        assert!(report.deleted.is_empty());
        assert!(tmp.path().join("unrelated-file.txt").exists());
    }

    #[test]
    fn enforce_retention_on_a_missing_directory_is_a_no_op_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist-yet");
        let report = enforce_retention(&missing, 1, 1);
        assert!(report.deleted.is_empty());
        assert!(report.deletion_failures.is_empty());
    }

    #[test]
    fn clear_logs_deletes_every_recognized_file_and_reports_them() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "pgn-studio.log.2026-08-01", 10);
        touch(tmp.path(), "pgn-studio.log.2026-08-02", 10);
        let report = clear_logs(tmp.path());
        assert_eq!(report.deleted.len(), 2);
        assert!(report.deletion_failures.is_empty());
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn clear_logs_leaves_an_unrelated_file_alone() {
        let tmp = tempfile::tempdir().unwrap();
        touch(tmp.path(), "pgn-studio.log.2026-08-01", 10);
        touch(tmp.path(), "notes.txt", 10);
        let report = clear_logs(tmp.path());
        assert_eq!(
            report.deleted,
            vec![tmp.path().join("pgn-studio.log.2026-08-01")]
        );
        assert!(tmp.path().join("notes.txt").exists());
    }

    #[test]
    fn init_logging_creates_the_log_directory_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let log_dir = tmp.path().join("logs");
        assert!(!log_dir.exists());
        // A subscriber may already be active from an earlier test in this
        // binary (`try_init` is deliberately tolerant of that - see
        // `init_logging`'s own doc comment) - this test asserts the
        // directory/guard side effects, not global-subscriber installation,
        // since only one test process-wide can ever be the one that wins.
        let guard = init_logging(&log_dir);
        assert!(log_dir.is_dir());
        assert!(guard.is_some());
    }
}
