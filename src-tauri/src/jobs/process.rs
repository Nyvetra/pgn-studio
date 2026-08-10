// SPDX-License-Identifier: GPL-3.0-or-later
//! Real engine process spawn, concurrent stream draining, the log
//! pipeline, and cancellation (architecture.md §10.3, §10.9, §10.10;
//! design-02 §2.2, §2.3, §2.5).
//!
//! **Deadlock avoidance (binding):** stdout and stderr are drained by two
//! dedicated tasks for the entire process lifetime, started immediately
//! after spawn and running independently of each other and of the
//! orchestrating select loop below. Never read one stream to completion
//! before starting the other - that is the classic 64 KiB pipe-buffer
//! deadlock design-02 §2.2 warns about.
//!
//! **Line splitting (binding):** both `\n` and `\r` are treated as
//! terminators (the progress tick is CR-terminated, verified empirically:
//! `Games: N\r`); empty segments between two terminators (e.g. the empty
//! segment between `\r` and `\n` in a CRLF pair) are dropped, which also
//! makes CRLF collapse to a single line break without special-casing it.

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::AsyncReadExt;

use super::events::{JobEventSink, JobStage, LogLevel};
use crate::domain::ProcessingMetrics;
use crate::engine::command_compiler::CompiledEngineCommand;

/// Both `\n` and `\r` end a line (design-02 §2.2's line-splitting rule).
/// Kept as a small, independently testable unit.
#[derive(Debug, Default)]
pub(crate) struct LineSplitter {
    buf: Vec<u8>,
}

impl LineSplitter {
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        for &byte in chunk {
            if byte == b'\n' || byte == b'\r' {
                if !self.buf.is_empty() {
                    lines.push(String::from_utf8_lossy(&self.buf).into_owned());
                    self.buf.clear();
                }
            } else {
                self.buf.push(byte);
            }
        }
        lines
    }

    /// Any trailing partial line at EOF (no terminator ever arrived).
    pub fn finish(self) -> Option<String> {
        if self.buf.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&self.buf).into_owned())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ClassifiedLine {
    /// `^Games: (\d+)$`
    Progress {
        processed_games: u64,
    },
    /// `^(\d+) games? matched out of (\d+)\.$`
    FinalSummary {
        matched: u64,
        total: u64,
    },
    Diagnostic,
}

fn parse_progress(line: &str) -> Option<u64> {
    line.strip_prefix("Games: ")?.parse().ok()
}

fn parse_final_summary(line: &str) -> Option<(u64, u64)> {
    let (left, right) = line.split_once(" matched out of ")?;
    let total: u64 = right.strip_suffix('.')?.parse().ok()?;
    let matched_str = left
        .strip_suffix(" games")
        .or_else(|| left.strip_suffix(" game"))?;
    let matched: u64 = matched_str.parse().ok()?;
    Some((matched, total))
}

/// Builds the live [`ProcessingMetrics`] snapshot to emit on a `job://metrics`
/// event for one `Games: N` progress tick (design-02 §2.3/§4.2: "throttled
/// `job://metrics`"). `base` carries the fields already known before the
/// engine was spawned (`input_files`/`input_bytes`, per design-02 §2.4 -
/// "always" derivable, so the caller passes them in rather than this
/// function guessing or defaulting them); every other optional field of
/// `base` is expected to be `None` (not derivable until the run finishes) and
/// is passed through unchanged - only `processed_games` is overwritten.
///
/// No additional debouncing is applied here beyond what the engine itself
/// already provides: progress ticks arrive at most once per 1000 games
/// (`grammar.c:1369`, cited in design-02 §2.2), which is the throttling
/// design-02 asks for.
fn live_metrics_for_tick(base: ProcessingMetrics, processed_games: u64) -> ProcessingMetrics {
    ProcessingMetrics {
        processed_games: Some(processed_games),
        ..base
    }
}

