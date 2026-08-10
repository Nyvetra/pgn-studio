// SPDX-License-Identifier: GPL-3.0-or-later
// `PublicError` (errors/, domain::result) is deliberately a rich,
// redaction-safe struct (code + title + message + remediation + log_path +
// technical_id) per architecture.md §18.2/design-02 §5.2 - that richness is
// the point (it is what lets a `technical_id` join a safe user-facing
// message to full internal detail in the local log), not an oversight to
// fix by boxing. Every `Result<_, PublicError>` in this crate is a cold
// error path (job setup/validation/cancellation), never a hot loop, so the
// stack-size micro-optimization `clippy::result_large_err` exists for does
// not apply here; boxing it at every one of the many call sites across
// `errors/`, `jobs/`, `filesystem/`, `application/`, and `commands/` would
// add indirection without a measurable benefit. Same reasoning covers
// `filesystem::publish::PublishFailure`, which is intentionally rich for
// the same reason (architecture.md §18.3's "never claim deletion that did
// not happen").
#![allow(clippy::result_large_err)]
//! PGN Studio Tauri application library.
//!
//! Phase 1a implemented the domain model ([`domain`]) and the pure
//! `pgn-extract` command compiler ([`engine::command_compiler`]). Phase 1b
//! added job orchestration ([`jobs`]), filesystem safety ([`filesystem`]),
//! the public error taxonomy ([`errors`]), and engine sidecar
//! resolution/self-test ([`engine::sidecar`]). Phase 2a adds the Tauri IPC
//! layer: typed commands ([`commands`]), the job event stream
//! ([`application::events`]), settings/history persistence
//! ([`persistence`]), and the application-wiring/orchestration layer
//! ([`application`]) that ties all of the above together behind
//! [`AppContext`](application::AppContext).

pub mod application;
pub mod commands;
pub mod domain;
pub mod engine;
pub mod errors;
pub mod filesystem;
pub mod jobs;
pub mod persistence;

use tauri::Manager;

/// Where the generated TypeScript bindings live, relative to this crate's
/// own manifest directory (`src-tauri/`) - resolved via
/// `CARGO_MANIFEST_DIR` rather than a relative runtime path so both this
/// debug-build auto-export and the `--export-bindings` CLI mode (see
/// [`run`]'s doc comment) write to the exact same file regardless of the
/// process's current working directory.
const GENERATED_TYPES_RELATIVE_PATH: &str = "../src/ipc/generated-types.ts";

/// The CLI flag `xtask export-bindings` shells out to (see
/// `xtask/src/main.rs`) - runs *this* binary (`pgn-studio[.exe]`) with this
/// single argument instead of building a real window.
const EXPORT_BINDINGS_FLAG: &str = "--export-bindings";

fn export_bindings_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GENERATED_TYPES_RELATIVE_PATH)
}

fn export_bindings() -> Result<(), String> {
    let out_path = export_bindings_path();
    commands::specta_builder()
        .export(specta_typescript::Typescript::default(), &out_path)
        .map_err(|e| format!("failed to export TypeScript bindings to {out_path:?}: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Headless bindings-export mode (design-02 §4.3/D-17): `xtask`
    // (`cargo run -p xtask -- export-bindings`) spawns *this compiled
    // binary* with `--export-bindings` rather than linking `pgn_studio_lib`
    // into a second binary crate directly. This works around a confirmed,
    // unresolved upstream Windows-only bug (tauri-apps/tauri#13948): a
    // second binary crate in the same Cargo workspace that imports a Tauri
    // app crate as a library dependency crashes at process startup with
    // `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) before `main` even runs -
    // empirically confirmed on this machine (`cargo run -p xtask` with
    // `pgn-studio` as a path dependency reproduced exactly this crash; the
    // normally-built `pgn-studio.exe` run standalone does not). Running the
    // real, already-correctly-linked app binary as a *subprocess* sidesteps
    // the broken direct-link scenario entirely, and this early return means
    // no `tauri::Builder`/window/plugin is ever constructed for this mode.
    if std::env::args().any(|a| a == EXPORT_BINDINGS_FLAG) {
        match export_bindings() {
            Ok(()) => println!("wrote {}", export_bindings_path().display()),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let specta_builder = commands::specta_builder();

    // Auto-regenerate on every debug run (the upstream tauri-specta
    // pattern - see its own README/lib.rs doc example), purely for
    // developer convenience during `cargo tauri dev`: if nothing changed,
    // this rewrites byte-identical content (no spurious git diff); if
    // something did, drift is visible immediately instead of only at CI.
    // `xtask export-bindings` above is the mechanism CI actually depends
    // on, since CI never runs a full GUI app to reach this code path.
    #[cfg(debug_assertions)]
    if let Err(e) = export_bindings() {
        eprintln!("warning: {e}");
    }

    tauri::Builder::default()
        // Phase 2a: native file/folder pickers (`select_input_files` etc.)
        // and "reveal in file manager"/"open with default app"
        // (`reveal_path`/`open_path`) - both called only from Rust command
        // handlers, never directly by the frontend (architecture.md §16.2).
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            let handle = app.handle().clone();
            // `application::startup::initialize` does real async I/O
            // (engine two-gate verify + self-test + Unicode probe, plus the
            // startup workspace sweep) - `setup` itself is a synchronous
            // hook that must finish before the window is shown, so it is
            // run to completion here via Tauri's own async-runtime bridge
            // rather than fire-and-forget spawned (engine-dependent
            // commands would otherwise race the app's own startup).
            let ctx = tauri::async_runtime::block_on(application::startup::initialize(&handle));
            app.manage(ctx);
            // Required for `job://*` events to resolve their registered
            // type metadata (`tauri_specta::Builder::mount_events`'s own
            // doc comment) - harmless here even though `JobEvent` is
            // emitted via plain `tauri::Emitter::emit`, not this builder's
            // own `Event` trait machinery (see `application::events`).
            specta_builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
