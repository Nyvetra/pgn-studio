// SPDX-License-Identifier: GPL-3.0-or-later
//! Platform-specific primitives filesystem safety needs and `std` does not
//! provide: no-replace atomic rename, free-disk-space query, and the
//! active-code-page representability probe (design-02 §3.2 step 3, §3.4
//! step 6).
//!
//! **Verification note (binding, see the crate-level report):** this
//! development machine is Windows-only (`<env>`: `Platform: win32`); the
//! `windows` submodule is compiled, `cargo check`-verified, and exercised
//! by this crate's own tests. The `unix` submodule cannot be compiled or
//! tested here at all (`#[cfg(unix)]` code is invisible to `cargo
//! check`/`clippy` on a Windows host target) - it is written directly
//! against `libc` crate source read for this task (exact signatures for
//! `renameat2`, `renamex_np`, `statvfs`, `setsid`, `kill`, `killpg` were
//! confirmed against the vendored `libc` source, not guessed), matching
//! the precedent DECISIONS-LEDGER.md D-006 already sets for macOS ("must be
//! reported honestly as unverified, never as passing"). Treat it the same
//! way: written in good faith, unverified by compilation.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::*;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::*;

/// Common shape for the one rename outcome filesystem safety cares about
/// distinguishing from every other I/O failure: "the destination already
/// exists" (§3.4's no-replace guarantee) vs. anything else.
#[derive(Debug)]
pub(crate) enum RenameError {
    AlreadyExists,
    Io(std::io::Error),
}

impl std::fmt::Display for RenameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenameError::AlreadyExists => write!(f, "destination already exists"),
            RenameError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RenameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RenameError::AlreadyExists => None,
            RenameError::Io(e) => Some(e),
        }
    }
}
