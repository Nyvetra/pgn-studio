// SPDX-License-Identifier: GPL-3.0-or-later
//! The pure command compiler (architecture.md §10.5; design-02 §1).
//!
//! `compile` is the heart of the system: a pure, total function from a
//! [`JobSpec`] plus [`EngineCapabilities`] plus a resolved [`CompileLayout`]
//! to a [`CompiledEngineCommand`]. No I/O, no clock, no randomness, no
//! panics. Every rule enforced here is cited against
//! `DECISIONS-LEDGER.md` D-007 (empirically verified against the real
//! engine binary) or design-02's source-cited flag table (§1.3) — this
//! module intentionally does not "improve on" or second-guess those
//! findings.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::domain::{
    ArtifactKind, BrokenOutput, DuplicateOutput, DuplicatePolicy, EngineCapabilities, InputFile,
    JobMode, JobSpec, SetupPolicy,
};

use super::criteria;
use super::EngineExecutable;

/// Everything the compiler needs from the impure world, resolved by the
/// application layer **before** compilation (design-02 §1.1). Every path
/// here is a precondition-guaranteed canonical absolute path; `compile`
/// trusts them without re-validating (re-validation, e.g. existence/
/// writability, is filesystem I/O and belongs to Phase 1b's `validate_job`).
#[derive(Debug, Clone)]
pub struct CompileLayout {
    /// Verified sidecar path.
    pub engine: EngineExecutable,
    /// `<app-cache>/jobs/<job-uuid>/` — criteria files and (if `-Z`)
    /// `virtual.tmp` land here (design-02 §3.3, D-7).
    pub workspace_root: PathBuf,
    /// Bundled `resources/pgn-extract/eco.pgn`, absolute.
    pub eco_file: PathBuf,
    /// Canonicalized form of `spec.output.directory` (design-02 D-8: temp
    /// outputs live in the destination directory, not the workspace, so
    /// publication is a same-volume rename).
    pub destination_dir: PathBuf,
}

/// The compiled, ready-to-spawn engine invocation. Spawning it is Phase 1b's
/// job; this module only ever *describes* the invocation.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledEngineCommand {
    pub executable: EngineExecutable,
    /// Exact argv\[1..\]; one token per element. Never built by string
    /// concatenation — every element here is what a shell-free
    /// `CreateProcessW`/`posix_spawn` call receives verbatim.
    pub args: Vec<OsString>,
    pub working_directory: PathBuf,
    pub generated_files: Vec<GeneratedCriteriaFile>,
    pub temporary_outputs: Vec<TemporaryOutput>,
    pub final_outputs: Vec<FinalOutput>,
    /// Human-inspection rendering only (§1.6). Never executed — see this
    /// module's `render_display_command` doc comment for the structural
    /// argument for why that guarantee holds.
    pub display_command: String,
    pub metrics_plan: MetricsPlan,
}

/// A criteria file the caller must write to `workspace_root` before
/// spawning (design-02 §1.1, §1.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCriteriaFile {
    pub relative_path: &'static str,
    pub content: String,
    pub sha256: String,
}

/// An output the engine itself creates as a side effect of an argv flag
/// (`-o`/`-d`), living in `destination_dir` under a deterministic temp name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporaryOutput {
    pub kind: ArtifactKind,
    pub path: PathBuf,
}

/// A path the orchestrator should atomically publish a temp output to (or,
/// for the log/report kinds, write directly to) after a successful run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalOutput {
    pub kind: ArtifactKind,
    pub path: PathBuf,
    pub publish_if_empty: bool,
}

