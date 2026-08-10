// SPDX-License-Identifier: GPL-3.0-or-later
//! `validate_job` (architecture.md §11.2; design-02 §3.2): the ordered,
//! 11-step filesystem-aware validation pipeline. Errors block; warnings
//! never do (design-02: "Warnings never block; errors do").
//!
//! This is a plain synchronous function (blocking filesystem I/O only, no
//! process spawn) - callers on an async runtime should run it via
//! `tokio::task::spawn_blocking` (architecture.md §19.4: filesystem
//! scanning must not run on the async/UI-adjacent thread). Kept
//! synchronous rather than `async fn` so it stays trivially unit-testable
//! and so its logic reads top-to-bottom like the spec's own numbered list.
//!
//! **Design-02 inconsistency found and resolved (documented here, and in
//! the crate-level report):** §3.2 step 4 and §3.4 step 5 name warning
//! "codes" `DUPLICATE_INPUT` and `EMPTY_OUTPUT` that do not exist in
//! architecture.md §18.1's closed `ErrorCode` taxonomy (which this
//! project's `errors/` module implements exactly, per this task's own
//! "closed §18.1 taxonomy" instruction). `LOW_DISK_SPACE` (§3.2 step 8) has
//! the same problem but was resolved cleanly by reusing
//! `ErrorCode::InsufficientDiskSpace` at warning grade (see
//! `errors::low_disk_space_warning`), the same underlying concern at lower
//! severity. `DUPLICATE_INPUT` and `EMPTY_OUTPUT` (the latter is
//! `filesystem::publish`'s concern, not this module's) have no such
//! natural home in the closed taxonomy, so rather than inventing new
//! `ErrorCode` variants (contradicting the closed-taxonomy instruction) or
//! silently dropping real signal the design doc clearly wants surfaced,
//! this module reports them as free-text [`ValidationOutcome::advisories`]:
//! informational, never blocking, and not pretending to be part of the
//! closed error/warning taxonomy.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::domain::{ConflictPolicy, EngineCapabilities, JobSpec, PublicError};
use crate::engine::command_compiler::{compile, CompileError, CompileLayout};
use crate::engine::EngineExecutable;
use crate::errors;

use super::identity;
use super::platform;

/// Everything [`validate_job`] needs beyond the spec and capabilities: a
/// verified engine (resolved once at startup, not per validation call) and
/// the two paths [`CompileLayout`] requires that are not filesystem facts
/// about the job itself. `workspace_root` need not exist yet - `compile`
/// never touches the filesystem (design-02 §1.1) - it only needs to be the
/// same deterministic path the real run will later create (so that, if a
/// caller inspects `compile_job_preview`-style output, the argv it shows
/// matches what will actually run).
#[derive(Debug, Clone)]
pub struct ValidationLayout {
    pub engine: EngineExecutable,
    pub workspace_root: PathBuf,
    pub eco_file: PathBuf,
}

/// The result of running the full validation pipeline (design-02 §4.1
/// `ValidationReportDto`, adapted: Phase 2's DTO layer will project this
/// into the wire type; this is the internal, richer shape).
#[derive(Debug, Clone, Default)]
pub struct ValidationOutcome {
    pub errors: Vec<PublicError>,
    pub warnings: Vec<crate::domain::JobWarning>,
    /// Free-text notes with no closed-taxonomy code - see this module's
    /// doc comment for why (`DUPLICATE_INPUT`).
    pub advisories: Vec<String>,
    pub estimated_input_bytes: u64,
    pub free_disk_bytes: Option<u64>,
}

impl ValidationOutcome {
    pub fn is_ready(&self) -> bool {
        self.errors.is_empty()
    }
}

const MIB: u64 = 1024 * 1024;
const DISK_SPACE_FLOOR_BYTES: u64 = 64 * MIB;
const DISK_SPACE_MARGIN_BYTES: u64 = 64 * MIB;

