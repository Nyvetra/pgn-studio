# application/

Application/orchestration layer (architecture.md §7.1): coordinates the
domain model, the engine port, the filesystem port, and the history/settings
port. This is where job validation, job lifecycle transitions
(`Draft -> Validating -> Ready -> Running -> ...`, architecture.md §9.1),
and cancellation orchestration live.

Commands in `commands/` are thin callers into this layer, not a
reimplementation of it.

Implemented in Phase 2a:
- `context.rs` - `AppContext`, the single piece of Tauri-managed state every
  command reads (verified engine bundle or startup failure, the Phase 1b
  single-flight job guard, settings/history stores, live-job snapshot).
- `startup.rs` - the one-time startup sequence (`engine::sidecar::
  startup_check`, the interrupted-workspace sweep, settings/history load)
  that builds an `AppContext`.
- `events.rs` - `JobEvent` (the closed `job://*` wire union) and
  `TauriJobEventSink`, implementing Phase 1b's `jobs::JobEventSink` seam by
  emitting real Tauri events and mirroring live progress into
  `AppContext::live_job`.
- `jobs.rs` - orchestration for `validate_job`/`compile_job_preview`/
  `start_job`/`cancel_job`/`get_job`/`list_recent_jobs`/`delete_job_history`.
- `inputs.rs` - `inspect_inputs`'s bounded-concurrency filesystem probing.
- `paths.rs` - the `reveal_path`/`open_path` allowlist check.
