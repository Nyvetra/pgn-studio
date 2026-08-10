// SPDX-License-Identifier: GPL-3.0-or-later
//! Core domain types (architecture.md §7.1, §9; design-02 §1.1, §4.1).
//!
//! Dependencies point inward: nothing in this module (or its submodules) may
//! depend on `tauri`, React/TypeScript, or any particular `pgn-extract`
//! version. Where the architecture document's illustrative Rust (§9.2) and
//! the design-02 specification's `JobSpecDto` (§4.1) disagree, this module
//! follows design-02 per the coordinator's explicit ruling ("design spec
//! wins"); deltas are called out on the affected type.
//!
//! Every inbound type (everything reachable from [`JobSpec`], which is
//! constructed by the frontend and crosses the IPC boundary) derives
//! `Serialize`/`Deserialize` with `rename_all = "camelCase"` and
//! `#[serde(deny_unknown_fields)]`, so a stray/misspelled field from a
//! future frontend build fails loudly instead of being silently ignored.
//! Outbound-only types (e.g. [`JobResult`]) derive `Serialize` for the same
//! wire format but do not need `deny_unknown_fields` since Rust never
//! deserializes them.

mod capability;
mod filters;
mod job_spec;
mod operations;
mod output;
mod result;
mod runtime;

pub use capability::{EngineCapabilities, EngineIdentity};
pub use filters::{FenPatternFilter, FilterPlan, MoveBounds, SetupPolicy, TagName, TagOp, TagRule};
pub use job_spec::{InputFile, JobSpec, CURRENT_SCHEMA_VERSION};
pub use operations::{
    BrokenOutput, CleanupOptions, DuplicatePolicy, EcoOptions, JobMode, OperationPlan,
    OutputNotation,
};
pub use output::{ArtifactKind, ConflictPolicy, DuplicateOutput, OutputArtifact, OutputPlan};
pub use result::{ErrorCode, JobResult, JobStatus, JobWarning, ProcessingMetrics, PublicError};
pub use runtime::RuntimeOptions;