/// Runs the full ordered validation pipeline (design-02 §3.2, steps 1-11;
/// step numbers below are cited in comments so this can be checked line by
/// line against the spec).
pub fn validate_job(
    spec: &JobSpec,
    caps: &EngineCapabilities,
    layout: &ValidationLayout,
) -> ValidationOutcome {
    let mut errors: Vec<PublicError> = Vec::new();
    let mut warnings: Vec<crate::domain::JobWarning> = Vec::new();
    let mut advisories: Vec<String> = Vec::new();

    // --- Step 1: spec shape -------------------------------------------
    if spec.inputs.is_empty() {
        errors.push(errors::invalid_job_spec(
            "inputs",
            "at least one input file is required",
        ));
    }
    if !priorities_are_contiguous(spec) {
        errors.push(errors::invalid_job_spec(
            "inputs[].priority",
            "priorities must be a contiguous, gap-free, duplicate-free sequence starting at 0",
        ));
    }
    if let Err(reason) = validate_base_name(&spec.output.base_name) {
        errors.push(errors::invalid_job_spec("output.baseName", &reason));
    }

    // --- Step 2 (+3, +4): per-input filesystem checks ------------------
    let mut estimated_input_bytes: u64 = 0;
    let mut input_handles: Vec<(usize, same_file::Handle)> = Vec::new();
    for (idx, input) in spec.inputs.iter().enumerate() {
        if !input.path.is_absolute() {
            errors.push(errors::invalid_job_spec(
                "inputs[].path",
                "path must be absolute",
            ));
            continue;
        }
        let metadata = match std::fs::metadata(&input.path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                errors.push(errors::input_not_found(&input.path));
                continue;
            }
            Err(e) => {
                // design-02 step 2 maps "exists and is a regular file" to
                // INPUT_NOT_FOUND; any other stat failure (e.g. permission
                // denied even to stat the path) is INPUT_NOT_READABLE.
                errors.push(errors::input_not_readable_io(&input.path, &e));
                continue;
            }
        };
        if !metadata.is_file() {
            // "no dir/reparse-only" - design-02 step 2 maps this to
            // INPUT_NOT_FOUND too (the thing the user pointed at is not a
            // usable input file, same remediation: check the path).
            errors.push(errors::input_not_found(&input.path));
            continue;
        }

        let has_pgn_extension = input
            .path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("pgn"))
            .unwrap_or(false);
        if !has_pgn_extension {
            errors.push(errors::invalid_job_spec(
                "inputs[].path",
                "must have a .pgn extension",
            ));
            continue;
        }

        let file = match std::fs::File::open(&input.path) {
            Ok(f) => f,
            Err(e) => {
                errors.push(errors::input_not_readable_io(&input.path, &e));
                continue;
            }
        };

        // Step 3: ACP round-trip (only meaningful when the running engine
        // build cannot address non-ACP-representable paths, D-3).
        if !caps.unicode_paths && !platform::is_acp_representable(input.path.as_os_str()) {
            errors.push(errors::input_not_readable_unicode_unsupported(&input.path));
            continue;
        }

        estimated_input_bytes = estimated_input_bytes.saturating_add(metadata.len());

        match same_file::Handle::from_file(file) {
            Ok(handle) => input_handles.push((idx, handle)),
            Err(_) => { /* identity is best-effort for the duplicate-input advisory only */ }
        }
    }

    // Step 4: duplicate inputs (same identity twice) -> advisory, not an
    // error (design-02: "legal; the engine would treat the second as pure
    // duplicates"). See this module's doc comment for why this is an
    // advisory string rather than a JobWarning.
    for i in 0..input_handles.len() {
        for j in (i + 1)..input_handles.len() {
            if input_handles[i].1 == input_handles[j].1 {
                let (idx_a, idx_b) = (input_handles[i].0, input_handles[j].0);
                advisories.push(format!(
                    "\"{}\" and \"{}\" are the same file on disk; the second will be treated as \
                     a pure duplicate of the first.",
                    spec.inputs[idx_a].path.display(),
                    spec.inputs[idx_b].path.display()
                ));
            }
        }
    }

    // --- Step 11 (Elo bounds; move bounds are checked by `compile` below,
    // date-range low<=high is not independently structurally checkable -
    // see the crate-level report) -----------------------------------
    validate_elo_bounds(spec, &mut errors);

    // --- Step 5: destination directory ---------------------------------
    let mut canonical_destination_dir: Option<PathBuf> = None;
    match std::fs::metadata(&spec.output.directory) {
        Ok(meta) if !meta.is_dir() => {
            errors.push(errors::output_not_writable_not_a_directory(
                &spec.output.directory,
            ));
        }
        Ok(_) => {
            if !caps.unicode_paths
                && !platform::is_acp_representable(spec.output.directory.as_os_str())
            {
                errors.push(errors::output_not_writable_unicode_unsupported(
                    &spec.output.directory,
                ));
            } else if let Err(e) = identity::probe_writable(&spec.output.directory) {
                errors.push(errors::output_not_writable_io(&spec.output.directory, &e));
            } else {
                match std::fs::canonicalize(&spec.output.directory) {
                    Ok(canonical) => canonical_destination_dir = Some(canonical),
                    Err(e) => {
                        errors.push(errors::output_not_writable_io(&spec.output.directory, &e))
                    }
                }
            }
        }
        Err(e) => {
            errors.push(errors::output_not_writable_io(&spec.output.directory, &e));
        }
    }

    // --- Steps 6, 7, 9, 10 (aliasing, conflict pre-check, capability
    // gating, criteria representability): delegated to the pure compiler,
    // which is the single source of truth for all of these already (never
    // re-implemented here, to guarantee this can't drift from what will
    // actually be compiled at run time) -------------------------------
    let mut free_disk_bytes: Option<u64> = None;
    if let Some(destination_dir) = canonical_destination_dir.clone() {
        let compile_layout = CompileLayout {
            engine: layout.engine.clone(),
            workspace_root: layout.workspace_root.clone(),
            eco_file: layout.eco_file.clone(),
            destination_dir: destination_dir.clone(),
        };
        match compile(spec, caps, &compile_layout) {
            Err(CompileError::InvalidSpec { field, reason }) => {
                errors.push(errors::invalid_job_spec(&field, &reason));
            }
            Err(CompileError::UnsupportedOption { option, reason }) => {
                errors.push(errors::unsupported_engine_option(option, &reason));
            }
            Ok(compiled) => {
                // Step 6: aliasing, checked against every planned artifact
                // (temporary AND final - a temp output aliasing a source is
                // just as destructive as a final one).
                let mut aliased_artifact_paths: HashSet<PathBuf> = HashSet::new();
                let artifact_paths = compiled
                    .temporary_outputs
                    .iter()
                    .map(|t| &t.path)
                    .chain(compiled.final_outputs.iter().map(|f| &f.path));
                for artifact_path in artifact_paths {
                    for input in &spec.inputs {
                        if !input.path.is_absolute() || !input.path.exists() {
                            // Already reported (or unreadable) above; do not
                            // pile on a second, less-specific error.
                            continue;
                        }
                        if identity::is_aliased(&input.path, artifact_path).unwrap_or(false) {
                            errors.push(errors::input_output_collision(&input.path, artifact_path));
                            aliased_artifact_paths.insert(artifact_path.clone());
                        }
                    }
                }

                // Step 7: conflict policy pre-check. "identity also
                // compared to inputs first, so a collision reports as
                // collision, not conflict" - achieved by skipping any
                // final path already flagged as aliased above.
                if spec.output.conflict_policy == ConflictPolicy::Fail {
                    for final_output in &compiled.final_outputs {
                        if aliased_artifact_paths.contains(&final_output.path) {
                            continue;
                        }
                        if final_output.path.exists() {
                            errors.push(errors::output_exists(&final_output.path));
                        }
                    }
                }
            }
        }

        // --- Step 8: disk space -----------------------------------------
        match platform::disk_free_bytes(&destination_dir) {
            Ok(free) => {
                free_disk_bytes = Some(free);
                let required = estimated_input_bytes
                    .saturating_add(estimated_input_bytes / 10) // *1.1
                    .saturating_add(DISK_SPACE_MARGIN_BYTES);
                if free < DISK_SPACE_FLOOR_BYTES {
                    errors.push(errors::insufficient_disk_space(required, free));
                } else if free < required {
                    warnings.push(errors::low_disk_space_warning(required, free));
                }
            }
            Err(_) => { /* non-fatal: disk space just stays unreported */ }
        }
    }

    ValidationOutcome {
        errors,
        warnings,
        advisories,
        estimated_input_bytes,
        free_disk_bytes,
    }
}

