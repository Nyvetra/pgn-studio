// SPDX-License-Identifier: GPL-3.0-or-later
//! Processing operations: [`OperationPlan`], [`JobMode`], [`DuplicatePolicy`],
//! [`CleanupOptions`], [`BrokenOutput`], [`EcoOptions`], [`OutputNotation`]
//! (architecture.md §9.2, §10.6, §10.7; design-02 §1.3, §4.1, D-6, D-13).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;

/// What processing to run (architecture.md §9.2 `OperationPlan`; design-02
/// §4.1 `operations`).
///
/// Design-02 delta (binding): the architecture document's illustrative
/// `OperationPlan` (§9.2) has separate `merge: bool` / `validate: bool`
/// flags. Design-02's actual DTO replaces both with a single closed `mode`
/// (`JobMode`), which cannot express the contradictory `merge: false,
/// validate: false` (or `true, true`) states the two-boolean form allowed.
/// This module follows design-02.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationPlan {
    pub mode: JobMode,
    pub duplicates: DuplicatePolicy,
    pub cleanup: CleanupOptions,
    pub broken: BrokenOutput,
    pub eco: EcoOptions,
    pub output_notation: OutputNotation,
    /// Check file for the "New Games Against Master" preset: `-c<master.pgn>`
    /// (architecture.md §12.2; design-02 flag table row 4, canonical order
    /// O-5, Decision D-11, golden test G-9).
    ///
    /// Judgment call: design-02 §4.1's `JobSpecDto` does not include a field
    /// for this, even though its own flag-mapping table (row 4), canonical
    /// argument order (§1.4 O-5), and required golden test G-9 all depend on
    /// one existing. This is treated as a gap in that DTO listing rather
    /// than a deliberate omission, since dropping it would make G-9
    /// impossible to implement. The compiler requires `.pgn` (case
    /// insensitive) and an absolute path (row 4b, T-4), and requires
    /// `duplicates != DuplicatePolicy::None` (row 4a: "`-c` alone does
    /// nothing"), rejecting other combinations as `InvalidSpec` rather than
    /// silently emitting an inert `-c` or silently upgrading the duplicate
    /// policy on the caller's behalf.
    pub check_file: Option<PathBuf>,
}

/// `"process" | "validateOnly"` (design-02 §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobMode {
    Process,
    ValidateOnly,
}

/// How to treat games whose move sequence/hash repeats an earlier game in
/// input order (architecture.md §10.7; design-02 §0 finding 1, D-007 V-1/V-2,
/// D-1).
///
/// `-d` and `-D` are mutually exclusive at the engine level (exit 1 if both
/// are given); both `ReportAndKeepFirst` and `SuppressKeepFirst` *divert*
/// duplicates out of the main output, keeping only the first-encountered
/// copy — they never merely "additionally write" an audit file next to an
/// unfiltered main output. The two variants produce byte-identical main
/// outputs; the only difference is whether an audit file exists at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DuplicatePolicy {
    /// No duplicate handling: emit neither `-d` nor `-D`.
    None,
    /// `-d<path>` alone: divert later duplicate copies to an audit file,
    /// main output keeps only first copies.
    ReportAndKeepFirst,
    /// `-D` alone: silently divert (discard) later duplicate copies, no
    /// audit file, no path argument.
    SuppressKeepFirst,
}

/// Text-transform cleanup options (architecture.md §9.2; design-02 §4.1
/// `cleanup`).
///
/// Design-02 delta: the architecture document's illustrative
/// `CleanupOptions` (§9.2) also has `remove_all_tags: bool` (`--notags`/`-7`).
/// Design-02 explicitly drops this from V1 (row 12, Decision D-13: "V1 emits
/// no ... `-7`/`--notags`"); it is capability-map-only, never wired to a
/// `JobSpec` field. This module follows design-02 and omits the field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CleanupOptions {
    pub remove_comments: bool,
    pub remove_variations: bool,
    pub remove_nags: bool,
    pub remove_move_numbers: bool,
    pub remove_results: bool,
    /// Named tags to drop via repeated `--detag <Tag>` (design-02 row 11).
    /// Each entry must match `^[A-Za-z][A-Za-z0-9]*$`; the compiler rejects
    /// anything else as `InvalidSpec` rather than emitting a token the
    /// engine would misparse.
    pub remove_tags: Vec<String>,
    pub reject_bad_results: bool,
    pub fix_result_tags: bool,
}

/// What to do with games the engine cannot parse cleanly (architecture.md
/// §5.1 item 14, §11.6; design-02 §0 finding 2, D-007 V-5, D-6).
///
/// There is deliberately no "separate file" variant: empirically, a single
/// `pgn-extract` invocation cannot route broken games to their own output —
/// without `--keepbroken` they are dropped everywhere (including the
/// non-matching file); with it they land in the *main* output. Making the
/// impossible option unrepresentable (rather than accepting it and silently
/// downgrading it) is the enforcement mechanism for architecture.md §29 here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BrokenOutput {
    /// Default: broken games are dropped (no `--keepbroken`).
    Discard,
    /// `--keepbroken`: broken games land in the main output.
    KeepInMainOutput,
}

/// ECO/opening classification (`-e<eco.pgn>`) toggle (design-02 §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EcoOptions {
    pub enabled: bool,
}

/// Output move notation (design-02 row 29, D-13).
///
/// V1 supports only [`OutputNotation::San`] (the engine's default; the
/// compiler emits no `-W` token for it at all). [`OutputNotation::Uci`]
/// exists solely so the capability-gated totality rule is exercisable
/// end-to-end (golden test G-12: requesting it must produce
/// `CompileError::UnsupportedOption`, never a silently-approximated
/// command) — it is never advertised by the capability map and the
/// compiler never emits a flag for it. Other notations the engine may
/// support (e.g. long algebraic) are deliberately not enumerated here: their
/// exact `-W<fmt>` spelling was not part of the source citations available
/// for this task, and inventing one would violate the project's
/// never-invent rule (DECISIONS-LEDGER.md header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum OutputNotation {
    San,
    Uci,
}
