// SPDX-License-Identifier: GPL-3.0-or-later
//! Job lifecycle management (architecture.md §9.1, §10.9, §10.10; design-02
//! §2): the single-job-at-a-time state machine, per-job process
//! spawning/streaming/cancellation, and the top-level orchestration that
//! turns a validated [`crate::domain::JobSpec`] into a
//! [`crate::domain::JobResult`].
//!
//! This module is Phase 1b's "clean Rust API the command layer will call"
//! for `start_job`/`cancel_job` (design-02 §4.1) - it does not itself
//! define any `#[tauri::command]` (that is Phase 2's `commands/` module).

pub mod events;
pub mod run;

mod process;
#[cfg(windows)]
mod windows_job_object;

pub use events::{JobEventSink, JobStage, LogLevel, NullEventSink};
pub use run::{run_job, RunJobContext};

use uuid::Uuid;

use crate::domain::PublicError;
use crate::errors;

/// Single-flight guard (architecture.md §19.3: "Version 1 permits one
/// active engine process"; design-02 §2.6). Exactly one job may be
/// `Running`/`Cancelling` at a time; auxiliary work (hashing, validation)
/// never takes this slot.
///
/// Uses a plain [`std::sync::Mutex`] rather than `tokio::sync::Mutex`
/// (design-02's own illustrative sketch shows the latter) so that the
/// release half of the guarantee ([`SlotGuard`]'s `Drop` impl) can run
/// synchronously on every exit path - success, failure, cancellation,
/// *and panic* - without needing an async `Drop`, which Rust does not
/// have. The critical section is always a few field reads/writes, never
/// held across an `.await`, so a synchronous mutex is the right tool: it
/// still gives the atomic "check-and-claim" that prevents two concurrent
/// `run_job` calls from both believing the slot is free (design-02: "spawn
/// happens while holding the lock → no TOCTOU" - here, *claiming* the slot
/// is what happens atomically; the actual OS-level process spawn happens
/// afterward, which is sufficient: no second caller can ever observe the
/// slot as free while a first caller's job is genuinely active).
pub struct AppState {
    active: std::sync::Mutex<Option<ActiveSlot>>,
}

struct ActiveSlot {
    job_id: Uuid,
    cancel_tx: tokio::sync::watch::Sender<bool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active: std::sync::Mutex::new(None),
        }
    }

    /// The currently running job's id, if any (for `get_job`/UI display -
    /// Phase 2 concern, exposed here since it is cheap and harmless to
    /// offer now).
    pub fn active_job_id(&self) -> Option<Uuid> {
        self.active
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|a| a.job_id)
    }

    /// Attempts to claim the single-flight slot for `job_id`. On success,
    /// the returned [`SlotGuard`] must be held for the entire duration of
    /// the run - dropping it (including via an early `return`, `?`, or
    /// panic-driven unwind) releases the slot.
    pub fn try_acquire(&self, job_id: Uuid) -> Result<SlotGuard<'_>, PublicError> {
        let mut slot = self.active.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = slot.as_ref() {
            return Err(errors::job_already_running(existing.job_id));
        }
        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        *slot = Some(ActiveSlot { job_id, cancel_tx });
        Ok(SlotGuard {
            state: self,
            job_id,
            cancel_rx,
        })
    }

    /// `cancel_job(job_id)` (design-02 §2.5 step 1). Mismatched/absent ids
    /// map to `JOB_NOT_ACTIVE`-as-`INVALID_JOB_SPEC` per design-02's own
    /// explicit instruction (see `errors::job_not_active`).
    pub fn request_cancel(&self, job_id: Uuid) -> Result<(), PublicError> {
        let slot = self.active.lock().unwrap_or_else(|p| p.into_inner());
        match slot.as_ref() {
            Some(active) if active.job_id == job_id => {
                // `send` always notifies waiters, regardless of whether the
                // new value differs from the old one - exactly one
                // cancellation is ever requested per job, so this is
                // unconditionally correct.
                let _ = active.cancel_tx.send(true);
                Ok(())
            }
            Some(active) => Err(errors::job_not_active(job_id, Some(active.job_id))),
            None => Err(errors::job_not_active(job_id, None)),
        }
    }
}

