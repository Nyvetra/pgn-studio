// SPDX-License-Identifier: GPL-3.0-or-later
//! [`EngineCapabilities`] and [`EngineIdentity`] (architecture.md §10.4;
//! design-02 §1.7).
//!
//! These are domain types — the *shape* of "what can this engine build do" —
//! deliberately kept independent of any particular pinned version so the
//! compiler stays engine-version-agnostic (architecture.md §7.1). The
//! concrete, tested static map for the pinned `v26-06` build lives in
//! `engine::capability`, which constructs an [`EngineCapabilities`] value
//! from here; that separation is what section D of the Phase 1a task scope
//! ("static capability map") describes.

use serde::{Deserialize, Serialize};

use super::OutputNotation;

/// Identity of a verified `pgn-extract` sidecar build (design-02 §1.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineIdentity {
    /// The engine's own `CURRENT_VERSION` string, e.g. `"v26-06"`.
    pub version: String,
    /// SHA-256 of the sidecar executable, lowercase hex.
    pub sha256: String,
    /// Rust target triple the sidecar was built for, e.g.
    /// `"x86_64-pc-windows-msvc"`.
    pub target_triple: String,
}

/// What a specific pinned `pgn-extract` build can do (architecture.md §10.4;
/// design-02 §1.7).
///
/// `false` on any field is a hard capability gate: the compiler returns
/// `CompileError::UnsupportedOption` rather than dropping, downgrading, or
/// approximating the corresponding request (architecture.md §29). Help-text
/// parsing is explicitly not the contract (architecture.md §10.4) — this
/// struct is populated from a tested static map for the pinned build
/// (`engine::capability`), cross-checked at startup against the running
/// binary's identity (Phase 1b concern; not implemented here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineCapabilities {
    pub identity: EngineIdentity,
    /// `-D`/`-d` family works at all.
    pub duplicate_detection: bool,
    /// `-d` specifically (diverts duplicates *and* writes an audit file).
    pub duplicate_audit_file: bool,
    /// `-Z` disk-backed duplicate table.
    pub external_duplicate_table: bool,
    /// `-c<master.pgn>` check-file support.
    pub check_file: bool,
    /// `-e<eco.pgn>` ECO/opening classification.
    pub eco_classification: bool,
    /// `FENPattern`/`FENPatternI` criteria-file lines.
    pub fen_patterns: bool,
    /// `-v<variations>` textual opening-line filters.
    pub textual_variations: bool,
    /// `--fixresulttags`.
    pub fix_result_tags: bool,
    /// `--nobadresults`.
    pub reject_bad_results: bool,
    /// Whether a *separate* broken-games output file is possible in one
    /// pass. Always `false` for this engine family (architecture.md §5.1
    /// item 14, §11.6; design-02 D-007 V-5) — kept as an explicit field
    /// rather than removed so the capability map stays self-documenting
    /// about a limitation callers might otherwise assume exists.
    pub separate_broken_output: bool,
    /// Output notations this build can produce. V1 ships `[San]` only
    /// (design-02 D-13); requesting anything not in this list is
    /// `CompileError::UnsupportedOption`.
    pub supported_output_formats: Vec<OutputNotation>,
    /// Whether the sidecar can address non-ACP-representable (e.g.
    /// non-Latin Unicode) paths, per the startup Unicode-path probe
    /// (design-02 Decision D-3). Not consulted by the compiler itself
    /// (`compile` never touches the filesystem); recorded here because it
    /// is a capability of the pinned build like any other, and Phase 1b's
    /// `validate_job` gates on it.
    pub unicode_paths: bool,
}
