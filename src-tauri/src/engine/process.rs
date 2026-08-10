// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared "never a shell, never ambient state" process-spawn primitives
//! (architecture.md §10.3, §16.2; design-02 §2.2), used by both the sidecar
//! self-test/version-probe ([`super::sidecar`]) and the real job runner
//! (`jobs::process`), so both are provably built from the same base
//! configuration.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use super::EngineExecutable;

/// `CREATE_NO_WINDOW` (`0x0800_0000`) - passed as a raw flag value (not via
/// `windows-sys`) because `tokio::process::Command::creation_flags` just
/// takes a `u32`; no FFI type is needed for this one constant.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Builds a `tokio::process::Command` for the bundled engine with every
/// rule from design-02 §2.2 applied: argument array (never a shell),
/// working directory set explicitly, `stdin` nulled (T-8 belt-and-braces),
/// both output streams piped (and captured **separately** - never merged),
/// `ECO_FILE` stripped from the environment (row 16), the child reaped on
/// drop, no console window on Windows, and its own process group on Unix
/// (so a process-group signal can target it without hitting the parent).
pub(crate) fn build_command(
    engine: &EngineExecutable,
    args: &[OsString],
    working_directory: &Path,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(engine.path());
    cmd.args(args)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("ECO_FILE")
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        // SAFETY: `setsid()` is async-signal-safe and is the only thing
        // this closure does; it runs in the child after `fork`, before
        // `exec`, exactly as `pre_exec`'s contract requires.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    cmd
}

/// Buffered result of running the engine to completion. Only for small,
/// bounded outputs (the capability self-test's tiny fixtures, `--version`) -
/// the real job runner never uses this (design-02 §2.3: "Never hold the
/// whole log in memory"); it drains both pipes concurrently into a bounded
/// channel plus an on-disk log instead (see `jobs::process`).
#[derive(Debug)]
pub(crate) struct CapturedRun {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) async fn run_to_completion(
    engine: &EngineExecutable,
    args: &[OsString],
    working_directory: &Path,
) -> std::io::Result<CapturedRun> {
    // `Command::output()` spawns and drains both pipes concurrently
    // internally (it does not suffer the 64 KiB pipe-buffer deadlock this
    // codebase is otherwise careful about) and awaits exit - exactly right
    // for the tiny, bounded outputs this function is for.
    let output = build_command(engine, args, working_directory)
        .output()
        .await?;
    Ok(CapturedRun {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
