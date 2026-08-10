// SPDX-License-Identifier: GPL-3.0-or-later
//! [`JobSpec`] and [`InputFile`] (architecture.md §9.2; design-02 §4.1).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{FilterPlan, OperationPlan, OutputPlan, RuntimeOptions};

/// The schema version every [`JobSpec`] must currently declare.
///
/// Design-02 §4.1 types this as the TypeScript literal `1`, not a general
/// integer. The compiler enforces the literal value (see
/// `engine::command_compiler::compile`); this constant is the single source
/// of truth for it.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// A complete, user-authored job description (architecture.md §9.2).
///
/// This is the same shape that crosses the Tauri IPC boundary as
/// `JobSpecDto` (design-02 §4.1) — there is deliberately no separate
/// "wire" vs "domain" type in this codebase, so `#[serde(deny_unknown_fields)]`
/// on every nested struct is load-bearing: it is the only thing standing
/// between a typo'd frontend field and that field being silently ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobSpec {
    pub schema_version: u32,
    pub id: Uuid,
    pub name: String,
    pub inputs: Vec<InputFile>,
    pub output: OutputPlan,
    pub operations: OperationPlan,
    pub filters: FilterPlan,
    pub runtime: RuntimeOptions,
}

/// One source PGN file, in the order the user added/reordered it.
///
/// Design-02 delta (§4.1, binding per the coordinator's ruling): the
/// architecture document's illustrative `InputFile` (§9.2) also carries
/// `size_bytes`, `modified_at`, and `sha256`. Design-02's actual
/// `JobSpecDto.inputs[]` shape does not — those richer, I/O-derived fields
/// belong to the separate `InputInspectionDto` returned by the
/// `inspect_inputs` command (design-02 §4.1), which is populated by
/// filesystem probing the pure compiler must never perform. `JobSpec.inputs`
/// carries only what the compiler actually needs: a path and its retention
/// priority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputFile {
    pub path: PathBuf,
    pub display_name: String,
    /// Zero-based retention priority. Input order **is** duplicate-retention
    /// priority (architecture.md §10.7; design-02 T-5, §1.4): the compiler
    /// appends input paths to argv in ascending `priority` order via a
    /// stable sort. Design-02 §1.4 additionally states `validate_job`
    /// rejects gaps/duplicates in the priority sequence — that hygiene
    /// check belongs to the Phase 1b validation pipeline (design-02 §3.2),
    /// not to this pure compiler, so `compile` accepts any `u32` values here
    /// and merely orders by them.
    pub priority: u32,
}
