// SPDX-License-Identifier: GPL-3.0-or-later
//! The `job://*` wire event ([`JobEvent`]) and its Tauri emitter
//! ([`TauriJobEventSink`]) - design-02 §4.2, wiring `jobs::JobEventSink`
//! (the seam Phase 1b left exactly for this, per that trait's own doc
//! comment) to real `job://state`/`job://stage`/`job://log`/`job://metrics`/
//! `job://artifact`/`job://completed` events.
//!
//! **Six channels, one Rust enum (design-02 §4.2, binding):** `JobEvent` is
//! a single `#[serde(tag = "type")]` enum so there is exactly one generated
//! TypeScript union type, but each variant is emitted on its own named
//! channel (`job://state`, ...) via [`JobEvent::channel`] rather than
//! through `tauri_specta`'s `Event` derive/`collect_events!` machinery -
//! that machinery binds one Rust *type* to exactly one channel name, which
//! does not fit "one closed union, six channels". `.typ::<JobEvent>()`
//! (`commands::specta_builder`) still gets the type into
//! `generated-types.ts`; `src/ipc/events.ts` hand-writes the six typed
//! `listen` wrappers on top of it, exactly the kind of "one small
//! hand-written typed listen wrapper" design-02 §4.3 already pre-approves
//! for the ts-rs fallback path - here it is just the event side, since
//! commands still get fully generated typed wrappers.

use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use uuid::Uuid;

use crate::domain::{JobResult, JobStatus, OutputArtifact, ProcessingMetrics};
use crate::jobs::{JobEventSink, JobStage, LogLevel};

use super::context::{AppContext, LiveJobSnapshot};

/// The closed `job://*` payload union (design-02 §4.2). `Clone` is required
/// by `tauri::Emitter::emit`'s `Serialize + Clone` bound.
///
/// **`rename_all_fields`, not just `rename_all` (load-bearing):** on an
/// enum, plain `rename_all = "camelCase"` only camel-cases the *variant*
/// names (the `type` tag's value: `"state"`, `"stage"`, ...) - it does
/// **not** cascade into each struct variant's own field names. Without
/// `rename_all_fields` here too, every event would serialize `job_id`
/// verbatim (snake_case) while every other DTO in this crate uses camelCase,
/// a real wire-format inconsistency that was caught by this module's own
/// `job_event_serializes_job_id_as_camel_case_on_the_wire` test comparing
/// actual `serde_json::to_string` output against the generated TypeScript
/// (`specta`/`specta-serde` correctly reflect whichever behavior serde
/// itself has, so a mismatch here would have been a real bug, not just a
/// cosmetic one).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum JobEvent {
    State {
        job_id: Uuid,
        seq: u64,
        state: JobStatus,
    },
    Stage {
        job_id: Uuid,
        seq: u64,
        stage: JobStage,
        message: String,
    },
    Log {
        job_id: Uuid,
        seq: u64,
        level: LogLevel,
        line: String,
    },
    Metrics {
        job_id: Uuid,
        seq: u64,
        metrics: ProcessingMetrics,
    },
    Artifact {
        job_id: Uuid,
        seq: u64,
        artifact: OutputArtifact,
    },
    Completed {
        job_id: Uuid,
        seq: u64,
        // Boxed: `JobResult` is by far the largest variant payload here
        // (clippy::large_enum_variant) - boxing keeps every other, far
        // more frequent event (`state`/`stage`/`log`/`metrics`/`artifact`)
        // from paying for `JobResult`'s size on every `JobEvent` value.
        result: Box<JobResult>,
    },
}

impl JobEvent {
    /// The exact channel name each variant is emitted on (design-02 §4.2 /
    /// architecture.md §14.2). Kept as one small match here rather than six
    /// separate emit call sites, so the channel-name/variant pairing can
    /// never drift.
    pub fn channel(&self) -> &'static str {
        match self {
            JobEvent::State { .. } => "job://state",
            JobEvent::Stage { .. } => "job://stage",
            JobEvent::Log { .. } => "job://log",
            JobEvent::Metrics { .. } => "job://metrics",
            JobEvent::Artifact { .. } => "job://artifact",
            JobEvent::Completed { .. } => "job://completed",
        }
    }
}

/// Implements [`JobEventSink`] by emitting [`JobEvent`]s through a Tauri
/// `AppHandle` and mirroring live progress into
/// `AppContext::live_job` (so `get_job` can answer for the active job
/// without waiting for `job://completed`).
///
/// Re-fetches `app.state::<AppContext>()` on every call rather than holding
/// a direct reference, since `AppHandle` is the only piece that needs to be
/// `'static` for use inside `tokio::spawn` (design-02 §4.1's `start_job`
/// runs the job in a background task and returns once it is accepted, not
/// once it finishes - see `commands::jobs::start_job`).
pub struct TauriJobEventSink<R: Runtime> {
    app: AppHandle<R>,
    job_id: Uuid,
    name: String,
    started_at: DateTime<Utc>,
    input_paths: Vec<std::path::PathBuf>,
}

