// SPDX-License-Identifier: GPL-3.0-or-later
//! Output planning: [`OutputPlan`], [`ConflictPolicy`], [`DuplicateOutput`],
//! [`ArtifactKind`], [`OutputArtifact`] (architecture.md §9.2, §11.5, §11.6;
//! design-02 §3.4, §3.5, §4.1).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Where and how to publish job outputs (design-02 §4.1 `output`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputPlan {
    /// User-selected destination directory, as typed/browsed in the UI.
    ///
    /// The compiler never reads this field directly: `CompileLayout::destination_dir`
    /// (engine::command_compiler) carries the *canonicalized* form of this
    /// same directory, resolved by the application layer before compilation
    /// (design-02 §1.1). Keeping the raw, pre-canonicalization value here is
    /// still required for DTO round-tripping (history, the Review screen)
    /// and for Phase 1b's `validate_job` to diagnose it.
    pub directory: PathBuf,
    pub base_name: String,
    /// Whether to emit the merged/deduplicated main output (`-o`) at all.
    /// `false` is a legitimate, non-error request (see
    /// `engine::command_compiler` for the exact rules); it is not a variant
    /// of validate-only mode, which is controlled by `OperationPlan.mode`.
    pub unique_games: bool,
    /// Whether a produced duplicates-audit temp file (see
    /// `OperationPlan::duplicates`) should be *published* as a final
    /// artifact. This is a genuinely separate axis from
    /// `OperationPlan::duplicates`: the latter selects the engine's
    /// duplicate-handling flag (`-d`/`-D`, or neither); this field only
    /// decides whether a `-d` audit file that the engine produced as a side
    /// effect is promoted to `final_outputs` or left as a discarded
    /// temporary. `engine::command_compiler::compile` rejects combinations
    /// where the two disagree in a way that could never produce a real file
    /// (e.g. `Audit` requested under `DuplicatePolicy::None`) as
    /// `CompileError::InvalidSpec`, rather than silently ignoring either
    /// field (architecture.md §29).
    pub duplicate_games: DuplicateOutput,
    pub log_file: bool,
    pub manifest: bool,
    /// When true, publish a duplicates-audit artifact even if it would be
    /// empty (design-02 D-21 / architecture.md §11.6). Only meaningful when
    /// `duplicate_games == Audit`.
    pub always_create_audit: bool,
    pub conflict_policy: ConflictPolicy,
    /// Must be `true` for `conflict_policy == ReplaceAfterConfirmation` to
    /// compile (design-02 §3.5: "requires `confirmed_replace: true` in the
    /// spec, set only after an explicit UI confirmation dialog"). Checked by
    /// the compiler, not merely by convention, since it is a pure
    /// spec-internal consistency rule that needs no filesystem access.
    pub confirmed_replace: bool,
}

/// Whether a duplicate-games audit file, if the engine produces one as a
/// side effect of `OperationPlan::duplicates`, should be published.
///
/// See the field doc on [`OutputPlan::duplicate_games`] for how this
/// interacts with `OperationPlan::duplicates`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateOutput {
    None,
    Audit,
}

/// What to do when a planned output path already exists at publication time
/// (architecture.md §11.5; design-02 §3.5). Phase 1a only *stores* this
/// choice on the spec; acting on it (numeric-suffix search, recycle-bin
/// replacement, no-replace atomic rename) is filesystem I/O and belongs to
/// Phase 1b (design-02 §3.4 step 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    Fail,
    AddNumericSuffix,
    ReplaceAfterConfirmation,
}

/// The kind of a planned or published output file (design-02 §1.1).
///
/// Shared between the engine layer (`TemporaryOutput`/`FinalOutput` in
/// `engine::command_compiler`) and the domain's own [`OutputArtifact`], so it
/// lives here rather than in `engine/` — matching the explicit placement in
/// the Phase 1a task scope and architecture.md §7.1's "dependencies point
/// inward" rule (the engine module may depend on this domain type; the
/// domain must never depend on the engine module).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    UniqueGames,
    DuplicateGames,
    ReportJson,
    ReportText,
    LogText,
}

/// A published output artifact, as reported on [`super::JobResult`].
///
/// Neither architecture.md §9 nor design-02 spells out this type's exact
/// field list (design-02's `job://artifact` event payload references an
/// `OutputArtifactDto` without defining it). This shape is a judgment call:
/// the minimum needed to describe "what got written, and how big is it" for
/// history/manifest display. Populating it is Phase 1b's job (it requires a
/// `stat` call after publication); Phase 1a only defines the shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputArtifact {
    pub kind: ArtifactKind,
    pub path: PathBuf,
    pub size_bytes: u64,
}