/// Which optional [`crate::domain::ProcessingMetrics`] fields this specific
/// compiled command can, in principle, produce (design-02 §2.4).
///
/// Judgment call: design-02 references a `metrics_plan: MetricsPlan` field
/// on `CompiledEngineCommand` (§1.1) and describes *rules* for what is
/// derivable in §2.4, but never spells out `MetricsPlan`'s own field list.
/// This shape is inferred directly from those rules: one flag per optional
/// `ProcessingMetrics` field (`input_files`/`input_bytes` are excluded here
/// because §2.4 says they are "from validation stats (always)" — a Phase 1b
/// concern unrelated to the compiled command's shape). A `true` here means
/// "attempting to derive this metric is meaningful for this spec", not "a
/// value is guaranteed" — a run can still fail before the postflight stage
/// that would actually compute it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricsPlan {
    /// Always true: `-s --summary` is unconditional (O-1/O-2).
    pub processed_games: bool,
    /// Always true: the final summary line is always parsed on exit 0,
    /// regardless of mode or filters.
    pub input_games: bool,
    pub output_games: bool,
    pub duplicate_games: bool,
    /// **Always `false` in V1 — never derivable.** design-02 §2.4 originally
    /// scoped this to "no filters active, and duplicate policy is `None` or
    /// `ReportAndKeepFirst`", reasoning that filtered-out-but-valid games
    /// were the only way `total.saturating_sub(matched)` (the final
    /// `"N games matched out of M."` summary line) could be confused for a
    /// broken-game count. Phase 4 empirically falsified that "narrow case is
    /// safe" claim against the real pinned engine, with no filters active and
    /// no duplicate handling involved at all: a game missing its result
    /// marker (`fixtures/malformed/missing-result-marker.pgn`) is silently
    /// invisible to *both* `M` and the matched count whenever it is the last
    /// game in the merged input stream (`0 games matched out of 0` for that
    /// fixture alone — the malformed game is not just unmatched, it is never
    /// counted as "processed" at all), while the exact same fixture merged
    /// ahead of a trailing valid game *is* counted as processed and matched,
    /// but has its entire move list silently discarded from the published
    /// output (replaced with just the bare `Result` tag value) — again with
    /// zero effect on `total - matched`. Both are real data-loss/quality
    /// events that this arithmetic reports as `0` broken games, indistinguishable
    /// from a genuinely clean run. Since `compile` is pure (no filesystem
    /// access) it cannot know ahead of time whether a job's input contains
    /// this pattern, so no configuration-based gate can rescue the
    /// derivation — unlike the filter case, this is a property of engine
    /// *parse-recovery* behavior, not job configuration. Per the project's
    /// binding "never substitute 0 for a metric that could not be measured"
    /// rule (`domain::result::ProcessingMetrics`), the only honest value is
    /// `false` here, always, so `ProcessingMetrics.broken_games` stays `None`
    /// on every job. See `phase4_integration.rs`'s
    /// `broken_games_metric_stays_none_even_though_a_game_was_silently_dropped`
    /// and `missing_result_marker_is_invisible_to_the_matched_total_summary`
    /// for the reproducing fixtures and real-engine proof.
    pub broken_games: bool,
    pub output_bytes: bool,
}

/// Everything that can go wrong compiling a structurally-typed [`JobSpec`]
/// (design-02 §1.1). `compile` is total: every reachable failure mode ends
/// here, never in a panic.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompileError {
    /// The spec asked for something this engine build's capability map does
    /// not advertise, or something out of V1's scope by design (e.g. a
    /// non-empty `advancedArgs`). Mirrors architecture.md §29: never
    /// silently dropped, downgraded, or approximated.
    #[error("unsupported engine option \"{option}\": {reason}")]
    UnsupportedOption {
        option: &'static str,
        reason: String,
    },
    /// The spec is not structurally valid, independent of engine
    /// capability — e.g. no inputs, a relative path, an unrepresentable
    /// criteria value, or an internally-inconsistent combination of fields.
    #[error("invalid job spec field \"{field}\": {reason}")]
    InvalidSpec { field: String, reason: String },
}

const TEMP_PREFIX: &str = ".pgnstudio-tmp-";

