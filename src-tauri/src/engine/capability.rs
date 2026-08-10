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

/// The build-info JSON the engine-build workstream writes for the pinned
/// Windows sidecar (`scripts/build-pgn-extract.ps1`). Embedded at *compile*
/// time via `include_str!` rather than copied by hand into a string
/// literal, specifically so this module can never drift from the actual
/// pinned binary's verified identity (DECISIONS-LEDGER.md's repeated
/// "never invent, never placeholder" rule for pin values) — if that
/// workstream re-pins the binary, this map picks up the new identity on the
/// next build with no manual edit required here.
///
/// Only the Windows x86_64 build exists at the time of this task
/// (DECISIONS-LEDGER.md D-006: macOS is out of scope, no Mac available).
/// A future macOS build would need its own `build-info-*.json` and this
/// function would need to select by `cfg!(target_os = ..)` / target triple.
const BUILD_INFO_JSON: &str = include_str!("../../binaries/build-info-x86_64-pc-windows-msvc.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildInfo {
    triple: String,
    sha256: String,
    engine_version: String,
}

fn pinned_identity() -> EngineIdentity {
    let info: BuildInfo = serde_json::from_str(BUILD_INFO_JSON).expect(
        "src-tauri/binaries/build-info-x86_64-pc-windows-msvc.json must be valid JSON \
         with triple/sha256/engineVersion string fields — if the engine-build tooling \
         changed this file's shape, update BuildInfo to match",
    );
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
        assert_eq!(caps.identity.target_triple, "x86_64-pc-windows-msvc");
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