pub(crate) fn classify_line(line: &str) -> ClassifiedLine {
    if let Some(processed_games) = parse_progress(line) {
        return ClassifiedLine::Progress { processed_games };
    }
    if let Some((matched, total)) = parse_final_summary(line) {
        return ClassifiedLine::FinalSummary { matched, total };
    }
    ClassifiedLine::Diagnostic
}

/// One line read from either stream, tagged with which one, sent into the
/// shared channel that feeds the single ordering-preserving emitter loop
/// (design-02 §2.3: "one emitter task serializes all event types for a
/// job, so relative order is preserved end-to-end").
struct StreamLine {
    line: String,
}

/// Reads one stream to EOF, splitting on `\n`/`\r`, forwarding every
/// resulting line into `tx`. Uses a **backpressuring** send (never drops):
/// the on-disk log (design-02 §2.3: "full log to disk... every
/// stderr/stdout line... is appended") must be complete, so this task
/// would rather momentarily slow the child's writes (by not yet reading
/// more from its pipe while the channel is briefly full) than lose a line.
/// This does not reintroduce the classic deadlock: the *other* stream's
/// reader task is a fully independent tokio task that keeps making
/// progress regardless of this one's backpressure state.
async fn drain_stream(
    mut reader: impl tokio::io::AsyncRead + Unpin,
    tx: tokio::sync::mpsc::Sender<StreamLine>,
) {
    let mut splitter = LineSplitter::default();
    let mut buf = [0u8; 8192];
    loop {
        let n = match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        for line in splitter.feed(&buf[..n]) {
            if tx.send(StreamLine { line }).await.is_err() {
                return; // receiver gone (run_engine returning) - stop reading
            }
        }
    }
    if let Some(last) = splitter.finish() {
        let _ = tx.send(StreamLine { line: last }).await;
    }
}

const UI_BATCH_FLUSH_LINES: usize = 64;
const UI_BATCH_FLUSH_INTERVAL: Duration = Duration::from_millis(100);
/// Defensive cap on the UI-facing batch buffer, well above the normal
/// 64-line flush threshold - see this module's report note on design-02
/// §2.3's channel-overflow wording for why this is a deliberately
/// simplified (but still bounded-memory, still order-preserving for what
/// it does keep) reading of "drops oldest... injects one synthetic line."
const UI_BATCH_HARD_CAP: usize = 2000;

struct UiBatch {
    lines: VecDeque<String>,
    dropped: u64,
}

impl UiBatch {
    fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            dropped: 0,
        }
    }

    fn push(&mut self, line: String) {
        if self.lines.len() >= UI_BATCH_HARD_CAP {
            self.lines.pop_front();
            self.dropped += 1;
        }
        self.lines.push_back(line);
    }

    fn should_flush(&self) -> bool {
        self.lines.len() >= UI_BATCH_FLUSH_LINES
    }

    fn flush(&mut self, sink: &dyn JobEventSink, seq: &AtomicU64) {
        if self.dropped > 0 {
            sink.log(
                seq.fetch_add(1, Ordering::SeqCst),
                LogLevel::Warn,
                &format!("... {} lines omitted — see full log", self.dropped),
            );
            self.dropped = 0;
        }
        while let Some(line) = self.lines.pop_front() {
            sink.log(seq.fetch_add(1, Ordering::SeqCst), LogLevel::Info, &line);
        }
    }
}

/// The outcome of one engine run - either it ran to completion (with
/// whatever exit code) or cancellation was observed and termination was
/// carried out. Either way, every line was written to the disk log before
/// this returns.
#[derive(Debug)]
pub(crate) enum EngineRunResult {
    Completed {
        exit_code: Option<i32>,
        final_summary: Option<(u64, u64)>,
        last_progress: Option<u64>,
    },
    Cancelled {
        last_progress: Option<u64>,
    },
}