/// Compiles a [`JobSpec`] into a [`CompiledEngineCommand`].
///
/// Pure and total: no filesystem access, no clock, no randomness, no
/// panics. Deterministic: calling this twice with equal arguments produces
/// byte-identical output (temp file names are derived from `spec.id`, never
/// from a fresh random/time source), which is what makes exact-argv golden
/// tests possible at all.
pub fn compile(
    spec: &JobSpec,
    caps: &EngineCapabilities,
    layout: &CompileLayout,
) -> Result<CompiledEngineCommand, CompileError> {
    validate_structural(spec)?;
    validate_capabilities(spec, caps)?;

    let tags_file =
        criteria::render_tags_file(&spec.filters.tag_rules, spec.filters.fen_pattern.as_ref())?;
    let variations_file = criteria::render_variations_file(&spec.filters.textual_variations)?;

    let simple_id = spec.id.simple().to_string();
    // `Uuid::simple()` always yields exactly 32 lowercase hex characters, so
    // this slice never panics.
    let id_prefix = &simple_id[..12];

    let mut args: Vec<OsString> = Vec::new();
    let mut generated_files: Vec<GeneratedCriteriaFile> = Vec::new();
    let mut temporary_outputs: Vec<TemporaryOutput> = Vec::new();
    let mut final_outputs: Vec<FinalOutput> = Vec::new();

    // O-1, O-2 (hard: -s before --summary — assignment vs OR, table row 30).
    args.push(OsString::from("-s"));
    args.push(OsString::from("--summary"));

    // O-3
    if spec.runtime.use_external_duplicate_table {
        args.push(OsString::from("-Z"));
    }

    // O-4 (hard: never both -d and -D; ValidateOnly forces DuplicatePolicy::None
    // via validate_structural, so this arm naturally emits nothing under -r).
    let mut temp_dupes_path: Option<PathBuf> = None;
    match spec.operations.duplicates {
        DuplicatePolicy::None => {}
        DuplicatePolicy::SuppressKeepFirst => {
            args.push(OsString::from("-D"));
        }
        DuplicatePolicy::ReportAndKeepFirst => {
            let path = layout
                .destination_dir
                .join(format!("{TEMP_PREFIX}{id_prefix}-duplicates.pgn"));
            args.push(attached_flag("-d", &path));
            temp_dupes_path = Some(path);
        }
    }

    // O-5 (preset only: "New Games Against Master", Decision D-11).
    if let Some(check_file) = &spec.operations.check_file {
        args.push(attached_flag("-c", check_file));
    }

    // O-6: filter flags.
    if let Some(bounds) = &spec.filters.move_bounds {
        // Hard: --maxmoves MUST precede --minmoves (DECISIONS-LEDGER.md
        // D-007 V-3) — reversed, the engine silently drops the upper bound
        // whenever max < 2*min - 1 and exits 0 anyway.
        if let Some(max) = bounds.max {
            args.push(OsString::from("--maxmoves"));
            args.push(OsString::from(max.to_string()));
        }
        if let Some(min) = bounds.min {
            args.push(OsString::from("--minmoves"));
            args.push(OsString::from(min.to_string()));
        }
    }
    if spec.filters.checkmate_only {
        args.push(OsString::from("--checkmate"));
    }
    match spec.filters.setup_policy {
        SetupPolicy::Any => {}
        SetupPolicy::StandardStartOnly => args.push(OsString::from("--nosetuptags")),
        SetupPolicy::SetupOnly => args.push(OsString::from("--onlysetuptags")),
    }
    if let Some(tags) = &tags_file {
        let path = layout.workspace_root.join("criteria").join("tags.txt");
        args.push(attached_flag("-t", &path));
        generated_files.push(GeneratedCriteriaFile {
            relative_path: "criteria/tags.txt",
            content: tags.content.clone(),
            sha256: tags.sha256.clone(),
        });
    }
    if let Some(vars) = &variations_file {
        let path = layout
            .workspace_root
            .join("criteria")
            .join("variations.txt");
        args.push(attached_flag("-v", &path));
        generated_files.push(GeneratedCriteriaFile {
            relative_path: "criteria/variations.txt",
            content: vars.content.clone(),
            sha256: vars.sha256.clone(),
        });
    }

    // O-7: cleanup flags, then --detag*, then --nobadresults/--fixresulttags,
    // then --keepbroken (see doc comment above `validate_structural` for why
    // --keepbroken lives here: design-02's own O-1..O-11 table never places
    // it, a gap this compiler closes rather than leaving unimplemented).
    let cleanup = &spec.operations.cleanup;
    if cleanup.remove_comments {
        args.push(OsString::from("-C"));
    }
    if cleanup.remove_nags {
        args.push(OsString::from("-N"));
    }
    if cleanup.remove_variations {
        args.push(OsString::from("-V"));
    }
    if cleanup.remove_move_numbers {
        args.push(OsString::from("--nomovenumbers"));
    }
    if cleanup.remove_results {
        args.push(OsString::from("--noresults"));
    }
    for tag in &cleanup.remove_tags {
        args.push(OsString::from("--detag"));
        args.push(OsString::from(tag.clone()));
    }
    if cleanup.reject_bad_results {
        args.push(OsString::from("--nobadresults"));
    }
    if cleanup.fix_result_tags {
        args.push(OsString::from("--fixresulttags"));
    }
    if spec.operations.broken == BrokenOutput::KeepInMainOutput {
        args.push(OsString::from("--keepbroken"));
    }

    // O-8
    if spec.operations.eco.enabled {
        args.push(attached_flag("-e", &layout.eco_file));
    }

    // O-9 / O-10
    let mut temp_unique_path: Option<PathBuf> = None;
    match spec.operations.mode {
        JobMode::ValidateOnly => {
            args.push(OsString::from("-r"));
        }
        JobMode::Process => {
            if spec.output.unique_games {
                let path = layout
                    .destination_dir
                    .join(format!("{TEMP_PREFIX}{id_prefix}-unique.pgn"));
                args.push(attached_flag("-o", &path));
                temp_unique_path = Some(path);
            }
        }
    }

    // O-11 (hard: input order == duplicate-retention priority, T-5, §10.7).
    let mut ordered_inputs: Vec<&InputFile> = spec.inputs.iter().collect();
    ordered_inputs.sort_by_key(|f| f.priority);
    for input in ordered_inputs {
        args.push(input.path.as_os_str().to_owned());
    }

    // Temporary/final output planning.
    if let Some(path) = &temp_unique_path {
        temporary_outputs.push(TemporaryOutput {
            kind: ArtifactKind::UniqueGames,
            path: path.clone(),
        });
        final_outputs.push(FinalOutput {
            kind: ArtifactKind::UniqueGames,
            path: layout
                .destination_dir
                .join(format!("{}.pgn", spec.output.base_name)),
            // D-21: the main output publishes even when empty.
            publish_if_empty: true,
        });
    }
    if let Some(path) = &temp_dupes_path {
        temporary_outputs.push(TemporaryOutput {
            kind: ArtifactKind::DuplicateGames,
            path: path.clone(),
        });
        if spec.output.duplicate_games == DuplicateOutput::Audit {
            final_outputs.push(FinalOutput {
                kind: ArtifactKind::DuplicateGames,
                path: layout
                    .destination_dir
                    .join(format!("{}.duplicates.pgn", spec.output.base_name)),
                publish_if_empty: spec.output.always_create_audit,
            });
        }
    }
    if spec.output.log_file {
        final_outputs.push(FinalOutput {
            kind: ArtifactKind::LogText,
            path: layout
                .destination_dir
                .join(format!("{}.log.txt", spec.output.base_name)),
            publish_if_empty: true,
        });
    }
    if spec.output.manifest {
        final_outputs.push(FinalOutput {
            kind: ArtifactKind::ReportJson,
            path: layout
                .destination_dir
                .join(format!("{}.report.json", spec.output.base_name)),
            publish_if_empty: true,
        });
        final_outputs.push(FinalOutput {
            kind: ArtifactKind::ReportText,
            path: layout
                .destination_dir
                .join(format!("{}.report.txt", spec.output.base_name)),
            publish_if_empty: true,
        });
    }

    let metrics_plan = MetricsPlan {
        processed_games: true,
        input_games: true,
        output_games: temp_unique_path.is_some() && spec.runtime.count_output_games,
        duplicate_games: spec.output.duplicate_games == DuplicateOutput::Audit
            && temp_dupes_path.is_some()
            && spec.runtime.count_output_games,
        // Never derivable — see the field doc on `MetricsPlan::broken_games`
        // for the empirical (Phase 4) reason this is unconditionally false,
        // not gated on filters/duplicate policy the way it used to be.
        broken_games: false,
        output_bytes: !final_outputs.is_empty(),
    };

    let display_command = render_display_command(&args);

    Ok(CompiledEngineCommand {
        executable: layout.engine.clone(),
        args,
        working_directory: layout.workspace_root.clone(),
        generated_files,
        temporary_outputs,
        final_outputs,
        display_command,
        metrics_plan,
    })
}

