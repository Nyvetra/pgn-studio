// SPDX-License-Identifier: GPL-3.0-or-later
//! The tested static [`EngineCapabilities`] map for the pinned `v26-06`
//! `pgn-extract` build (architecture.md §10.4; design-02 §1.7; Phase 1a task
//! scope section D).
//!
//! Help-text parsing is explicitly *not* the contract (architecture.md
//! §10.4): every boolean below is a hand-verified fact from
//! DECISIONS-LEDGER.md D-007 and design-02 §1.3's source-cited flag table,
//! not something inferred from `--help` output at runtime.

use crate::domain::{EngineCapabilities, EngineIdentity, OutputNotation};
use serde::Deserialize;

/// Defines, for one target triple, the three things that must agree about
/// it: the triple string itself, the `build-info-<triple>.json` embedded at
/// *compile* time, and the panic message naming that file.
///
/// Embedding via `include_str!` rather than a hand-copied string literal is
/// what stops this module drifting from the actual pinned binary's verified
/// identity (DECISIONS-LEDGER.md's "never invent, never placeholder" rule
/// for pin values) - re-pin the engine and this map picks the new identity
/// up on the next build with no edit here.
///
/// A macro rather than three `#[cfg]`-attributed `const` lines because
/// `include_str!` requires a *literal* path, so the triple cannot be
/// selected at runtime and would otherwise have to be spelled out three
/// times per arm. `include_str!(concat!(..))` is fine: `concat!` expands to
/// a literal before `include_str!` sees it.
macro_rules! pinned_target {
    ($triple:literal) => {
        /// The target triple this crate is compiled for, and whose sidecar
        /// and `build-info-<triple>.json` it embeds. Shared with
        /// [`crate::engine::sidecar`] so the triple is stated once.
        pub(crate) const TARGET_TRIPLE: &str = $triple;

        const BUILD_INFO_JSON: &str =
            include_str!(concat!("../../binaries/build-info-", $triple, ".json"));

        const BUILD_INFO_PARSE_ERROR: &str = concat!(
            "src-tauri/binaries/build-info-",
            $triple,
            ".json must be valid JSON with triple/sha256/engineVersion string fields \
             — if the engine-build tooling changed this file's shape, update BuildInfo to match"
        );
    };
}

// Gated on target_os AND target_arch: `target_arch = "x86_64"` alone
// matches both Windows and Intel macOS. These are exactly the three
// toolchains engine-src/upstream.lock declares and the two build scripts
// know how to produce.
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pinned_target!("x86_64-pc-windows-msvc");
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pinned_target!("aarch64-apple-darwin");
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pinned_target!("x86_64-apple-darwin");

// Without this arm an unsupported target fails with a bare "cannot find
// value `BUILD_INFO_JSON`", which says nothing about what is actually
// missing. Adding a target means adding an upstream.lock toolchain entry
// and a build-script branch too, not just a cfg arm here.
#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
)))]
compile_error!(
    "PGN Studio builds a pgn-extract sidecar only for the three triples in \
     engine-src/upstream.lock's `toolchains` map: x86_64-pc-windows-msvc, \
     aarch64-apple-darwin, x86_64-apple-darwin. Add a lock toolchain entry, a \
     scripts/build-pgn-extract.* branch, and a cfg arm in \
     src-tauri/src/engine/capability.rs before building for another target."
);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildInfo {
    triple: String,
    sha256: String,
    engine_version: String,
}

fn pinned_identity() -> EngineIdentity {
    let info: BuildInfo = serde_json::from_str(BUILD_INFO_JSON).expect(BUILD_INFO_PARSE_ERROR);
    EngineIdentity {
        version: info.engine_version,
        sha256: info.sha256,
        target_triple: info.triple,
    }
}