// Used by the Unix branch of `request_termination` only - on a
// Windows-only compilation target (this development machine) that branch
// is `#[cfg(unix)]`-excluded entirely, which would otherwise make this
// look unused.
#[allow(dead_code)]
const CANCEL_GRACE: Duration = Duration::from_secs(3);

/// Spawns the compiled engine command, drains both streams concurrently to
/// the on-disk log (and, throttled, to `sink`), and returns once the
/// process has exited (or been cancelled and confirmed terminated).
///
/// `cancel_rx` observes `true` exactly once, when `cancel_job` is called
/// for this run (design-02 §2.5 step 1 happens in the caller, before this
/// is ever invoked - `run_engine` only implements steps 2-6).
///
/// `base_metrics` carries the pre-spawn-known fields (`input_files`/
/// `input_bytes`) used to fill out each live `job://metrics` event fired on
/// a progress tick (see [`live_metrics_for_tick`]); the caller (`jobs::run`)
/// already computes these before spawning.
pub(crate) async fn run_engine(
    compiled: &CompiledEngineCommand,
    log_path: &Path,
    mut cancel_rx: tokio::sync::watch::Receiver<bool>,
    sink: &dyn JobEventSink,
    seq: &AtomicU64,
    base_metrics: ProcessingMetrics,
) -> std::io::Result<EngineRunResult> {
    let mut cmd = crate::engine::process::build_command(
        &compiled.executable,
        &compiled.args,
        &compiled.working_directory,
    );
    let mut child = cmd.spawn()?;

    #[cfg(windows)]
    let job_object: Option<super::windows_job_object::JobObject> = {
        // `Child::raw_handle` is an inherent method on Windows (no trait
        // import needed).
        child.raw_handle().and_then(|raw| {
            match super::windows_job_object::JobObject::create() {
                Ok(job) => {
                    // Best-effort: the job object is defense-in-depth
                    // (design-02 §2.2) - if assignment fails, cancellation
                    // below falls back to `child.start_kill()`.
                    let _ = job.assign(raw as _);
                    Some(job)
                }
                Err(_) => None,
            }
        })
    };
    #[cfg(unix)]
    let child_pid: Option<i32> = child.id().map(|pid| pid as i32);

    let stdout = child
        .stdout
        .take()
        .expect("configured with Stdio::piped() in engine::process::build_command");
    let stderr = child
        .stderr
        .take()
        .expect("configured with Stdio::piped() in engine::process::build_command");

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamLine>(4096);
    let stdout_task = tokio::spawn(drain_stream(stdout, tx.clone()));
    let stderr_task = tokio::spawn(drain_stream(stderr, tx.clone()));
    drop(tx); // the two reader tasks hold the only remaining senders

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let mut log_writer = std::io::BufWriter::new(log_file);

    let mut final_summary: Option<(u64, u64)> = None;
    let mut last_progress: Option<u64> = None;
    let mut ui_batch = UiBatch::new();
    let mut flush_interval = tokio::time::interval(UI_BATCH_FLUSH_INTERVAL);
    flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut sent_processing_stage = false;
    let mut cancelled = false;

    'drain: loop {
        tokio::select! {
            biased;
            changed = cancel_rx.changed() => {
                if changed.is_ok() && *cancel_rx.borrow() {
                    cancelled = true;
                    // design-02 §2.5 step 1's "emits job://state =
                    // cancelling" - `AppState::request_cancel` (jobs::mod)
                    // only flips the watch bool (it has no event sink);
                    // this is the first point that actually observes the
                    // transition and has a sink to hand.
                    sink.state(seq.fetch_add(1, Ordering::SeqCst), crate::domain::JobStatus::Cancelling);
                    break 'drain;
                }
            }
            maybe = rx.recv() => {
                match maybe {
                    Some(StreamLine { line }) => {
                        if !sent_processing_stage {
                            sink.stage(seq.fetch_add(1, Ordering::SeqCst), JobStage::Processing, "Processing");
                            sent_processing_stage = true;
                        }
                        let _ = writeln!(log_writer, "{line}");
                        match classify_line(&line) {
                            ClassifiedLine::Progress { processed_games } => {
                                last_progress = Some(processed_games);
                                sink.metrics(
                                    seq.fetch_add(1, Ordering::SeqCst),
                                    &live_metrics_for_tick(base_metrics, processed_games),
                                );
                            }
                            ClassifiedLine::FinalSummary { matched, total } => {
                                final_summary = Some((matched, total));
                                last_progress = Some(total);
                            }
                            ClassifiedLine::Diagnostic => {
                                ui_batch.push(line);
                            }
                        }
                        if ui_batch.should_flush() {
                            ui_batch.flush(sink, seq);
                        }
                    }
                    None => break 'drain, // both readers reached EOF
                }
            }
            _ = flush_interval.tick() => {
                ui_batch.flush(sink, seq);
                let _ = log_writer.flush();
            }
        }
    }

    if cancelled {
        request_termination(&mut child, log_path).await;
        #[cfg(unix)]
        let _ = child_pid; // silence unused-on-non-unix-paths warning shape
        #[cfg(windows)]
        if let Some(job) = &job_object {
            let _ = job.terminate();
        } else {
            let _ = child.start_kill();
        }
        let _ = child.wait().await;
        // Step 5: drain to true EOF now that the process is dead.
        while let Some(StreamLine { line }) = rx.recv().await {
            let _ = writeln!(log_writer, "{line}");
        }
        let _ = stdout_task.await;
        let _ = stderr_task.await;
        ui_batch.flush(sink, seq);
        let _ = log_writer.flush();
        return Ok(EngineRunResult::Cancelled { last_progress });
    }

    let status = child.wait().await?;
    let _ = stdout_task.await;
    let _ = stderr_task.await;
    ui_batch.flush(sink, seq);
    let _ = log_writer.flush();
    Ok(EngineRunResult::Completed {
        exit_code: status.code(),
        final_summary,
        last_progress,
    })
}