fn priorities_are_contiguous(spec: &JobSpec) -> bool {
    let mut priorities: Vec<u32> = spec.inputs.iter().map(|i| i.priority).collect();
    priorities.sort_unstable();
    priorities
        .iter()
        .enumerate()
        .all(|(idx, &p)| idx as u32 == p)
}

const RESERVED_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

fn validate_base_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("must not be empty".to_string());
    }
    if name.len() > 120 {
        return Err(format!("must be at most 120 bytes, got {}", name.len()));
    }
    if name.chars().any(|c| RESERVED_CHARS.contains(&c)) {
        return Err(r#"must not contain any of < > : " / \ | ? *"#.to_string());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("must not contain control characters".to_string());
    }
    if name.starts_with(' ') || name.ends_with(' ') || name.ends_with('.') {
        return Err("must not have leading/trailing spaces or a trailing dot".to_string());
    }
    if identity::is_reserved_windows_device_name(name) {
        return Err(format!("\"{name}\" is a reserved device name"));
    }
    Ok(())
}

fn validate_elo_bounds(spec: &JobSpec, errors: &mut Vec<PublicError>) {
    use crate::domain::TagName;
    for rule in &spec.filters.tag_rules {
        let is_elo_tag = matches!(
            rule.tag,
            TagName::WhiteElo | TagName::BlackElo | TagName::Elo | TagName::EloDiff
        );
        if !is_elo_tag {
            continue;
        }
        if let Ok(value) = rule.value.parse::<i64>() {
            if !(0..=4000).contains(&value) {
                errors.push(errors::invalid_job_spec(
                    "filters.tagRules[].value (Elo)",
                    &format!("{value} is outside the valid range 0..=4000"),
                ));
            }
        }
    }
}

