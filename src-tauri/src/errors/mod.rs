// SPDX-License-Identifier: GPL-3.0-or-later
//! The public error taxonomy and shape (architecture.md §18; design-02 §5).
//!
//! This module owns **construction** of [`PublicError`]/[`JobWarning`]
//! (the types themselves live in [`crate::domain::result`] — see that
//! module's doc comments for why, and for the crate-visibility mechanism
//! that stands in for design-02 §5.2's "private outside the module"
//! requirement). Every function here is a **per-code constructor**: one
//! function per [`ErrorCode`] variant (design-02 §5.1's table), each
//! building a safe, template-based `message` and never forwarding a raw
//! `Display`/`Debug` of an internal error, a backtrace, or unclassified OS
//! error text (design-02 §5.2 rule 1).
//!
//! **Binding redaction rule.** Nothing in this module ever does
//! `format!("{}", io_err)`, `format!("{:?}", io_err)`, or
//! `io_err.to_string()` and puts the result in a `message`/`title`/
//! `remediation` string. Where an underlying error is available, it is
//! classified into one of a fixed, stable set of phrases (see
//! [`classify_io_error`]) and *separately* logged in full via
//! [`log_technical_detail`], joined to the public error only by
//! `technical_id` (design-02 §5.2 rule 2). Reviewers checking this
//! guarantee should grep this file for `{:?}` / `{}` interpolations of any
//! parameter whose type is `std::io::Error`, `anyhow::Error`, or `dyn
//! std::error::Error` — there should be none inside a `message`/`title`/
//! `remediation` argument.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::domain::{ErrorCode, JobWarning, PublicError};

/// Stable, user-safe classification of an [`std::io::Error`]'s kind
/// (design-02 §5.2 rule 1: "a stable classification ('permission denied',
/// 'not found', 'already exists', 'disk full', 'other')"). Never returns
/// the error's own `Display` text, which on Windows is raw
/// `FormatMessageW` output that can vary by locale and occasionally
/// embeds paths or other detail not vetted for the redaction rule.
pub fn classify_io_error(err: &std::io::Error) -> &'static str {
    use std::io::ErrorKind;
    match err.kind() {
        ErrorKind::PermissionDenied => "permission denied",
        ErrorKind::NotFound => "not found",
        ErrorKind::AlreadyExists => "already exists",
        ErrorKind::StorageFull => "disk full",
        _ => "other",
    }
}

