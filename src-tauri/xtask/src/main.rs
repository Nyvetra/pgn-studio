// SPDX-License-Identifier: GPL-3.0-or-later
//! Developer tasks for PGN Studio.
//!
//! `cargo run -p xtask -- export-bindings` regenerates
//! `src/ipc/generated-types.ts` from the tauri-specta-annotated command and
//! event surface defined in `pgn_studio_lib::commands` (design-02 §4.3,
//! decision D-17). This is also what CI runs (`.github/workflows/rust.yml`)
//! before diffing the file to catch drift between the Rust source of truth
//! and the committed TypeScript.
//!
//! **Why this shells out instead of linking `pgn_studio_lib` directly:**
//! an earlier version of this crate depended on `pgn-studio`/`tauri`/
//! `tauri-specta` and called `pgn_studio_lib::commands::specta_builder()`
//! in-process. That reproduced a confirmed, unresolved upstream
//! Windows-only bug (tauri-apps/tauri#13948, "STATUS_ENTRYPOINT_NOT_FOUND
//! on windows when running app imported into a crate as a library"): a
//! second binary crate in the same Cargo workspace that imports a Tauri
//! app crate as a library dependency crashes at process startup with
//! `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) before `main` even runs -
//! reproduced empirically on this machine. The real `pgn-studio` binary,
//! run standalone, does not have this problem (verified: it starts and
//! stays running normally). So this task instead runs `pgn-studio` itself,
//! as a subprocess, with a `--export-bindings` flag `src/lib.rs::run`
//! checks for before ever constructing a `tauri::Builder`/window -
//! sidestepping the broken direct-link scenario entirely.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    // This crate's own manifest dir is `src-tauri/xtask`; the workspace
    // root (where `cargo run --bin pgn-studio` must be invoked from) is its
    // parent, `src-tauri`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is always nested one level under the workspace root")
        .to_path_buf()
}

fn export_bindings() {
    let status = Command::new(env!("CARGO"))
        .args([
            "run",
            "--quiet",
            "--bin",
            "pgn-studio",
            "--",
            "--export-bindings",
        ])
        .current_dir(workspace_root())
        .status()
        .expect("failed to spawn `cargo run --bin pgn-studio -- --export-bindings`");
    if !status.success() {
        eprintln!("export-bindings failed: pgn-studio exited with {status}");
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn print_usage() {
    eprintln!("Usage: cargo run -p xtask -- <task>");
    eprintln!();
    eprintln!("Tasks:");
    eprintln!("  export-bindings    Regenerate src/ipc/generated-types.ts");
}

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("export-bindings") => export_bindings(),
        Some(other) => {
            eprintln!("error: unknown task {other:?}");
            print_usage();
            std::process::exit(1);
        }
        None => {
            print_usage();
            std::process::exit(1);
        }
    }
}
