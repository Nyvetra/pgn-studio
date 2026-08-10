// SPDX-License-Identifier: GPL-3.0-or-later
//! Job event types and the [`JobEventSink`] port (architecture.md §10.9;
//! design-02 §2.3, §4.2).
//!
//! Phase 1b explicitly does not build Tauri commands or events (that is
//! Phase 2) - [`JobEventSink`] is the seam: `jobs::run_job` calls it for
//! every `job://*` event design-02 §4.2 defines, and Phase 2's command
//! layer implements it by calling `app_handle.emit(...)`. [`NullEventSink`]
//! lets this module (and its tests) run without any listener.

use crate::domain::{JobResult, JobStatus, OutputArtifact, ProcessingMetrics};

/// design-02 §2.3's stage sequence: `preparing` (workspace + criteria files
/// written) -> `starting` (spawn) -> `processing` (first stderr bytes, or
/// 500 ms after spawn) -> `finalizing` (exit observed; postflight +
/// publication) -> terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStage {
    Preparing,
    Starting,
    Processing,
    Finalizing,
}

impl JobStage {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStage::Preparing => "preparing",
            JobStage::Starting => "starting",
            JobStage::Processing => "processing",
            JobStage::Finalizing => "finalizing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Receives every `job://*` event (design-02 §4.2) for one job run. Every
/// method takes the event's own monotonically increasing `seq` - callers
/// (`jobs::run_job`) own the single per-job counter, per design-02's
/// "every event carries a per-job monotonically increasing `seq` (u64) so
/// the frontend can detect and discard reordered/stale deliveries."
///
/// `&self` (not `&mut self`) so a sink can be shared (e.g. `Arc<dyn
/// JobEventSink>`) across the reader tasks and the orchestrating future
/// without extra synchronization on the trait boundary itself.
pub trait JobEventSink: Send + Sync {
    fn state(&self, seq: u64, state: JobStatus);
    fn stage(&self, seq: u64, stage: JobStage, message: &str);
    fn log(&self, seq: u64, level: LogLevel, line: &str);
    fn metrics(&self, seq: u64, metrics: &ProcessingMetrics);
    fn artifact(&self, seq: u64, artifact: &OutputArtifact);
    fn completed(&self, seq: u64, result: &JobResult);
}

/// A sink that discards every event - the default for contexts (tests,
/// programmatic use without a UI) that do not need live progress.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullEventSink;

impl JobEventSink for NullEventSink {
    fn state(&self, _seq: u64, _state: JobStatus) {}
    fn stage(&self, _seq: u64, _stage: JobStage, _message: &str) {}
    fn log(&self, _seq: u64, _level: LogLevel, _line: &str) {}
    fn metrics(&self, _seq: u64, _metrics: &ProcessingMetrics) {}
    fn artifact(&self, _seq: u64, _artifact: &OutputArtifact) {}
    fn completed(&self, _seq: u64, _result: &JobResult) {}
}