/// Writes the full internal chain (this error plus every `.source()` behind
/// it) to the local `tracing` log at `ERROR`, keyed by `technical_id`
/// (design-02 §5.2 rule 2: "`technical_id` is the join key between what the
/// user sees and what the log contains").
///
/// No `tracing` subscriber is installed by Phase 1b (that is
/// application-wiring: `main.rs`/Phase 2's job) — until one is, this event
/// is simply dropped by the default no-op subscriber, which is safe and
/// correct (nothing is lost that a later subscriber couldn't have captured
/// anyway; nothing panics or blocks in the meantime). The call sites and
/// structured fields are correct today so Phase 2 gets full logging for
/// free by installing a subscriber, with no changes needed here.
///
/// When a job workspace is active, callers should *additionally* append a
/// line to `<ws>/logs/engine.log` themselves (the jobs module owns that
/// file's writer; this function has no workspace context) — design-02 §5.2
/// rule 2's "and, when a job is active, to `<ws>/logs/engine.log` context".
pub fn log_technical_detail(
    technical_id: Uuid,
    code: ErrorCode,
    context: &str,
    source: &(dyn std::error::Error + 'static),
) {
    let mut chain = String::new();
    chain.push_str(&source.to_string());
    let mut cursor = source.source();
    while let Some(next) = cursor {
        chain.push_str(" caused by: ");
        chain.push_str(&next.to_string());
        cursor = next.source();
    }
    tracing::error!(
        technical_id = %technical_id,
        code = ?code,
        context,
        chain = %chain,
        "internal error detail (redacted from user-facing message)"
    );
}

fn new_technical_id() -> Uuid {
    Uuid::new_v4()
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

// ---------------------------------------------------------------------
// INPUT_NOT_FOUND
// ---------------------------------------------------------------------

/// Raised by: validation step 2 (missing/not-regular input); startup sweep
/// of history rerun specs (design-02 §5.1).
pub fn input_not_found(path: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::InputNotFound,
        "File not found",
        format!("\"{}\" could not be found.", display_path(path)),
        Some("Check that the file still exists, then re-add it.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// INPUT_NOT_READABLE
// ---------------------------------------------------------------------

/// Raised by: validation open failure (design-02 §5.1). `source` is logged
/// in full via [`log_technical_detail`]; only its stable classification
/// (see [`classify_io_error`]) reaches `message`.
pub fn input_not_readable_io(path: &Path, source: &std::io::Error) -> PublicError {
    let technical_id = new_technical_id();
    log_technical_detail(
        technical_id,
        ErrorCode::InputNotReadable,
        "opening input file for reading",
        source,
    );
    PublicError::from_redacted_parts(
        ErrorCode::InputNotReadable,
        "File not readable",
        format!(
            "\"{}\" could not be opened for reading ({}).",
            display_path(path),
            classify_io_error(source)
        ),
        Some("Check permissions / rename the file to a simpler name.".to_string()),
        None,
        technical_id,
    )
}

/// Raised by: ACP-unrepresentable path when `caps.unicode_paths == false`
/// (design-02 §5.1, D-3) — no underlying `io::Error` exists in this case
/// (the path is rejected before ever asking the OS to open it), so there is
/// nothing to classify or log beyond the public message itself.
pub fn input_not_readable_unicode_unsupported(path: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::InputNotReadable,
        "File path not supported",
        format!(
            "\"{}\" contains characters this build of the bundled engine cannot address.",
            display_path(path)
        ),
        Some("Check permissions / rename the file to a simpler name.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// INPUT_OUTPUT_COLLISION
// ---------------------------------------------------------------------

/// Raised by: §3.1 aliasing check, at validation time and again immediately
/// before publication (design-02 §5.1).
pub fn input_output_collision(input_path: &Path, output_path: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::InputOutputCollision,
        "Output would overwrite a source file",
        format!(
            "The output \"{}\" is the same file as the source \"{}\".",
            display_path(output_path),
            display_path(input_path)
        ),
        Some("Choose a different output folder or base name.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// OUTPUT_NOT_WRITABLE
// ---------------------------------------------------------------------

/// Raised by: destination probe failure (design-02 §5.1).
pub fn output_not_writable_io(dir: &Path, source: &std::io::Error) -> PublicError {
    let technical_id = new_technical_id();
    log_technical_detail(
        technical_id,
        ErrorCode::OutputNotWritable,
        "probing destination directory for writability",
        source,
    );
    PublicError::from_redacted_parts(
        ErrorCode::OutputNotWritable,
        "Folder not writable",
        format!(
            "\"{}\" is not writable ({}).",
            display_path(dir),
            classify_io_error(source)
        ),
        Some("Choose a writable folder.".to_string()),
        None,
        technical_id,
    )
}

/// Raised by: validation step 5, when the destination path exists but is
/// not a directory (e.g. a file). No underlying `io::Error` describes this
/// case (`std::fs::metadata` succeeds; it is the metadata's own `is_dir()`
/// that is false), so, like the unicode variant below, there is nothing to
/// classify or log beyond the message itself.
pub fn output_not_writable_not_a_directory(dir: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::OutputNotWritable,
        "Not a folder",
        format!("\"{}\" is not a folder.", display_path(dir)),
        Some("Choose a writable folder.".to_string()),
        None,
        new_technical_id(),
    )
}

/// Raised by: ACP case for outputs (design-02 §5.1, D-3) — see
/// [`input_not_readable_unicode_unsupported`] for why there is no
/// underlying `io::Error` to classify here either.
pub fn output_not_writable_unicode_unsupported(dir: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::OutputNotWritable,
        "Folder path not supported",
        format!(
            "\"{}\" contains characters this build of the bundled engine cannot address.",
            display_path(dir)
        ),
        Some("Choose a writable folder.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// OUTPUT_EXISTS
// ---------------------------------------------------------------------

/// Raised by: `Fail` policy pre-check; no-replace rename's
/// `ERROR_ALREADY_EXISTS`; `AddNumericSuffix` search exhausted at 999
/// (design-02 §5.1, §3.5).
pub fn output_exists(path: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::OutputExists,
        "Output already exists",
        format!("\"{}\" already exists.", display_path(path)),
        Some("Pick another name, or allow numbered copies.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// INSUFFICIENT_DISK_SPACE
// ---------------------------------------------------------------------

/// Raised by: validation hard floor; engine failure with free-space
/// postmortem below 16 MiB (design-02 §5.1, §3.2 step 8).
pub fn insufficient_disk_space(required_bytes: u64, available_bytes: u64) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::InsufficientDiskSpace,
        "Not enough disk space",
        format!(
            "This job needs about {required_bytes} bytes free but only {available_bytes} bytes \
             are available."
        ),
        Some("Free up disk space and run again.".to_string()),
        None,
        new_technical_id(),
    )
}

/// Warning-grade counterpart of [`insufficient_disk_space`] for design-02
/// §3.2 step 8's *soft* floor ("< 1.1 × Σ input_bytes + 64 MiB ⇒ warning
/// `LOW_DISK_SPACE`"). `LOW_DISK_SPACE` is not itself a member of the
/// closed §18.1 `ErrorCode` taxonomy (design-02 names it, but architecture
/// §18.1's list - which `ErrorCode` implements exactly - does not); rather
/// than inventing a new taxonomy member for a doc/spec inconsistency, this
/// reuses [`ErrorCode::InsufficientDiskSpace`] at warning grade, which is
/// both accurate (it is the same underlying concern, just below the hard
/// floor) and requires no change to the closed enum. See this crate's
/// top-level report for the full writeup of this design-02 inconsistency.
pub fn low_disk_space_warning(required_bytes: u64, available_bytes: u64) -> JobWarning {
    JobWarning::from_redacted_parts(
        ErrorCode::InsufficientDiskSpace,
        format!(
            "Disk space is low: this job may need about {required_bytes} bytes but only \
             {available_bytes} bytes are available. Free up disk space and run again."
        ),
    )
}

// ---------------------------------------------------------------------
// INVALID_JOB_SPEC
// ---------------------------------------------------------------------

/// Raised by: DTO shape, bounds, base-name, criteria representability,
/// unknown job id in `cancel_job`/`get_job`, reused job id (design-02 §5.1).
/// `reason` is already field-specific human text (e.g. from
/// `engine::command_compiler::CompileError::InvalidSpec`), so it is folded
/// directly into `message` rather than needing a separate `remediation`
/// template — design-02's own table entry for this code says remediation is
/// "field-specific", i.e. exactly what `reason` already is.
pub fn invalid_job_spec(field: &str, reason: &str) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::InvalidJobSpec,
        "Invalid job configuration",
        format!("\"{field}\": {reason}"),
        None,
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// UNSUPPORTED_ENGINE_OPTION
// ---------------------------------------------------------------------

/// Raised by: compiler totality (design-02 §1.7); validation step 9;
/// non-empty `advancedArgs` (design-02 §5.1).
pub fn unsupported_engine_option(option: &str, reason: &str) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::UnsupportedEngineOption,
        "Option not supported",
        format!("\"{option}\" is not supported by this build: {reason}"),
        Some("This build's engine does not support that option.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// ENGINE_MISSING
// ---------------------------------------------------------------------

/// Raised by: sidecar resolver, file absent (design-02 §5.1).
pub fn engine_missing(expected_path: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::EngineMissing,
        "Engine not found",
        format!(
            "The bundled processing engine was not found at \"{}\".",
            display_path(expected_path)
        ),
        Some("Reinstall PGN Studio.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// ENGINE_TAMPERED
// ---------------------------------------------------------------------

/// Raised by: resolver, SHA-256 mismatch vs. pinned identity (design-02
/// §5.1). Both hashes are already-computed hex digests, not raw error text,
/// so including them is safe and useful for support diagnosis.
pub fn engine_tampered(expected_sha256: &str, actual_sha256: &str) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::EngineTampered,
        "Engine verification failed",
        format!(
            "The bundled processing engine did not match its expected checksum (expected \
             {expected_sha256}, found {actual_sha256})."
        ),
        Some("Reinstall PGN Studio; the bundled engine failed verification.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// ENGINE_START_FAILED
// ---------------------------------------------------------------------

/// Raised by: `CreateProcessW`/exec error; `--version` probe failure at
/// startup (design-02 §5.1).
pub fn engine_start_failed_io(source: &std::io::Error) -> PublicError {
    let technical_id = new_technical_id();
    log_technical_detail(
        technical_id,
        ErrorCode::EngineStartFailed,
        "spawning the bundled engine process",
        source,
    );
    PublicError::from_redacted_parts(
        ErrorCode::EngineStartFailed,
        "Engine failed to start",
        format!(
            "The bundled processing engine could not be started ({}).",
            classify_io_error(source)
        ),
        Some("Reinstall; if it persists, check antivirus quarantine.".to_string()),
        None,
        technical_id,
    )
}

/// Raised by: `--version` probe returning a nonzero exit or an unexpected
/// identity string (no `io::Error` exists in this path — the process
/// started fine, it just did not answer as expected).
pub fn engine_start_failed_bad_probe(detail: &str) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::EngineStartFailed,
        "Engine failed to start",
        format!("The bundled processing engine did not respond as expected: {detail}."),
        Some("Reinstall; if it persists, check antivirus quarantine.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// ENGINE_EXIT_NONZERO
// ---------------------------------------------------------------------

/// Raised by: any nonzero/abnormal engine exit, including `abort()`
/// (design-02 §5.1). `stderr_tail` is the engine's *own* diagnostic output
/// (not a Rust internal error), already captured by the job's log pipeline;
/// design-02 explicitly wants it "preserved in the job log and referenced
/// by `log_path`". The last line is echoed into `message` for immediate
/// actionability (e.g. "Unable to open the ECO file eco.pgn."), which is
/// the engine talking to the user directly, not a redaction violation.
pub fn engine_exit_nonzero(
    exit_code: Option<i32>,
    log_path: &Path,
    stderr_tail: &[String],
) -> PublicError {
    let code_text = match exit_code {
        Some(c) => format!("exit code {c}"),
        None => "no exit code (terminated by signal)".to_string(),
    };
    let last_line = stderr_tail
        .iter()
        .rev()
        .find(|line| !line.trim().is_empty())
        .cloned();
    let message = match last_line {
        Some(line) => format!("The processing engine stopped early ({code_text}): {line}"),
        None => format!("The processing engine stopped early ({code_text})."),
    };
    PublicError::from_redacted_parts(
        ErrorCode::EngineExitNonzero,
        "Processing stopped early",
        message,
        Some("Open the log for the engine's own message.".to_string()),
        Some(log_path.to_path_buf()),
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// ENGINE_OUTPUT_MISSING / ENGINE_OUTPUT_INVALID
// ---------------------------------------------------------------------

/// Raised by: publication step 4 (design-02 §5.1, §3.4).
pub fn engine_output_missing(path: &Path, log_path: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::EngineOutputMissing,
        "Expected output missing",
        format!(
            "The engine exited successfully but \"{}\" was not created.",
            display_path(path)
        ),
        Some("See log; the engine did not produce the expected file.".to_string()),
        Some(log_path.to_path_buf()),
        new_technical_id(),
    )
}

/// Raised by: publication step 5 (design-02 §5.1, §3.4).
pub fn engine_output_invalid(path: &Path, reason: &str, log_path: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::EngineOutputInvalid,
        "Output looks invalid",
        format!("\"{}\" does not look valid: {reason}.", display_path(path)),
        Some("See log; the engine did not produce the expected file.".to_string()),
        Some(log_path.to_path_buf()),
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// JOB_ALREADY_RUNNING
// ---------------------------------------------------------------------

/// Raised by: §2.6 slot guard (design-02 §5.1).
pub fn job_already_running(running_job_id: Uuid) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::JobAlreadyRunning,
        "A job is already running",
        format!("Job {running_job_id} is still running."),
        Some("Wait for or cancel the current job.".to_string()),
        None,
        new_technical_id(),
    )
}

/// Raised by: `cancel_job(job_id)` when `job_id` does not match the
/// currently active job (design-02 §2.5 step 1: "validates the id against
/// the active slot (mismatch ⇒ typed error `JOB_NOT_ACTIVE` mapped onto
/// `INVALID_JOB_SPEC` messaging)"). `JOB_NOT_ACTIVE` is not itself a member
/// of the closed §18.1 taxonomy - design-02's own text says to map it onto
/// `INVALID_JOB_SPEC`, so (unlike `DUPLICATE_INPUT`/`EMPTY_OUTPUT`, which
/// design-02 names without giving a mapping) this one has an explicit,
/// unambiguous resolution already.
pub fn job_not_active(requested: Uuid, currently_active: Option<Uuid>) -> PublicError {
    let message = match currently_active {
        Some(active) => {
            format!("Job {requested} is not the active job (currently running: job {active}).")
        }
        None => format!("Job {requested} is not currently running."),
    };
    PublicError::from_redacted_parts(
        ErrorCode::InvalidJobSpec,
        "Job not active",
        message,
        None,
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// PATH_NOT_ALLOWED (Phase 2a addition, mapped onto INVALID_JOB_SPEC)
// ---------------------------------------------------------------------

/// Raised by: `reveal_path`/`open_path`'s allowlist check (design-02 §4.1:
/// "only paths that appear in job history/artifacts"). `PATH_NOT_ALLOWED`
/// is not itself a member of the closed §18.1 taxonomy; like
/// [`job_not_active`] does for an unknown job id, this reuses
/// `ErrorCode::InvalidJobSpec` for "the request parameter you gave me is
/// rejected" rather than inventing a 20th taxonomy member.
pub fn path_not_allowed(path: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::InvalidJobSpec,
        "Path not allowed",
        format!(
            "\"{}\" is not a known job input or output path.",
            display_path(path)
        ),
        Some(
            "Only files and folders from a job's own inputs or outputs can be revealed or \
             opened this way."
                .to_string(),
        ),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// INPUT_NOT_READABLE (Phase 2c addition: directory variant, "Add Folder")
// ---------------------------------------------------------------------

/// Raised by: `scan_input_directory`'s root-directory read failure
/// (architecture.md §13.2 "Add Folder"). Reuses `InputNotReadable` - the
/// same underlying hazard as a source file that cannot be opened, just for
/// the folder the user picked to scan rather than a single PGN, matching
/// the precedent [`path_not_allowed`]/[`job_not_active`] already set for
/// reusing an existing code with new, situation-specific wording rather
/// than adding a 20th taxonomy member.
pub fn directory_not_readable_io(path: &Path, source: &std::io::Error) -> PublicError {
    let technical_id = new_technical_id();
    log_technical_detail(
        technical_id,
        ErrorCode::InputNotReadable,
        "reading the selected folder for Add Folder",
        source,
    );
    PublicError::from_redacted_parts(
        ErrorCode::InputNotReadable,
        "Folder not readable",
        format!(
            "\"{}\" could not be read ({}).",
            display_path(path),
            classify_io_error(source)
        ),
        Some("Check the folder still exists and that you have permission to read it.".to_string()),
        None,
        technical_id,
    )
}

// ---------------------------------------------------------------------
// INPUT_OUTPUT_COLLISION (Phase 2c addition: export-destination variant,
// "Save Job")
// ---------------------------------------------------------------------

/// Raised by: `export_job_manifest`'s destination check (architecture.md
/// §11.1's "no backend command may open a source PGN with write access",
/// applied to a job-manifest export target). Reuses `InputOutputCollision`
/// for the same reason [`path_not_allowed`] reuses an existing code: this is
/// exactly the same underlying hazard (writing over a file the job itself
/// depends on or produced), just for the manifest-export flow rather than
/// the PGN-processing flow.
pub fn export_destination_collision(destination: &Path, colliding_with: &Path) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::InputOutputCollision,
        "Save location matches a job file",
        format!(
            "\"{}\" is the same file as \"{}\", which this job already uses.",
            display_path(destination),
            display_path(colliding_with)
        ),
        Some("Choose a different file name or folder.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// INVALID_JOB_SPEC (Phase 2c addition: saved-manifest variant, "Save Job"
// re-validation)
// ---------------------------------------------------------------------

/// Raised by: `filesystem::manifest::parse_and_revalidate_exported_manifest`
/// (architecture.md §16.1's threat model - a saved job file is untrusted
/// input, never assumed genuine merely because it parses as JSON). Reuses
/// `InvalidJobSpec` for the same reason [`invalid_job_spec`] itself is
/// reused throughout this module for "the input you gave me is rejected"
/// shapes that do not have their own §18.1 taxonomy member.
pub fn invalid_saved_manifest(reason: &str) -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::InvalidJobSpec,
        "Saved job file is not valid",
        format!("This saved job file could not be used: {reason}."),
        Some("Re-export the job, or check that the file was not modified.".to_string()),
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// JOB_CANCELLED
// ---------------------------------------------------------------------

/// Raised by: terminal result of the cancel flow, carried in
/// `JobResultDto.error` (design-02 §5.1). No remediation text in the
/// table (shown as "—") — cancellation is a normal, user-initiated outcome,
/// not a fault to recover from.
pub fn job_cancelled() -> PublicError {
    PublicError::from_redacted_parts(
        ErrorCode::JobCancelled,
        "Job cancelled",
        "The job was cancelled.".to_string(),
        None,
        None,
        new_technical_id(),
    )
}

// ---------------------------------------------------------------------
// TEMP_CLEANUP_FAILED (warning-grade)
// ---------------------------------------------------------------------

/// Raised by: §2.5 step 6 / §3.4 failure-path deletion (design-02 §5.1,
/// §18.3). Always warning-grade, attached to an otherwise-terminal result;
/// names the exact leftover paths rather than merely claiming cleanup
/// happened when it did not (§18.3's binding honesty rule).
pub fn temp_cleanup_failed(leftover_paths: &[PathBuf]) -> JobWarning {
    let listed = leftover_paths
        .iter()
        .map(|p| display_path(p))
        .collect::<Vec<_>>()
        .join(", ");
    let message = if leftover_paths.is_empty() {
        "Temporary files could not be fully cleaned up.".to_string()
    } else {
        format!(
            "Temporary files could not be fully cleaned up: {listed}. You may delete the \
             listed folder manually."
        )
    };
    JobWarning::from_redacted_parts(ErrorCode::TempCleanupFailed, message)
}

// ---------------------------------------------------------------------
// HISTORY_WRITE_FAILED (warning-grade)
// ---------------------------------------------------------------------

/// Raised by: manifest/history persistence failure — the job itself may
/// still be `Succeeded`; surfaced as a result warning (design-02 §5.1).
pub fn history_write_failed(source: &std::io::Error) -> JobWarning {
    let technical_id = new_technical_id();
    log_technical_detail(
        technical_id,
        ErrorCode::HistoryWriteFailed,
        "writing job history/manifest",
        source,
    );
    JobWarning::from_redacted_parts(
        ErrorCode::HistoryWriteFailed,
        format!(
            "The job manifest could not be saved ({}). Check app-data disk space/permissions.",
            classify_io_error(source)
        ),
    )
}

// ---------------------------------------------------------------------
// UNKNOWN_INTERNAL_ERROR
// ---------------------------------------------------------------------

/// The sole escape hatch for a truly unmapped internal error (design-02
/// §5.2 rule 4: "a `From<anyhow::Error>` escape hatch exists only for
/// `UNKNOWN_INTERNAL_ERROR`... lint-flagged (`#[deprecated]` shim) to keep
/// mappings explicit"). `#[deprecated]` here is not a claim that the
/// function is going away - it is a deliberate speed bump so every call
/// site needs an explicit `#[allow(deprecated)]` acknowledging "yes, I
/// really could not map this to a specific code", which is exactly what
/// keeps this from quietly becoming the default way to build a
/// `PublicError`. Phase 1b's own code never calls this (every error this
/// phase can produce has a specific code); it exists for Phase 2's command
/// boundary and any future truly-unanticipated failure.
#[deprecated(
    note = "escape hatch for truly unmapped errors only; prefer a specific per-code constructor \
            in this module. Call sites must add #[allow(deprecated)] to acknowledge that."
)]
pub fn unknown_internal_error(err: anyhow::Error) -> PublicError {
    let technical_id = new_technical_id();
    tracing::error!(
        technical_id = %technical_id,
        code = ?ErrorCode::UnknownInternalError,
        chain = ?err,
        "unmapped internal error (redacted from user-facing message)"
    );
    PublicError::from_redacted_parts(
        ErrorCode::UnknownInternalError,
        "Unexpected error",
        format!("An unexpected internal error occurred (id {technical_id})."),
        Some("Report this with the technical id.".to_string()),
        None,
        technical_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some_io_error(kind: std::io::ErrorKind) -> std::io::Error {
        std::io::Error::new(
            kind,
            "some very specific raw OS message that must never leak",
        )
    }

    #[test]
    fn classify_io_error_never_leaks_raw_text() {
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::AlreadyExists,
            std::io::ErrorKind::StorageFull,
            std::io::ErrorKind::Other,
        ] {
            let classified = classify_io_error(&some_io_error(kind));
            assert!(
                !classified.contains("very specific raw OS message"),
                "classification must never contain the raw io::Error text"
            );
        }
    }

    #[test]
    fn every_constructor_produces_a_distinct_technical_id() {
        let a = input_not_found(Path::new("a.pgn"));
        let b = input_not_found(Path::new("a.pgn"));
        assert_ne!(
            a.technical_id(),
            b.technical_id(),
            "two calls with identical arguments must still mint distinct technical ids"
        );
    }

    #[test]
    fn input_not_readable_io_never_leaks_raw_os_text_in_message() {
        let err = some_io_error(std::io::ErrorKind::PermissionDenied);
        let public = input_not_readable_io(Path::new(r"C:\secret\a.pgn"), &err);
        assert!(!public.message().contains("very specific raw OS message"));
        assert_eq!(public.code(), ErrorCode::InputNotReadable);
        assert!(public.message().contains("permission denied"));
    }

    #[test]
    fn engine_tampered_reports_both_hashes() {
        let public = engine_tampered("aaaa", "bbbb");
        assert_eq!(public.code(), ErrorCode::EngineTampered);
        assert!(public.message().contains("aaaa"));
        assert!(public.message().contains("bbbb"));
    }

    #[test]
    fn engine_exit_nonzero_uses_last_nonblank_stderr_line() {
        let log_path = Path::new(r"C:\ws\logs\engine.log");
        let tail = vec![
            "Games: 1000".to_string(),
            "Unable to open the ECO file eco.pgn.".to_string(),
            "".to_string(),
        ];
        let public = engine_exit_nonzero(Some(1), log_path, &tail);
        assert!(public
            .message()
            .contains("Unable to open the ECO file eco.pgn."));
        assert_eq!(public.log_path(), Some(log_path));
    }

    #[test]
    fn temp_cleanup_failed_names_exact_leftover_paths() {
        let leftovers = vec![PathBuf::from(r"C:\dest\.pgnstudio-tmp-abc-unique.pgn")];
        let warning = temp_cleanup_failed(&leftovers);
        assert_eq!(warning.code(), ErrorCode::TempCleanupFailed);
        assert!(warning
            .message()
            .contains(r"C:\dest\.pgnstudio-tmp-abc-unique.pgn"));
    }

    #[test]
    fn temp_cleanup_failed_never_claims_deletion_with_empty_list_wording() {
        // §18.3: never claim deletion that did not happen. With a non-empty
        // leftover list the message must name paths, not say "cleaned up".
        let leftovers = vec![PathBuf::from(r"C:\dest\x.pgn")];
        let warning = temp_cleanup_failed(&leftovers);
        assert!(warning.message().contains("could not be fully cleaned up"));
    }

    #[test]
    fn job_cancelled_has_no_remediation() {
        let public = job_cancelled();
        assert_eq!(public.remediation(), None);
    }

    #[test]
    fn directory_not_readable_io_never_leaks_raw_os_text_in_message() {
        let err = some_io_error(std::io::ErrorKind::PermissionDenied);
        let public = directory_not_readable_io(Path::new(r"C:\secret\folder"), &err);
        assert!(!public.message().contains("very specific raw OS message"));
        assert_eq!(public.code(), ErrorCode::InputNotReadable);
        assert!(public.message().contains("permission denied"));
    }

    #[test]
    fn export_destination_collision_names_both_paths() {
        let public = export_destination_collision(
            Path::new(r"C:\out\job.json"),
            Path::new(r"C:\out\master-clean.pgn"),
        );
        assert_eq!(public.code(), ErrorCode::InputOutputCollision);
        assert!(public.message().contains(r"C:\out\job.json"));
        assert!(public.message().contains(r"C:\out\master-clean.pgn"));
    }

    #[test]
    fn invalid_saved_manifest_includes_the_given_reason() {
        let public = invalid_saved_manifest("unsupported job file version 999");
        assert_eq!(public.code(), ErrorCode::InvalidJobSpec);
        assert!(public
            .message()
            .contains("unsupported job file version 999"));
    }

    #[allow(deprecated)]
    #[test]
    fn unknown_internal_error_includes_technical_id_in_message() {
        let err = anyhow::anyhow!("raw internal detail that must not leak verbatim");
        let public = unknown_internal_error(err);
        assert_eq!(public.code(), ErrorCode::UnknownInternalError);
        assert!(!public.message().contains("raw internal detail"));
        assert!(public
            .message()
            .contains(&public.technical_id().to_string()));
    }
}
