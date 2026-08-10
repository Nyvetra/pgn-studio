// SPDX-License-Identifier: GPL-3.0-or-later
//! The `pgn-extract` engine adapter (architecture.md §7.1, §10).
//!
//! Phase 1a implemented the pure command compiler ([`command_compiler`]),
//! criteria file rendering ([`criteria`]), and the static capability map
//! ([`capability`]) for the pinned `v26-06` build. Phase 1b adds sidecar
//! path resolution, the two-gate integrity check, the startup self-test,
//! and the Unicode-path capability probe ([`sidecar`]), plus the shared
//! no-shell process-spawn primitives ([`process`]) that both the self-test
//! and the real job runner (`jobs::process`) build on.

pub mod capability;
pub mod command_compiler;
pub mod criteria;
pub(crate) mod process;
pub mod sidecar;

/// Golden command tests (design-02 §1.8, G-1..G-12; task section E).
///
/// These live inside the crate (as a `#[cfg(test)]` module) rather than in
/// `src-tauri/tests/` as a normal external integration test, because they
/// need `EngineExecutable::new_unverified`, which is deliberately
/// `pub(crate)` — see that constructor's doc comment. An external
/// integration test crate cannot see `pub(crate)` items at all, so the
/// choice was either weaken that visibility (and the guarantee it encodes)
/// or keep these tests internal; the guarantee was judged more important
/// than matching `tests/smoke_test.rs`'s external-crate pattern.
#[cfg(test)]
mod golden_tests;

use std::path::{Path, PathBuf};

/// A verified path to the `pgn-extract` sidecar executable.
///
/// Never a raw [`PathBuf`] in any public API (design-02 §1.6): the intent is
/// that the *only* way to obtain one is the verified sidecar resolver
/// ([`sidecar::resolve_and_verify`]), which checks the file's SHA-256
/// against the pinned identity and spawns `--version` before handing out an
/// instance (design-02 §2.2: "the resolver asserts the sidecar filename
/// ends in `.exe`... and matches the pinned SHA-256 before the path can
/// become an `EngineExecutable`").
///
/// [`EngineExecutable::new_unverified`] is a crate-private escape hatch that
/// exists solely so the Phase 1a pure compiler — and its golden tests — can
/// construct a [`command_compiler::CompiledEngineCommand`] without a real
/// sidecar on disk. It must not become `pub`, and production code must
/// always go through [`sidecar::resolve_and_verify`] instead, or the "only a
/// verified path can become one of these" guarantee this type exists to
/// provide would be silently broken.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EngineExecutable(PathBuf);

impl EngineExecutable {
    /// Builds an `EngineExecutable` without verifying sidecar identity.
    ///
    /// `#[cfg(test)]`-gated so ordinary (non-test) builds stay free of a
    /// dead-code warning and so it is structurally impossible for
    /// production code to reach for this instead of the real resolver.
    #[cfg(test)]
    pub(crate) fn new_unverified(path: PathBuf) -> Self {
        Self(path)
    }

    /// Builds an `EngineExecutable` after both integrity gates have passed
    /// (design-02 §1.7's "two-gate" identity check). `pub(crate)` and
    /// reserved for [`sidecar::resolve_and_verify`] - see the type-level
    /// doc comment.
    pub(crate) fn new_verified(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}