/// Design-02 §2.5 steps 2-3: "send a normal termination request when
/// supported" + a bounded grace period. Unix: `SIGTERM` to the process
/// group created by `setsid` (design-02 §2.2), then up to
/// [`CANCEL_GRACE`] awaiting exit. Windows: **no-op** - "no graceful
/// signal exists for a windowless console process... this asymmetry is
/// safe because the engine holds no user-visible state: all its writes
/// target job-private temp files" (design-02 §2.5 step 2). The actual
/// force-termination (step 4) happens in the caller right after this
/// returns, per platform.
async fn request_termination(child: &mut tokio::process::Child, _log_path: &Path) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            // SAFETY: `killpg` is async-signal-safe to call from normal
            // (non-signal-handler) context; `pid as i32` is the child's own
            // pid, which is also its process group id because the child
            // called `setsid()` via `pre_exec` in
            // `engine::process::build_command`.
            unsafe {
                libc::killpg(pid as i32, libc::SIGTERM);
            }
        }
        let grace = tokio::time::sleep(CANCEL_GRACE);
        tokio::pin!(grace);
        tokio::select! {
            _ = child.wait() => {}
            _ = &mut grace => {}
        }
    }
    #[cfg(windows)]
    {
        // Nothing to do here; the caller proceeds straight to
        // TerminateJobObject/start_kill.
        let _ = child;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_splitter_treats_lf_as_terminator() {
        let mut s = LineSplitter::default();
        assert_eq!(s.feed(b"abc\ndef\n"), vec!["abc", "def"]);
    }

    #[test]
    fn line_splitter_treats_cr_as_terminator() {
        let mut s = LineSplitter::default();
        assert_eq!(
            s.feed(b"Games: 1000\rGames: 2000\r"),
            vec!["Games: 1000", "Games: 2000"]
        );
    }

    #[test]
    fn line_splitter_collapses_crlf_via_empty_segment_drop() {
        let mut s = LineSplitter::default();
        assert_eq!(
            s.feed(b"line one\r\nline two\r\n"),
            vec!["line one", "line two"]
        );
    }

    #[test]
    fn line_splitter_handles_chunk_boundaries_mid_line() {
        let mut s = LineSplitter::default();
        assert_eq!(s.feed(b"Games: 10"), Vec::<String>::new());
        assert_eq!(s.feed(b"00\r"), vec!["Games: 1000"]);
    }

    #[test]
    fn line_splitter_finish_flushes_trailing_partial_line() {
        let mut s = LineSplitter::default();
        assert_eq!(s.feed(b"no terminator yet"), Vec::<String>::new());
        assert_eq!(s.finish(), Some("no terminator yet".to_string()));
    }

    #[test]
    fn line_splitter_finish_is_none_when_last_line_was_terminated() {
        let mut s = LineSplitter::default();
        let _ = s.feed(b"complete\n");
        assert_eq!(s.finish(), None);
    }

    #[test]
    fn classify_progress_tick() {
        assert_eq!(
            classify_line("Games: 3000"),
            ClassifiedLine::Progress {
                processed_games: 3000
            }
        );
    }

    #[test]
    fn classify_final_summary_singular_and_plural() {
        assert_eq!(
            classify_line("1 game matched out of 1."),
            ClassifiedLine::FinalSummary {
                matched: 1,
                total: 1
            }
        );
        assert_eq!(
            classify_line("3 games matched out of 5."),
            ClassifiedLine::FinalSummary {
                matched: 3,
                total: 5
            }
        );
    }

    #[test]
    fn classify_anything_else_is_diagnostic() {
        assert_eq!(
            classify_line("Unable to open the ECO file eco.pgn."),
            ClassifiedLine::Diagnostic
        );
        assert_eq!(classify_line(""), ClassifiedLine::Diagnostic);
    }

    #[test]
    fn live_metrics_for_tick_overwrites_only_processed_games() {
        let base = ProcessingMetrics {
            input_files: 3,
            input_bytes: 12_345,
            processed_games: None,
            input_games: None,
            output_games: None,
            duplicate_games: None,
            broken_games: None,
            output_bytes: None,
        };
        let live = live_metrics_for_tick(base, 2000);
        assert_eq!(live.processed_games, Some(2000));
        // Every pre-spawn-known field is carried through unchanged, and
        // every not-yet-derivable field stays None - never a guessed 0
        // (design-02 §2.4's binding "never substitute 0" rule).
        assert_eq!(live.input_files, base.input_files);
        assert_eq!(live.input_bytes, base.input_bytes);
        assert_eq!(live.input_games, None);
        assert_eq!(live.output_games, None);
        assert_eq!(live.duplicate_games, None);
        assert_eq!(live.broken_games, None);
        assert_eq!(live.output_bytes, None);
    }

    #[test]
    fn ui_batch_flush_calls_sink_for_every_line_in_order() {
        struct Recorder(std::sync::Mutex<Vec<String>>);
        impl JobEventSink for Recorder {
            fn state(&self, _: u64, _: crate::domain::JobStatus) {}
            fn stage(&self, _: u64, _: JobStage, _: &str) {}
            fn log(&self, _: u64, _: LogLevel, line: &str) {
                self.0.lock().unwrap().push(line.to_string());
            }
            fn metrics(&self, _: u64, _: &crate::domain::ProcessingMetrics) {}
            fn artifact(&self, _: u64, _: &crate::domain::OutputArtifact) {}
            fn completed(&self, _: u64, _: &crate::domain::JobResult) {}
        }
        let recorder = Recorder(std::sync::Mutex::new(Vec::new()));
        let seq = AtomicU64::new(0);
        let mut batch = UiBatch::new();
        batch.push("a".to_string());
        batch.push("b".to_string());
        batch.flush(&recorder, &seq);
        assert_eq!(recorder.0.into_inner().unwrap(), vec!["a", "b"]);
    }
}