impl<R: Runtime> TauriJobEventSink<R> {
    pub fn new(
        app: AppHandle<R>,
        job_id: Uuid,
        name: String,
        input_paths: Vec<std::path::PathBuf>,
    ) -> Self {
        Self {
            app,
            job_id,
            name,
            started_at: Utc::now(),
            input_paths,
        }
    }

    fn emit(&self, event: JobEvent) {
        // Emission failure (e.g. no window yet) is not a job failure - the
        // full log on disk and the final manifest (`jobs::run::finalize`)
        // remain authoritative regardless of whether any UI was listening.
        let _ = self.app.emit(event.channel(), event);
    }

    fn with_live<T>(&self, f: impl FnOnce(&mut Option<LiveJobSnapshot>) -> T) -> T {
        let ctx = self.app.state::<AppContext>();
        let mut guard = ctx.live_job.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut guard)
    }
}

impl<R: Runtime> JobEventSink for TauriJobEventSink<R> {
    fn state(&self, seq: u64, state: JobStatus) {
        // `run_job` calls `sink.state(_, Running)` exactly once, immediately
        // after successfully acquiring the single-flight slot (`jobs::run::
        // run_job`) - this is therefore the one safe place to *create* the
        // live snapshot. Creating it here (rather than in `new`) means a
        // `run_job` call that fails fast with `JOB_ALREADY_RUNNING` (which
        // returns before ever calling the sink) never touches `live_job` at
        // all, so it can never clobber a different, genuinely-running job's
        // snapshot.
        self.with_live(|slot| {
            if state == JobStatus::Running {
                *slot = Some(LiveJobSnapshot {
                    job_id: self.job_id,
                    name: self.name.clone(),
                    status: state,
                    started_at: self.started_at,
                    metrics: ProcessingMetrics {
                        input_files: 0,
                        input_bytes: 0,
                        processed_games: None,
                        input_games: None,
                        output_games: None,
                        duplicate_games: None,
                        broken_games: None,
                        output_bytes: None,
                    },
                    artifacts: Vec::new(),
                    warnings: Vec::new(),
                    input_paths: self.input_paths.clone(),
                });
            } else if let Some(snap) = slot.as_mut() {
                if snap.job_id == self.job_id {
                    snap.status = state;
                }
            }
        });
        self.emit(JobEvent::State {
            job_id: self.job_id,
            seq,
            state,
        });
    }

    fn stage(&self, seq: u64, stage: JobStage, message: &str) {
        self.emit(JobEvent::Stage {
            job_id: self.job_id,
            seq,
            stage,
            message: message.to_string(),
        });
    }

    fn log(&self, seq: u64, level: LogLevel, line: &str) {
        self.emit(JobEvent::Log {
            job_id: self.job_id,
            seq,
            level,
            line: line.to_string(),
        });
    }

    fn metrics(&self, seq: u64, metrics: &ProcessingMetrics) {
        self.with_live(|slot| {
            if let Some(snap) = slot.as_mut() {
                if snap.job_id == self.job_id {
                    snap.metrics = *metrics;
                }
            }
        });
        self.emit(JobEvent::Metrics {
            job_id: self.job_id,
            seq,
            metrics: *metrics,
        });
    }

    fn artifact(&self, seq: u64, artifact: &OutputArtifact) {
        self.with_live(|slot| {
            if let Some(snap) = slot.as_mut() {
                if snap.job_id == self.job_id {
                    snap.artifacts.push(artifact.clone());
                }
            }
        });
        self.emit(JobEvent::Artifact {
            job_id: self.job_id,
            seq,
            artifact: artifact.clone(),
        });
    }

    fn completed(&self, seq: u64, result: &JobResult) {
        self.emit(JobEvent::Completed {
            job_id: self.job_id,
            seq,
            result: Box::new(result.clone()),
        });
        // Terminal: `get_job` should now prefer the persisted history
        // record (`commands::jobs::start_job` records it right around this
        // same point) over a stale live snapshot.
        self.with_live(|slot| {
            if slot.as_ref().map(|s| s.job_id) == Some(self.job_id) {
                *slot = None;
            }
        });
    }
}

