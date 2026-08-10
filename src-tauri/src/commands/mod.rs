// SPDX-License-Identifier: GPL-3.0-or-later
//! Tauri `#[tauri::command]` entry points (design-02 §4.1; architecture.md
//! §14.1). Every command here is `async`, returns `Result<T, PublicError>`
//! (`PublicError` is exactly design-02 §4.1's `PublicErrorDto` - see this
//! crate's Phase 2a report for why there is no separate `Dto`-suffixed
//! mirror), and stays a thin wrapper that delegates to `application::` -
//! see each submodule's own doc comment.
//!
//! [`specta_builder`] is the single source of truth for the exported
//! command/event/type surface: `lib.rs::run()` calls it both for the real
//! app (`.invoke_handler(...)`) and for its own headless
//! `--export-bindings` mode (`.export(...)`, no window constructed - see
//! `run`'s doc comment for why `xtask export-bindings` shells out to this
//! same binary rather than linking this crate directly), so the generated
//! TypeScript can never drift from what is actually registered (design-02
//! §4.3/D-17).

pub mod app;
pub mod dialogs;
pub mod dto;
pub mod engine;
pub mod inputs;
pub mod jobs;
pub mod settings;

use tauri_specta::{collect_commands, Builder};

use crate::application::events::JobEvent;

/// Builds (but does not export or mount) the full tauri-specta command/type
/// registry. `tauri::Wry` is this app's only real runtime - see
/// `application::jobs::start_job`'s use of the same default via bare
/// `tauri::AppHandle` for the same reasoning.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        // specta-typescript forbids exporting u64/i64/u128/i128/usize/isize
        // as TypeScript `number` by default (they can silently lose
        // precision above 2^53). This crate's u64 fields are exclusively
        // byte counts and game counts (`ProcessingMetrics`,
        // `OutputArtifact::size_bytes`, `InputInspectionDto::size_bytes`,
        // job `seq` counters); none of those realistically approach 2^53
        // (9 PB / 9 quadrillion games) for a desktop PGN tool, so the
        // documented opt-in cast is the pragmatic choice here over
        // switching every count/size field to a `String`-encoded bigint
        // wire type. See `Builder::dangerously_cast_bigints_to_number`'s
        // own doc comment for the general caveat.
        .dangerously_cast_bigints_to_number()
        .commands(collect_commands![
            app::get_app_info,
            engine::get_engine_info,
            engine::get_engine_capabilities,
            dialogs::select_input_files,
            dialogs::select_input_directory,
            dialogs::select_output_directory,
            dialogs::reveal_path,
            dialogs::open_path,
            inputs::inspect_inputs,
            inputs::scan_input_directory,
            jobs::validate_job,
            jobs::compile_job_preview,
            jobs::start_job,
            jobs::cancel_job,
            jobs::get_job,
            jobs::list_recent_jobs,
            jobs::delete_job_history,
            jobs::export_job_manifest,
            settings::get_settings,
            settings::update_settings,
        ])
        // `JobEvent` is not registered via `tauri_specta`'s own
        // `Event`/`collect_events!` machinery - see `application::events`'s
        // module doc comment for why (one closed union, six channel names,
        // which that machinery's one-type-one-channel model does not fit).
        // `.typ::<JobEvent>()` still gets the type itself into
        // `generated-types.ts` for `src/ipc/events.ts`'s hand-written
        // listen wrappers to import.
        .typ::<JobEvent>()
}