/// Structural/consistency validation that needs no [`EngineCapabilities`]
/// (spec-shape rules that would hold even against a hypothetical engine
/// that supported everything).
fn validate_structural(spec: &JobSpec) -> Result<(), CompileError> {
    if spec.schema_version != crate::domain::CURRENT_SCHEMA_VERSION {
        return Err(CompileError::InvalidSpec {
            field: "schemaVersion".to_string(),
            reason: format!(
                "expected schema version {}, got {}",
                crate::domain::CURRENT_SCHEMA_VERSION,
                spec.schema_version
            ),
        });
    }

    // T-8: the engine reads stdin with zero input files; the compiler
    // requires at least one so a mis-compiled command can never hang.
    if spec.inputs.is_empty() {
        return Err(CompileError::InvalidSpec {
            field: "inputs".to_string(),
            reason: "at least one input file is required".to_string(),
        });
    }
    for input in &spec.inputs {
        if !input.path.is_absolute() {
            return Err(CompileError::InvalidSpec {
                field: "inputs[].path".to_string(),
                reason: format!(
                    "path must be absolute (there is no end-of-options marker, so a relative \
                     path starting with '-' would be parsed as a flag): {}",
                    input.path.display()
                ),
            });
        }
    }

    if spec.output.base_name.trim().is_empty() {
        return Err(CompileError::InvalidSpec {
            field: "output.baseName".to_string(),
            reason: "must not be empty".to_string(),
        });
    }

    if spec.output.conflict_policy == crate::domain::ConflictPolicy::ReplaceAfterConfirmation
        && !spec.output.confirmed_replace
    {
        return Err(CompileError::InvalidSpec {
            field: "output.confirmedReplace".to_string(),
            reason: "conflictPolicy \"replaceAfterConfirmation\" requires confirmedReplace to \
                     be true, set only after an explicit UI confirmation dialog"
                .to_string(),
        });
    }

    // ValidateOnly (-r) structurally excludes -o/-d/-D (design-02 row 13,
    // canonical order O-9). Rather than silently ignoring unique_games/
    // duplicates when the caller also set them, reject the contradiction
    // (architecture.md §29).
    if spec.operations.mode == JobMode::ValidateOnly {
        if spec.output.unique_games {
            return Err(CompileError::InvalidSpec {
                field: "output.uniqueGames".to_string(),
                reason: "validateOnly mode (-r) never produces a unique-games output; set \
                         output.uniqueGames to false or use operations.mode = \"process\""
                    .to_string(),
            });
        }
        if spec.operations.duplicates != DuplicatePolicy::None {
            return Err(CompileError::InvalidSpec {
                field: "operations.duplicates".to_string(),
                reason: "validateOnly mode (-r) is incompatible with duplicate handling \
                         (neither -d nor -D is emitted under -r); set operations.duplicates \
                         to \"none\" or use operations.mode = \"process\""
                    .to_string(),
            });
        }
    }

    if spec.output.duplicate_games == DuplicateOutput::Audit
        && spec.operations.duplicates != DuplicatePolicy::ReportAndKeepFirst
    {
        return Err(CompileError::InvalidSpec {
            field: "output.duplicateGames".to_string(),
            reason: "a duplicates audit artifact can only be published when \
                     operations.duplicates is \"reportAndKeepFirst\" (that is the only policy \
                     under which the engine writes a diverted-duplicates file at all)"
                .to_string(),
        });
    }

    if let Some(check_file) = &spec.operations.check_file {
        if !check_file.is_absolute() {
            return Err(CompileError::InvalidSpec {
                field: "operations.checkFile".to_string(),
                reason: "must be an absolute path".to_string(),
            });
        }
        let has_pgn_extension = check_file
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("pgn"))
            .unwrap_or(false);
        if !has_pgn_extension {
            return Err(CompileError::InvalidSpec {
                field: "operations.checkFile".to_string(),
                reason: "must end in \".pgn\" (case-insensitive); the engine reads any other \
                         extension as a list of check-file names, not a single master database"
                    .to_string(),
            });
        }
        if spec.operations.duplicates == DuplicatePolicy::None {
            return Err(CompileError::InvalidSpec {
                field: "operations.checkFile".to_string(),
                reason: "a check file requires operations.duplicates to be \
                         \"reportAndKeepFirst\" or \"suppressKeepFirst\" — \"-c\" alone does \
                         nothing (duplicate detection against the master only activates \
                         alongside -d/-D)"
                    .to_string(),
            });
        }
    }

    if let Some(bounds) = &spec.filters.move_bounds {
        for value in [bounds.min, bounds.max].into_iter().flatten() {
            if !(1..=4999).contains(&value) {
                return Err(CompileError::InvalidSpec {
                    field: "filters.moveBounds".to_string(),
                    reason: format!("bound {value} is outside the valid range 1..=4999"),
                });
            }
        }
        if let (Some(min), Some(max)) = (bounds.min, bounds.max) {
            if min > max {
                return Err(CompileError::InvalidSpec {
                    field: "filters.moveBounds".to_string(),
                    reason: format!("min ({min}) must not exceed max ({max})"),
                });
            }
        }
    }

    for tag in &spec.operations.cleanup.remove_tags {
        if !is_valid_tag_identifier(tag) {
            return Err(CompileError::InvalidSpec {
                field: "operations.cleanup.removeTags[]".to_string(),
                reason: format!(
                    "\"{tag}\" is not a valid tag name; expected to match ^[A-Za-z][A-Za-z0-9]*$"
                ),
            });
        }
    }

    if !spec.filters.advanced_args.is_empty() {
        return Err(CompileError::UnsupportedOption {
            option: "filters.advancedArgs",
            reason: "raw advanced engine arguments are reserved for a future version and must \
                     be empty in V1"
                .to_string(),
        });
    }

    Ok(())
}