/// Decorates any [`JobEventSink`], resolving a one-shot `Receiver<()>` the
/// first time *any* method is called, then delegating to `inner`.
///
/// `jobs::run_job`'s only synchronous failure mode
/// (`ErrorCode::JobAlreadyRunning`) returns before ever calling the sink
/// (`jobs::run::run_job`: `let guard = state.try_acquire(job_id)?;` is the
/// very first line, and every sink call follows it) - so "the receiver
/// resolved" is a reliable, race-free signal that a job genuinely started,
/// usable by `commands::jobs::start_job` to know when it is safe to return
/// `JobAcceptedDto` to the frontend without waiting for the whole run to
/// finish (design-02 §4.2's correlation rule requires listeners to be
/// registered "before `start_job` is invoked", which in turn requires
/// `start_job` to not resolve before the job has actually started).
pub struct FirstEventSignal<S: JobEventSink> {
    inner: S,
    signal: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl<S: JobEventSink> FirstEventSignal<S> {
    pub fn new(inner: S) -> (Self, tokio::sync::oneshot::Receiver<()>) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (
            Self {
                inner,
                signal: Mutex::new(Some(tx)),
            },
            rx,
        )
    }

    fn fire(&self) {
        if let Some(tx) = self.signal.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = tx.send(());
        }
    }
}

impl<S: JobEventSink> JobEventSink for FirstEventSignal<S> {
    fn state(&self, seq: u64, state: JobStatus) {
        self.fire();
        self.inner.state(seq, state);
    }
    fn stage(&self, seq: u64, stage: JobStage, message: &str) {
        self.fire();
        self.inner.stage(seq, stage, message);
    }
    fn log(&self, seq: u64, level: LogLevel, line: &str) {
        self.fire();
        self.inner.log(seq, level, line);
    }
    fn metrics(&self, seq: u64, metrics: &ProcessingMetrics) {
        self.fire();
        self.inner.metrics(seq, metrics);
    }
    fn artifact(&self, seq: u64, artifact: &OutputArtifact) {
        self.fire();
        self.inner.artifact(seq, artifact);
    }
    fn completed(&self, seq: u64, result: &JobResult) {
        self.fire();
        self.inner.completed(seq, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct CountingSink(AtomicUsize);
    impl JobEventSink for CountingSink {
        fn state(&self, _: u64, _: JobStatus) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn stage(&self, _: u64, _: JobStage, _: &str) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn log(&self, _: u64, _: LogLevel, _: &str) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn metrics(&self, _: u64, _: &ProcessingMetrics) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn artifact(&self, _: u64, _: &OutputArtifact) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn completed(&self, _: u64, _: &JobResult) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn job_event_channel_names_match_design_02() {
        let job_id = Uuid::new_v4();
        assert_eq!(
            JobEvent::State {
                job_id,
                seq: 0,
                state: JobStatus::Running
            }
            .channel(),
            "job://state"
        );
        assert_eq!(
            JobEvent::Stage {
                job_id,
                seq: 0,
                stage: JobStage::Preparing,
                message: String::new()
            }
            .channel(),
            "job://stage"
        );
        assert_eq!(
            JobEvent::Log {
                job_id,
                seq: 0,
                level: LogLevel::Info,
                line: String::new()
            }
            .channel(),
            "job://log"
        );
        assert_eq!(
            JobEvent::Artifact {
                job_id,
                seq: 0,
                artifact: crate::domain::OutputArtifact {
                    kind: crate::domain::ArtifactKind::UniqueGames,
                    path: Default::default(),
                    size_bytes: 0,
                }
            }
            .channel(),
            "job://artifact"
        );
    }

    /// Guards the `rename_all_fields` fix documented on `JobEvent`'s own
    /// doc comment: without it, `job_id` would silently serialize as
    /// snake_case while the generated `generated-types.ts` (via
    /// `specta`/`specta-serde`, which reflects real serde behavior rather
    /// than assuming it) would correctly show `jobId` - a real, silent
    /// wire-format mismatch the frontend would never catch at compile time.
    #[test]
    fn job_event_serializes_every_field_as_camel_case_on_the_wire() {
        let json = serde_json::to_string(&JobEvent::State {
            job_id: Uuid::nil(),
            seq: 7,
            state: JobStatus::Running,
        })
        .unwrap();
        assert!(
            json.contains("\"jobId\":"),
            "expected camelCase jobId, got: {json}"
        );
        assert!(
            !json.contains("\"job_id\""),
            "must not contain snake_case job_id, got: {json}"
        );
    }

    #[tokio::test]
    async fn first_event_signal_fires_exactly_once_on_first_call() {
        let (sink, rx) = FirstEventSignal::new(CountingSink::default());
        sink.state(0, JobStatus::Running);
        sink.stage(1, JobStage::Preparing, "x");
        assert_eq!(
            sink.inner.0.load(Ordering::SeqCst),
            2,
            "every call still delegates"
        );
        rx.await.expect("receiver resolves after the first call");
    }

    #[tokio::test]
    async fn first_event_signal_never_fires_when_no_call_is_made() {
        let (sink, rx) = FirstEventSignal::new(CountingSink::default());
        drop(sink);
        assert!(
            rx.await.is_err(),
            "dropping the sink without any call must not resolve the receiver as Ok"
        );
    }
}