/// Exposed for [`super::publish`]'s empty-artifact-path derivation and for
/// tests that want a plausible not-yet-existing workspace root without
/// depending on a real app-cache directory.
pub fn deterministic_workspace_root(jobs_root: &Path, job_id: uuid::Uuid) -> PathBuf {
    jobs_root.join(job_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn caps() -> EngineCapabilities {
        crate::engine::capability::pinned_v26_06()
    }

    fn layout(tmp: &Path) -> ValidationLayout {
        ValidationLayout {
            engine: EngineExecutable::new_unverified(PathBuf::from(r"C:\engine\pgn-extract.exe")),
            workspace_root: tmp.join("workspace-not-yet-created"),
            eco_file: PathBuf::from(r"C:\resources\eco.pgn"),
        }
    }

    fn minimal_spec(inputs: Vec<InputFile>, destination: PathBuf) -> JobSpec {
        JobSpec {
            schema_version: CURRENT_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            name: "test".to_string(),
            inputs,
            output: OutputPlan {
                directory: destination,
                base_name: "out".to_string(),
                unique_games: true,
                duplicate_games: DuplicateOutput::None,
                log_file: false,
                manifest: false,
                always_create_audit: false,
                conflict_policy: ConflictPolicy::Fail,
                confirmed_replace: false,
            },
            operations: OperationPlan {
                mode: JobMode::Process,
                duplicates: DuplicatePolicy::None,
                cleanup: CleanupOptions {
                    remove_comments: false,
                    remove_variations: false,
                    remove_nags: false,
                    remove_move_numbers: false,
                    remove_results: false,
                    remove_tags: vec![],
                    reject_bad_results: false,
                    fix_result_tags: false,
                },
                broken: BrokenOutput::Discard,
                eco: EcoOptions { enabled: false },
                output_notation: OutputNotation::San,
                check_file: None,
            },
            filters: FilterPlan {
                tag_rules: vec![],
                move_bounds: None,
                checkmate_only: false,
                setup_policy: SetupPolicy::Any,
                fen_pattern: None,
                textual_variations: vec![],
                advanced_args: vec![],
            },
            runtime: RuntimeOptions {
                use_external_duplicate_table: false,
                count_output_games: true,
            },
        }
    }

    #[test]
    fn empty_inputs_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = minimal_spec(vec![], tmp.path().to_path_buf());
        let outcome = validate_job(&spec, &caps(), &layout(tmp.path()));
        assert!(!outcome.is_ready());
        assert!(outcome
            .errors
            .iter()
            .any(|e| e.code() == ErrorCode::InvalidJobSpec));
    }

    #[test]
    fn missing_input_file_is_input_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist.pgn");
        let spec = minimal_spec(
            vec![InputFile {
                path: missing,
                display_name: "x".to_string(),
                priority: 0,
            }],
            tmp.path().to_path_buf(),
        );
        let outcome = validate_job(&spec, &caps(), &layout(tmp.path()));
        assert!(outcome
            .errors
            .iter()
            .any(|e| e.code() == ErrorCode::InputNotFound));
    }

    #[test]
    fn valid_single_input_produces_no_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("a.pgn");
        std::fs::write(&input, b"[Event \"x\"]\n\n1. e4 e5 1-0\n").unwrap();
        let dest = tmp.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        let spec = minimal_spec(
            vec![InputFile {
                path: input,
                display_name: "a.pgn".to_string(),
                priority: 0,
            }],
            dest,
        );
        let outcome = validate_job(&spec, &caps(), &layout(tmp.path()));
        assert!(
            outcome.is_ready(),
            "unexpected errors: {:?}",
            outcome.errors
        );
        assert!(outcome.estimated_input_bytes > 0);
    }

    #[test]
    fn input_output_collision_is_detected_when_output_dir_is_input_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("out.pgn"); // same name compile() will pick for the final artifact
        std::fs::write(&input, b"[Event \"x\"]\n\n1. e4 e5 1-0\n").unwrap();
        let mut spec = minimal_spec(
            vec![InputFile {
                path: input,
                display_name: "out.pgn".to_string(),
                priority: 0,
            }],
            tmp.path().to_path_buf(),
        );
        spec.output.base_name = "out".to_string(); // final output => <dir>/out.pgn == the input
        let outcome = validate_job(&spec, &caps(), &layout(tmp.path()));
        assert!(outcome
            .errors
            .iter()
            .any(|e| e.code() == ErrorCode::InputOutputCollision));
    }

    #[test]
    fn output_exists_under_fail_policy() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("a.pgn");
        std::fs::write(&input, b"[Event \"x\"]\n\n1. e4 e5 1-0\n").unwrap();
        let dest = tmp.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        std::fs::write(dest.join("out.pgn"), b"pre-existing").unwrap();
        let spec = minimal_spec(
            vec![InputFile {
                path: input,
                display_name: "a.pgn".to_string(),
                priority: 0,
            }],
            dest,
        );
        let outcome = validate_job(&spec, &caps(), &layout(tmp.path()));
        assert!(outcome
            .errors
            .iter()
            .any(|e| e.code() == ErrorCode::OutputExists));
    }

    #[test]
    fn duplicate_input_identity_is_an_advisory_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("a.pgn");
        std::fs::write(&input, b"[Event \"x\"]\n\n1. e4 e5 1-0\n").unwrap();
        let dest = tmp.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        let spec = minimal_spec(
            vec![
                InputFile {
                    path: input.clone(),
                    display_name: "a.pgn".to_string(),
                    priority: 0,
                },
                InputFile {
                    path: input,
                    display_name: "a.pgn (again)".to_string(),
                    priority: 1,
                },
            ],
            dest,
        );
        let outcome = validate_job(&spec, &caps(), &layout(tmp.path()));
        assert!(
            outcome.is_ready(),
            "duplicates must not block: {:?}",
            outcome.errors
        );
        assert_eq!(outcome.advisories.len(), 1);
    }

    #[test]
    fn reserved_device_base_name_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("a.pgn");
        std::fs::write(&input, b"[Event \"x\"]\n\n1. e4 e5 1-0\n").unwrap();
        let mut spec = minimal_spec(
            vec![InputFile {
                path: input,
                display_name: "a.pgn".to_string(),
                priority: 0,
            }],
            tmp.path().to_path_buf(),
        );
        spec.output.base_name = "NUL".to_string();
        let outcome = validate_job(&spec, &caps(), &layout(tmp.path()));
        assert!(!outcome.is_ready());
    }

    #[test]
    fn elo_out_of_range_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("a.pgn");
        std::fs::write(&input, b"[Event \"x\"]\n\n1. e4 e5 1-0\n").unwrap();
        let mut spec = minimal_spec(
            vec![InputFile {
                path: input,
                display_name: "a.pgn".to_string(),
                priority: 0,
            }],
            tmp.path().to_path_buf(),
        );
        spec.filters.tag_rules.push(TagRule {
            tag: TagName::WhiteElo,
            op: TagOp::Ge,
            value: "99999".to_string(),
        });
        let outcome = validate_job(&spec, &caps(), &layout(tmp.path()));
        assert!(!outcome.is_ready());
    }
}