fn is_valid_tag_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric())
}

/// Capability-gated validation: everything the pinned build might not
/// support (architecture.md §29 totality rule).
fn validate_capabilities(spec: &JobSpec, caps: &EngineCapabilities) -> Result<(), CompileError> {
    if spec.runtime.use_external_duplicate_table && !caps.external_duplicate_table {
        return Err(CompileError::UnsupportedOption {
            option: "runtime.useExternalDuplicateTable",
            reason: "the disk-backed duplicate table (-Z) is not supported by this engine build"
                .to_string(),
        });
    }

    match spec.operations.duplicates {
        DuplicatePolicy::None => {}
        DuplicatePolicy::SuppressKeepFirst if !caps.duplicate_detection => {
            return Err(CompileError::UnsupportedOption {
                option: "operations.duplicates",
                reason: "duplicate suppression (-D) is not supported by this engine build"
                    .to_string(),
            });
        }
        DuplicatePolicy::ReportAndKeepFirst
            if !(caps.duplicate_detection && caps.duplicate_audit_file) =>
        {
            return Err(CompileError::UnsupportedOption {
                option: "operations.duplicates",
                reason: "the duplicates audit file (-d) is not supported by this engine build"
                    .to_string(),
            });
        }
        _ => {}
    }

    if spec.operations.check_file.is_some() && !caps.check_file {
        return Err(CompileError::UnsupportedOption {
            option: "operations.checkFile",
            reason: "check-file comparison (-c) is not supported by this engine build".to_string(),
        });
    }

    if spec.filters.fen_pattern.is_some() && !caps.fen_patterns {
        return Err(CompileError::UnsupportedOption {
            option: "filters.fenPattern",
            reason: "FEN pattern filters are not supported by this engine build".to_string(),
        });
    }

    let has_variation_text = spec
        .filters
        .textual_variations
        .iter()
        .any(|v| !v.trim().is_empty());
    if has_variation_text && !caps.textual_variations {
        return Err(CompileError::UnsupportedOption {
            option: "filters.textualVariations",
            reason: "textual opening-line filters (-v) are not supported by this engine build"
                .to_string(),
        });
    }

    if spec.operations.cleanup.reject_bad_results && !caps.reject_bad_results {
        return Err(CompileError::UnsupportedOption {
            option: "operations.cleanup.rejectBadResults",
            reason: "--nobadresults is not supported by this engine build".to_string(),
        });
    }
    if spec.operations.cleanup.fix_result_tags && !caps.fix_result_tags {
        return Err(CompileError::UnsupportedOption {
            option: "operations.cleanup.fixResultTags",
            reason: "--fixresulttags is not supported by this engine build".to_string(),
        });
    }

    if spec.operations.eco.enabled && !caps.eco_classification {
        return Err(CompileError::UnsupportedOption {
            option: "operations.eco.enabled",
            reason: "ECO classification (-e) is not supported by this engine build".to_string(),
        });
    }

    if !caps
        .supported_output_formats
        .contains(&spec.operations.output_notation)
    {
        return Err(CompileError::UnsupportedOption {
            option: "operations.outputNotation",
            reason: format!(
                "{:?} output notation is not supported by this engine build",
                spec.operations.output_notation
            ),
        });
    }

    Ok(())
}