/// RAII handle to the claimed single-flight slot. `job_id`'s job is the
/// only one that may run until this is dropped.
pub struct SlotGuard<'a> {
    state: &'a AppState,
    job_id: Uuid,
    cancel_rx: tokio::sync::watch::Receiver<bool>,
}

impl SlotGuard<'_> {
    pub fn job_id(&self) -> Uuid {
        self.job_id
    }

    /// A cloned receiver for `run_job`'s cancellation-observing select
    /// loop - cloning a `watch::Receiver` is cheap and every clone
    /// observes the same underlying value.
    pub fn cancel_receiver(&self) -> tokio::sync::watch::Receiver<bool> {
        self.cancel_rx.clone()
    }
}

impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        let mut slot = self.state.active.lock().unwrap_or_else(|p| p.into_inner());
        if slot.as_ref().map(|a| a.job_id) == Some(self.job_id) {
            *slot = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_is_rejected_while_first_guard_is_held() {
        let state = AppState::new();
        let job_a = Uuid::new_v4();
        let job_b = Uuid::new_v4();
        let guard_a = state.try_acquire(job_a).unwrap();
        let err = match state.try_acquire(job_b) {
            Err(e) => e,
            Ok(_) => panic!("expected JOB_ALREADY_RUNNING"),
        };
        assert_eq!(err.code(), crate::domain::ErrorCode::JobAlreadyRunning);
        assert_eq!(state.active_job_id(), Some(job_a));
        drop(guard_a);
    }

    #[test]
    fn dropping_the_guard_releases_the_slot() {
        let state = AppState::new();
        let job_a = Uuid::new_v4();
        {
            let _guard = state.try_acquire(job_a).unwrap();
            assert_eq!(state.active_job_id(), Some(job_a));
        }
        assert_eq!(state.active_job_id(), None);
        // A second job can now acquire the slot.
        let job_b = Uuid::new_v4();
        let _guard_b = state.try_acquire(job_b).unwrap();
        assert_eq!(state.active_job_id(), Some(job_b));
    }

    #[test]
    fn guard_releases_even_when_dropped_via_panic_unwind() {
        let state = std::sync::Arc::new(AppState::new());
        let job_a = Uuid::new_v4();
        let state_clone = state.clone();
        let result = std::panic::catch_unwind(move || {
            let _guard = state_clone.try_acquire(job_a).unwrap();
            panic!("simulated failure mid-job");
        });
        assert!(result.is_err());
        assert_eq!(
            state.active_job_id(),
            None,
            "the slot must be released even after a panic unwinds through the guard"
        );
    }

    #[test]
    fn cancel_unknown_job_id_is_rejected() {
        let state = AppState::new();
        let err = state.request_cancel(Uuid::new_v4()).unwrap_err();
        assert_eq!(err.code(), crate::domain::ErrorCode::InvalidJobSpec);
    }

    #[test]
    fn cancel_mismatched_job_id_is_rejected_and_does_not_cancel_the_active_job() {
        let state = AppState::new();
        let active_job = Uuid::new_v4();
        let guard = state.try_acquire(active_job).unwrap();
        let mut cancel_rx = guard.cancel_receiver();

        let err = state.request_cancel(Uuid::new_v4()).unwrap_err();
        assert_eq!(err.code(), crate::domain::ErrorCode::InvalidJobSpec);
        assert!(!*cancel_rx.borrow_and_update());
    }

    #[test]
    fn cancel_matching_job_id_notifies_the_receiver() {
        let state = AppState::new();
        let active_job = Uuid::new_v4();
        let guard = state.try_acquire(active_job).unwrap();
        let mut cancel_rx = guard.cancel_receiver();

        state.request_cancel(active_job).unwrap();
        assert!(*cancel_rx.borrow_and_update());
    }
}