/// The capability map for the pinned `v26-06` build.
///
/// Every `true`/`false` here is cited against DECISIONS-LEDGER.md D-007 and
/// design-02 §1.3, not guessed:
/// - `duplicate_detection`, `duplicate_audit_file`, `external_duplicate_table`,
///   `check_file`, `eco_classification`, `fen_patterns`, `textual_variations`,
///   `fix_result_tags`, `reject_bad_results`: all verified present
///   (design-02 §1.3 rows 2-5, 11, 16, 19-21, 23; `argsfile.c` citations
///   therein).
/// - `separate_broken_output`: **must stay `false`.** Empirically verified
///   impossible in one pass (DECISIONS-LEDGER.md D-007 V-5): without
///   `--keepbroken` broken games are dropped everywhere; with it they land
///   in the *main* output. There is no flag that routes them to their own
///   file. `BrokenOutput` (domain) has no variant that could even request
///   this, so no compiler code path needs to consult this field today — it
///   is kept `false` and documented so a future capability consumer never
///   assumes otherwise.
/// - `supported_output_formats`: `[San]` only (Decision D-13). The engine's
///   default output is SAN and V1 emits no `-W` token at all.
/// - `unicode_paths`: see the field-level comment below — this is the one
///   value in this struct that this static map cannot honestly assert.
pub fn pinned_v26_06() -> EngineCapabilities {
    EngineCapabilities {
        identity: pinned_identity(),
        duplicate_detection: true,
        duplicate_audit_file: true,
        external_duplicate_table: true,
        check_file: true,
        eco_classification: true,
        fen_patterns: true,
        textual_variations: true,
        fix_result_tags: true,
        reject_bad_results: true,
        separate_broken_output: false,
        supported_output_formats: vec![OutputNotation::San],
        // Conservatively `false`. Design-02 Decision D-3 makes this ONE
        // field genuinely runtime-derived, not static: it is set "from the
        // startup [Unicode-path] probe", which actually launches the pinned
        // sidecar against a non-ASCII fixture path and observes whether it
        // succeeds (architecture.md §10.4; design-02 §1.7 item (d)). Phase
        // 1a implements no process spawning at all, so no such probe has
        // been run by this code. `false` is the safe default (it can only
        // make `validate_job` *more* conservative, per design-02 §3.2 step
        // 3 — never silently accept a path the running binary might not
        // actually be able to open).
        //
        // Context for whoever wires up that probe: the sidecar does embed
        // the required `activeCodePage=UTF-8` manifest fragment
        // (`engine-src/manifest/pgn-extract.manifest`), whose own comment
        // records an informal Phase 0b verification against
        // `fixtures/unicode-paths/`. That is encouraging but is a
        // development-time, human-run check on one build, not the
        // per-launch automated probe design-02 specifies — it does not
        // license hardcoding `true` here.
        unicode_paths: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separate_broken_output_is_false() {
        assert!(!pinned_v26_06().separate_broken_output);
    }

    #[test]
    fn supported_output_formats_is_exactly_san() {
        assert_eq!(
            pinned_v26_06().supported_output_formats,
            vec![OutputNotation::San]
        );
    }

    #[test]
    fn identity_matches_the_pinned_build_info_file() {
        let caps = pinned_v26_06();
        assert_eq!(caps.identity.version, "v26-06");
        // Against TARGET_TRIPLE, not a Windows literal: this asserts the
        // *agreement* between the embedded build-info and the triple this
        // crate was compiled for, which is the property that matters and
        // the one that would catch a mismatched sidecar. A hardcoded
        // literal would simply fail on macOS while proving nothing extra
        // on Windows.
        assert_eq!(caps.identity.target_triple, TARGET_TRIPLE);
        assert_eq!(
            caps.identity.sha256.len(),
            64,
            "sha256 must be 64 hex chars"
        );
        assert!(
            caps.identity.sha256.chars().all(|c| c.is_ascii_hexdigit()),
            "sha256 must be hex"
        );
    }

    #[test]
    fn v1_capabilities_are_all_true_except_separate_broken_output() {
        let caps = pinned_v26_06();
        assert!(caps.duplicate_detection);
        assert!(caps.duplicate_audit_file);
        assert!(caps.external_duplicate_table);
        assert!(caps.check_file);
        assert!(caps.eco_classification);
        assert!(caps.fen_patterns);
        assert!(caps.textual_variations);
        assert!(caps.fix_result_tags);
        assert!(caps.reject_bad_results);
    }
}
