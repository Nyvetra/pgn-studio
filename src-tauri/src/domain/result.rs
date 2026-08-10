// SPDX-License-Identifier: GPL-3.0-or-later
//! Job outcome types: [`JobResult`], [`JobStatus`], [`ProcessingMetrics`],
//! plus the minimal [`ErrorCode`]/[`PublicError`]/[`JobWarning`] shapes
//! needed to type [`JobResult`] honestly (architecture.md §9.3, §18.1-§18.2;
//! design-02 §2.1, §2.4, §5.1, §5.2, D-19).
//!
//! None of these types are produced by the Phase 1a command compiler — they
//! describe the *outcome* of running a job, which is Phase 1b (job
//! orchestration) territory. They are defined here, per the Phase 1a task
//! scope, so the domain model is complete and so later phases have a real
//! type to build against instead of a `TODO`.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::{EngineIdentity, OutputArtifact};

/// Backend-owned runtime states (design-02 §2.1: "`Draft/Validating/Ready`
/// are spec-lifecycle states held in frontend store...; only
/// `Running/Cancelling` and the terminal states exist as backend-owned
/// runtime state"). A [`JobResult`] is only ever produced for a job that
/// reached `Running`, so those frontend-local states have no representation
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

/// Processing metrics (architecture.md §9.3).
///
/// **Binding rule: never substitute `0` for a metric that could not be
/// measured.** Every metric that is not *always* derivable is `Option<u64>`
/// and must stay `None` rather than being guessed or defaulted. `input_files`
/// and `input_bytes` are the two exceptions design-02 §2.4 calls out as
/// always known ("from validation stats (always)") — they alone are plain
/// `u64`, matching architecture.md §9.3's illustrative type exactly.
///
/// `processed_games` is design-02 Decision D-19: architecture.md §9.3's
/// struct has no live-progress field, but §13.6 requires honest live
/// progress reporting, so design-02 adds this optional field for the
/// in-flight `Games: N` tick count (as opposed to `input_games`, which is
/// only known from the *final* summary line after the process exits 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessingMetrics {
    pub input_files: u64,
    pub input_bytes: u64,
    /// Live/last-seen progress count from `Games: N` stderr ticks, or the
    /// final summary's `N` once the process exits (design-02 D-19).
    pub processed_games: Option<u64>,
    /// Final summary line's `M` (games actually parsed from normal inputs;
    /// check-file games are excluded by the engine itself).
    pub input_games: Option<u64>,
    pub output_games: Option<u64>,
    pub duplicate_games: Option<u64>,
    /// **Always `None` in V1.** design-02 §2.4 originally scoped this as
    /// derivable (`total - matched` from the final summary line) whenever no
    /// filters were active and duplicate policy was `None`/`ReportAndKeepFirst`.
    /// Phase 4 empirically disproved that against the real pinned engine:
    /// depending on stream position, a game with a parse-recoverable defect
    /// (e.g. a missing result marker) can be silently dropped from, or have
    /// its moves silently stripped from, the published output while leaving
    /// `total - matched` completely unchanged (see
    /// `engine::command_compiler::MetricsPlan::broken_games` and
    /// `phase4_integration.rs` for the reproducing fixtures). Since this can
    /// under-report a real, nonzero broken-game count as `0` — precisely
    /// what this struct's own binding rule above forbids — the compiler
    /// never plans this metric as derivable, so it is always `None`.
    pub broken_games: Option<u64>,
    pub output_bytes: Option<u64>,
}

/// The closed error-code taxonomy (architecture.md §18.1; design-02 §5.1).
///
/// This is the complete V1 set as enumerated in design-02 §5.1's
/// "raised by" table, plus one Phase 3 addition. Real construction is
/// intentionally NOT implemented here (see [`PublicError`]'s doc comment) —
/// this Phase 1a module only needs the closed set to exist so [`JobResult`],
/// [`PublicError`], and [`JobWarning`] can be typed completely rather than
/// with a placeholder `String`.
///
/// **`AnnotatedDuplicatesSuppressed` (Phase 3):** every later addition to
/// this enum before Phase 3 (`crate::errors`'s `path_not_allowed`,
/// `job_not_active`, `directory_not_readable_io`,
/// `export_destination_collision`, `invalid_saved_manifest`) deliberately
/// *reused* an existing variant rather than growing the enum, each with a
/// doc comment explaining why the reused code was still an honest fit. This
/// one variant breaks that pattern on purpose: architecture.md §24's Phase 3
/// exit criterion and §27's risk table both name "annotated-duplicate
/// warnings" as their own concern (a *content* advisory — "this audit file
/// holds annotations you may want to look at" — not a procedural/technical
/// failure), and none of the other 18 members describe anything like it.
/// Reusing an unrelated one (say, `EngineOutputInvalid`, which normally means
/// "this file does not look like valid PGN") would be more misleading than
/// adding a correctly-named 19th member — the same honesty rule
/// (`crate::errors`'s module doc: never blur what a code actually means)
/// argues for a new variant here rather than against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InputNotFound,
    InputNotReadable,
    InputOutputCollision,
    OutputNotWritable,
    OutputExists,
    InsufficientDiskSpace,
    InvalidJobSpec,
    UnsupportedEngineOption,
    EngineMissing,
    EngineTampered,
    EngineStartFailed,
    EngineExitNonzero,
    EngineOutputMissing,
    EngineOutputInvalid,
    JobAlreadyRunning,
    JobCancelled,
    TempCleanupFailed,
    HistoryWriteFailed,
    UnknownInternalError,
    /// Warning-grade only (architecture.md §24 Phase 3, §27): a
    /// `ReportAndKeepFirst` run published a non-empty duplicates-audit file
    /// in which at least one diverted duplicate carries a comment, NAG, or
    /// variation. See `crate::errors::annotated_duplicates_suppressed` and
    /// `crate::filesystem::duplicate_audit`.
    AnnotatedDuplicatesSuppressed,
}