/// One attached-form flag token: `-o` + path → `-oC:\path\to\file`, always a
/// single [`OsString`] (DECISIONS-LEDGER.md D-007 V-4: the separated form
/// `-e <path>` is a catastrophic silent-data-loss hazard for at least `-e`,
/// so every value-bearing short flag uniformly uses the attached form —
/// design-02 Decision D-2).
fn attached_flag(flag: &str, path: &Path) -> OsString {
    let mut token = OsString::from(flag);
    token.push(path.as_os_str());
    token
}

/// Renders `display_command` (design-02 §1.6): bare program name (never the
/// sidecar's absolute path — avoids leaking install paths into
/// screenshots), each argv token space-joined, any token containing a byte
/// outside `[A-Za-z0-9_./:\-]` quoted per platform convention.
///
/// **Never-executed guarantee.** This function returns a plain [`String`].
/// Nothing in this crate parses it back into an argument list or hands it
/// to a shell — the only spawn API this codebase has (Phase 1b) takes a
/// `&CompiledEngineCommand` and reads its `args: Vec<OsString>` directly.
/// Even a value containing `$(...)`, `&&`, or `%CD%` therefore round-trips
/// as inert display text; see this module's test
/// `display_command_is_inert_text_even_with_shell_metacharacters`.
fn render_display_command(args: &[OsString]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(args.len() + 1);
    parts.push("pgn-extract".to_string());
    parts.extend(args.iter().map(|a| quote_for_display(a)));
    parts.join(" ")
}

