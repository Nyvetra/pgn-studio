// SPDX-License-Identifier: GPL-3.0-or-later
//! Application/orchestration layer (architecture.md §7.1): coordinates the
//! domain model, the engine port, the filesystem port, `jobs::run_job`, and
//! the persistence stores. `commands/` handlers are thin callers into this
//! layer (and, for the simplest pass-throughs like `get_settings`, directly
//! into `persistence::*`'s own small trait methods) - business logic lives
//! here, not in a `#[tauri::command]` body.

pub mod context;
pub mod events;
pub mod inputs;
pub mod jobs;
pub mod paths;
pub mod startup;

pub use context::AppContext;

use crate::domain::PublicError;

/// Runs a blocking closure on Tokio's blocking-thread pool and maps a join
/// failure (panic inside the closure) onto the pre-agreed
/// `UNKNOWN_INTERNAL_ERROR` escape hatch, so every `application::` function
/// that wraps synchronous filesystem work (`filesystem::validate::
/// validate_job` and `engine::command_compiler::compile` are both
/// documented as blocking/pure-sync, never `async fn`, by their own design)
/// can stay a plain `async fn` without duplicating this mapping at every
/// call site (architecture.md §19.4: filesystem scanning must not run on
/// the async/UI-adjacent thread).
pub(crate) async fn run_blocking<T, F>(f: F) -> Result<T, PublicError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|join_err| {
        #[allow(deprecated)]
        crate::errors::unknown_internal_error(anyhow::anyhow!(
            "a background task panicked: {join_err}"
        ))
    })
}