/// The user-facing error shape (architecture.md §18.2; design-02 §5.2).
///
/// **Phase 1b update:** design-02 §5.2 requires this struct's fields to be
/// "private outside the module" that constructs it, with construction
/// happening only "through per-code constructors" that enforce the
/// redaction rule (no raw `Display`/`Debug` of internal errors, no
/// backtraces, no raw OS error text in `message`). Phase 1a left every field
/// `pub` as a deliberate placeholder (see the git history of this file) with
/// an explicit note not to construct instances outside the real `errors/`
/// module once it existed.
///
/// `errors/` now exists (architecture.md's dependency-inward rule places it
/// alongside `domain` — an application-layer module may depend on it, but
/// `domain` itself must not depend on `tauri`/React/a particular engine
/// version, and does not depend on `errors/` here either). To honor
/// design-02's literal privacy requirement without domain importing an
/// outer-layer module, the type stays defined here with genuinely private
/// fields; the only constructor is [`PublicError::from_redacted_parts`],
/// which is `pub(crate)` (Rust has no "friend module" visibility, so
/// crate-wide is the strongest restriction achievable while keeping the
/// type in `domain`) and is documented as reserved for `crate::errors`'s
/// per-code constructor functions. Every other module — including this
/// crate's own `jobs`/`filesystem` — must go through `crate::errors`, never
/// build a `PublicError` by hand; that convention is what actually carries
/// the redaction guarantee, since `from_redacted_parts` itself performs no
/// redaction (it trusts its caller completely).
///
/// Deliberately **not** `Deserialize`: this type is outbound-only (Rust
/// constructs it, the frontend only ever receives it), and deriving
/// `Deserialize` would reopen exactly the bypass this design closes — any
/// code could reconstruct an arbitrary `PublicError` via
/// `serde_json::from_str` instead of a struct literal.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PublicError {
    code: ErrorCode,
    title: String,
    message: String,
    remediation: Option<String>,
    log_path: Option<PathBuf>,
    technical_id: Uuid,
}

impl PublicError {
    /// Builds a `PublicError` from parts the caller asserts are already
    /// redacted. `pub(crate)` and reserved for `crate::errors`'s per-code
    /// constructors — see the type-level doc comment for why this is the
    /// narrowest visibility Rust allows here, and why it is still safe:
    /// every call site is reviewable (`grep` for this function name), and
    /// `crate::errors` is the only module that should ever call it.
    pub(crate) fn from_redacted_parts(
        code: ErrorCode,
        title: impl Into<String>,
        message: impl Into<String>,
        remediation: Option<String>,
        log_path: Option<PathBuf>,
        technical_id: Uuid,
    ) -> Self {
        Self {
            code,
            title: title.into(),
            message: message.into(),
            remediation,
            log_path,
            technical_id,
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn remediation(&self) -> Option<&str> {
        self.remediation.as_deref()
    }

    pub fn log_path(&self) -> Option<&Path> {
        self.log_path.as_deref()
    }

    pub fn technical_id(&self) -> Uuid {
        self.technical_id
    }
}

/// A non-fatal, warning-grade occurrence attached to an otherwise-terminal
/// [`JobResult`] (e.g. `TEMP_CLEANUP_FAILED`, `HISTORY_WRITE_FAILED` —
/// design-02 §5.1, §18.3). Reuses [`ErrorCode`] since design-02 treats
/// warnings as drawn from the same closed taxonomy, just surfaced at a
/// lower severity alongside a successful/terminal result rather than as the
/// result's own `error`.
///
/// Same privacy shape as [`PublicError`] and for the same reason: a warning
/// `message` is just as user-facing as an error's, so it goes through the
/// same redaction discipline in `crate::errors` rather than being built
/// ad hoc wherever a warning is noticed.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JobWarning {
    code: ErrorCode,
    message: String,
}

impl JobWarning {
    /// Reserved for `crate::errors`'s per-code constructors — see
    /// [`PublicError::from_redacted_parts`]'s doc comment.
    pub(crate) fn from_redacted_parts(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// The terminal outcome of a single job run (architecture.md §9.3).
///
/// Deliberately not `Deserialize` (Phase 1b correction): this type is
/// outbound-only, as this struct's own original doc comment already said —
/// dropping the derive (rather than just omitting `deny_unknown_fields`,
/// Phase 1a's approach) makes that true at the type level, and is required
/// anyway once `PublicError`/`JobWarning` stop being `Deserialize`.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JobResult {
    pub job_id: Uuid,
    pub status: JobStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub elapsed_ms: u64,
    pub engine: EngineIdentity,
    pub artifacts: Vec<OutputArtifact>,
    pub metrics: ProcessingMetrics,
    pub warnings: Vec<JobWarning>,
    pub error: Option<PublicError>,
}