fn quote_for_display(token: &OsStr) -> String {
    let text = token.to_string_lossy();
    let is_safe = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | ':' | '\\' | '-'));
    if is_safe {
        return text.into_owned();
    }
    if cfg!(windows) {
        let mut out = String::with_capacity(text.len() + 2);
        out.push('"');
        for c in text.chars() {
            if c == '"' {
                out.push('"');
            }
            out.push(c);
        }
        out.push('"');
        out
    } else {
        let mut out = String::with_capacity(text.len() + 2);
        out.push('\'');
        for c in text.chars() {
            if c == '\'' {
                out.push_str("'\\''");
            } else {
                out.push(c);
            }
        }
        out.push('\'');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CleanupOptions, ConflictPolicy, EcoOptions, FilterPlan, MoveBounds, OperationPlan,
        OutputNotation, OutputPlan, RuntimeOptions,
    };
    use uuid::Uuid;

    fn minimal_caps() -> EngineCapabilities {
        crate::engine::capability::pinned_v26_06()
    }

    fn minimal_layout() -> CompileLayout {
        CompileLayout {
            engine: EngineExecutable::new_unverified(PathBuf::from(r"C:\engine\pgn-extract.exe")),
            workspace_root: PathBuf::from(r"C:\workspace\job-1"),
            eco_file: PathBuf::from(r"C:\resources\eco.pgn"),
            destination_dir: PathBuf::from(r"C:\dest"),
        }
    }

    fn minimal_spec() -> JobSpec {
        JobSpec {
            schema_version: crate::domain::CURRENT_SCHEMA_VERSION,
            id: Uuid::nil(),
            name: "test".to_string(),
            inputs: vec![InputFile {
                path: PathBuf::from(r"C:\in\a.pgn"),
                display_name: "a.pgn".to_string(),
                priority: 0,
            }],
            output: OutputPlan {
                directory: PathBuf::from(r"C:\dest"),
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
    fn minimal_spec_compiles_to_merge_command() {
        let spec = minimal_spec();
        let compiled = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
        assert_eq!(
            compiled.args,
            vec![
                OsString::from("-s"),
                OsString::from("--summary"),
                OsString::from(format!(
                    r"-oC:\dest\.pgnstudio-tmp-{}-unique.pgn",
                    &spec.id.simple().to_string()[..12]
                )),
                OsString::from(r"C:\in\a.pgn"),
            ]
        );
    }

    #[test]
    fn empty_inputs_is_invalid_spec() {
        let mut spec = minimal_spec();
        spec.inputs.clear();
        let err = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap_err();
        assert!(matches!(err, CompileError::InvalidSpec { field, .. } if field == "inputs"));
    }

    #[test]
    fn relative_input_path_is_invalid_spec() {
        let mut spec = minimal_spec();
        spec.inputs[0].path = PathBuf::from("relative.pgn");
        let err = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap_err();
        assert!(matches!(err, CompileError::InvalidSpec { .. }));
    }

    #[test]
    fn both_d_and_capital_d_are_never_emitted_together() {
        // Structural: DuplicatePolicy is a closed 3-way enum, so there is no
        // spec that could ask for both -d and -D at once — the illegal
        // state is unrepresentable. This test documents that invariant by
        // exhaustively checking each variant emits at most one of them.
        for policy in [
            DuplicatePolicy::None,
            DuplicatePolicy::ReportAndKeepFirst,
            DuplicatePolicy::SuppressKeepFirst,
        ] {
            let mut spec = minimal_spec();
            spec.operations.duplicates = policy;
            if policy == DuplicatePolicy::ReportAndKeepFirst {
                spec.output.duplicate_games = DuplicateOutput::Audit;
            }
            let compiled = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
            let has_d = compiled
                .args
                .iter()
                .any(|a| a.to_string_lossy().starts_with("-d"));
            let has_capital_d = compiled.args.iter().any(|a| a == "-D");
            assert!(
                !(has_d && has_capital_d),
                "policy {policy:?} emitted both -d and -D"
            );
        }
    }

    #[test]
    fn validate_only_rejects_unique_games_true() {
        let mut spec = minimal_spec();
        spec.operations.mode = JobMode::ValidateOnly;
        // unique_games left true (default in minimal_spec) -> contradiction.
        let err = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap_err();
        assert!(
            matches!(err, CompileError::InvalidSpec { field, .. } if field == "output.uniqueGames")
        );
    }

    #[test]
    fn validate_only_with_consistent_flags_emits_dash_r_only() {
        let mut spec = minimal_spec();
        spec.operations.mode = JobMode::ValidateOnly;
        spec.output.unique_games = false;
        let compiled = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
        assert_eq!(
            compiled.args,
            vec![
                OsString::from("-s"),
                OsString::from("--summary"),
                OsString::from("-r"),
                OsString::from(r"C:\in\a.pgn"),
            ]
        );
    }

    #[test]
    fn uci_output_notation_is_unsupported() {
        let mut spec = minimal_spec();
        spec.operations.output_notation = OutputNotation::Uci;
        let err = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap_err();
        assert!(matches!(err, CompileError::UnsupportedOption { .. }));
    }

    #[test]
    fn non_empty_advanced_args_is_unsupported() {
        let mut spec = minimal_spec();
        spec.filters.advanced_args = vec!["--foo".to_string()];
        let err = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap_err();
        assert!(matches!(
            err,
            CompileError::UnsupportedOption {
                option: "filters.advancedArgs",
                ..
            }
        ));
    }

    #[test]
    fn check_file_without_duplicate_policy_is_rejected() {
        let mut spec = minimal_spec();
        spec.operations.check_file = Some(PathBuf::from(r"C:\master.pgn"));
        let err = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap_err();
        assert!(
            matches!(err, CompileError::InvalidSpec { field, .. } if field == "operations.checkFile")
        );
    }

    #[test]
    fn check_file_requiring_pgn_suffix() {
        let mut spec = minimal_spec();
        spec.operations.duplicates = DuplicatePolicy::SuppressKeepFirst;
        spec.operations.check_file = Some(PathBuf::from(r"C:\master.txt"));
        let err = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap_err();
        assert!(
            matches!(err, CompileError::InvalidSpec { field, .. } if field == "operations.checkFile")
        );
    }

    #[test]
    fn move_bounds_min_greater_than_max_is_rejected() {
        let mut spec = minimal_spec();
        spec.filters.move_bounds = Some(MoveBounds {
            min: Some(20),
            max: Some(10),
        });
        let err = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap_err();
        assert!(
            matches!(err, CompileError::InvalidSpec { field, .. } if field == "filters.moveBounds")
        );
    }

    #[test]
    fn broken_games_metric_not_derivable_with_filters() {
        let mut spec = minimal_spec();
        spec.filters.checkmate_only = true;
        let compiled = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
        assert!(!compiled.metrics_plan.broken_games);
    }

    #[test]
    fn broken_games_metric_not_derivable_with_suppress_keep_first() {
        let mut spec = minimal_spec();
        spec.operations.duplicates = DuplicatePolicy::SuppressKeepFirst;
        let compiled = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
        assert!(!compiled.metrics_plan.broken_games);
    }

    /// Phase 4 correction (see `MetricsPlan::broken_games`'s field doc): this
    /// scenario — no filters, `ReportAndKeepFirst` — is exactly the case
    /// design-02 §2.4 originally called "safe" and this test used to assert
    /// `true` for. Empirical testing against the real engine
    /// (`phase4_integration.rs`) proved that claim false: a game missing its
    /// result marker can be silently dropped from, or have its moves
    /// silently stripped from, a published output with **zero** effect on
    /// `total - matched`, in this exact configuration. `broken_games` is now
    /// unconditionally `false`, so this test documents "still false here
    /// too" rather than the old "true here specifically".
    #[test]
    fn broken_games_metric_is_never_derivable_not_even_in_the_previously_assumed_safe_case() {
        let mut spec = minimal_spec();
        spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
        spec.output.duplicate_games = DuplicateOutput::Audit;
        let compiled = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
        assert!(!compiled.metrics_plan.broken_games);
    }

    #[test]
    fn deterministic_across_repeated_calls() {
        let spec = minimal_spec();
        let a = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
        let b = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn display_command_is_inert_text_even_with_shell_metacharacters() {
        let mut spec = minimal_spec();
        spec.inputs[0].path = PathBuf::from(r"C:\in\$(rm -rf /) && evil %CD%.pgn");
        let compiled = compile(&spec, &minimal_caps(), &minimal_layout()).unwrap();
        // The raw text is present (quoted for display), but nothing in this
        // crate ever parses display_command back into a command: the only
        // spawn surface (Phase 1b) reads `compiled.args` directly.
        assert!(compiled.display_command.contains("$(rm -rf /)"));
        assert!(compiled.display_command.contains("&&"));
        assert!(compiled.display_command.contains("%CD%"));
        assert!(compiled.display_command.starts_with("pgn-extract "));
        // The dangerous path is exactly one argv element, verbatim.
        assert!(compiled
            .args
            .iter()
            .any(|a| a == OsStr::new(r"C:\in\$(rm -rf /) && evil %CD%.pgn")));
    }
}
