// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden command tests G-1..G-12 (design-02 §1.8; architecture.md §20.2;
//! Phase 1a task scope section E).
//!
//! Each test asserts the **exact argv array** `compile` produces for a
//! specific scenario, not a display string — per architecture.md §20.2:
//! "Test argument arrays, not only display strings." Temp file names are
//! deterministic from `spec.id` (a fixed `Uuid::from_u128(N)` per test), so
//! every expected array below is a literal, reproducible value.

use std::ffi::OsString;
use std::path::PathBuf;

use uuid::Uuid;

use crate::domain::*;
use crate::engine::capability::pinned_v26_06;
use crate::engine::command_compiler::{compile, CompileError, CompileLayout};
use crate::engine::EngineExecutable;

// Absolute fixture roots, spelled per platform. `compile` runs
// `validate_structural` first, which rejects any input path that is not
// `Path::is_absolute()` - and `C:\in\a.pgn` is a *relative*
// single-component path on Unix. So a Windows-only literal here would not
// merely look out of place on macOS: every test would take the
// InvalidSpec path instead of the scenario it is supposed to exercise.
#[cfg(windows)]
mod roots {
    pub const ENGINE_PATH: &str = r"C:\engine\pgn-extract.exe";
    pub const WORKSPACE_ROOT: &str = r"C:\ws\job";
    pub const ECO_FILE: &str = r"C:\resources\pgn-extract\eco.pgn";
    pub const DEST_DIR: &str = r"C:\dest";
    pub const IN_DIR: &str = r"C:\in";
    pub const MASTER_FILE: &str = r"C:\master\master.pgn";
    /// Spaces, an ampersand, parentheses and Bengali script in one path -
    /// design-02 §1.8 G-11's own example.
    pub const TORTURE_PATH: &str = r"C:\t t\a&b(1)\ঢাকা.pgn";
}

#[cfg(not(windows))]
mod roots {
    pub const ENGINE_PATH: &str = "/engine/pgn-extract";
    pub const WORKSPACE_ROOT: &str = "/ws/job";
    pub const ECO_FILE: &str = "/resources/pgn-extract/eco.pgn";
    pub const DEST_DIR: &str = "/dest";
    pub const IN_DIR: &str = "/in";
    pub const MASTER_FILE: &str = "/master/master.pgn";
    /// Deliberately the same character classes as the Windows fixture -
    /// space, ampersand, parentheses, Bengali script - so G-11 asserts the
    /// same property on both platforms rather than a weaker one here.
    pub const TORTURE_PATH: &str = "/t t/a&b(1)/ঢাকা.pgn";
}

use roots::*;

/// Builds an expected path by pushing components one at a time, so the
/// separator is the platform's own and the expectation is produced by the
/// same path arithmetic the compiler uses (`PathBuf::join`) rather than
/// restated with a hardcoded separator that only matches on Windows.
///
/// Components must be pushed individually: `join("criteria/tags.txt")`
/// keeps the embedded forward slash verbatim on Windows and would not
/// match `join("criteria").join("tags.txt")`.
fn under(root: &str, components: &[&str]) -> String {
    let mut path = PathBuf::from(root);
    for component in components {
        path.push(component);
    }
    path.to_string_lossy().into_owned()
}

/// The standard input fixture path, `IN_DIR/<name>`.
fn in_file(name: &str) -> String {
    under(IN_DIR, &[name])
}

fn layout() -> CompileLayout {
    CompileLayout {
        engine: EngineExecutable::new_unverified(PathBuf::from(ENGINE_PATH)),
        workspace_root: PathBuf::from(WORKSPACE_ROOT),
        eco_file: PathBuf::from(ECO_FILE),
        destination_dir: PathBuf::from(DEST_DIR),
    }
}

fn input(path: &str, priority: u32) -> InputFile {
    InputFile {
        path: PathBuf::from(path),
        display_name: path.to_string(),
        priority,
    }
}

/// A minimal, structurally-valid spec: one input (unless overridden),
/// `Process` mode, `unique_games: true`, everything else off. Each test
/// mutates only the fields the scenario cares about.
fn base_spec(id: Uuid, inputs: Vec<InputFile>) -> JobSpec {
    JobSpec {
        schema_version: CURRENT_SCHEMA_VERSION,
        id,
        name: "golden".to_string(),
        inputs,
        output: OutputPlan {
            directory: PathBuf::from(DEST_DIR),
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

fn tmpu(id: Uuid) -> String {
    under(
        DEST_DIR,
        &[&format!(
            ".pgnstudio-tmp-{}-unique.pgn",
            &id.simple().to_string()[..12]
        )],
    )
}

fn tmpd(id: Uuid) -> String {
    under(
        DEST_DIR,
        &[&format!(
            ".pgnstudio-tmp-{}-duplicates.pgn",
            &id.simple().to_string()[..12]
        )],
    )
}

#[test]
fn g1_merge_two_files() {
    let id = Uuid::from_u128(1);
    let spec = base_spec(
        id,
        vec![input(&in_file("a.pgn"), 0), input(&in_file("b.pgn"), 1)],
    );
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
        OsString::from(in_file("b.pgn")),
    ];
    assert_eq!(compiled.args, expected);
}

#[test]
fn g2_clean_collection_dedupe_and_audit() {
    let id = Uuid::from_u128(2);
    let mut spec = base_spec(
        id,
        vec![input(&in_file("a.pgn"), 0), input(&in_file("b.pgn"), 1)],
    );
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from(format!("-d{}", tmpd(id))),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
        OsString::from(in_file("b.pgn")),
    ];
    assert_eq!(compiled.args, expected);
}

#[test]
fn g3_suppress_duplicates_no_audit() {
    let id = Uuid::from_u128(3);
    let mut spec = base_spec(
        id,
        vec![input(&in_file("a.pgn"), 0), input(&in_file("b.pgn"), 1)],
    );
    spec.operations.duplicates = DuplicatePolicy::SuppressKeepFirst;
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from("-D"),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
        OsString::from(in_file("b.pgn")),
    ];
    assert_eq!(compiled.args, expected);
}

#[test]
fn g4_minimal_mainline_dedupe_and_strip() {
    let id = Uuid::from_u128(4);
    let mut spec = base_spec(id, vec![input(&in_file("a.pgn"), 0)]);
    spec.operations.duplicates = DuplicatePolicy::SuppressKeepFirst;
    spec.operations.cleanup.remove_comments = true;
    spec.operations.cleanup.remove_nags = true;
    spec.operations.cleanup.remove_variations = true;
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from("-D"),
        OsString::from("-C"),
        OsString::from("-N"),
        OsString::from("-V"),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
    ];
    assert_eq!(compiled.args, expected);
}

#[test]
fn g5_validate_only() {
    let id = Uuid::from_u128(5);
    let mut spec = base_spec(id, vec![input(&in_file("a.pgn"), 0)]);
    spec.operations.mode = JobMode::ValidateOnly;
    spec.output.unique_games = false;
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from("-r"),
        OsString::from(in_file("a.pgn")),
    ];
    assert_eq!(compiled.args, expected);
}

#[test]
fn g6_add_eco() {
    let id = Uuid::from_u128(6);
    let mut spec = base_spec(id, vec![input(&in_file("a.pgn"), 0)]);
    spec.operations.eco.enabled = true;
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from(format!("-e{ECO_FILE}")),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
    ];
    assert_eq!(compiled.args, expected);
}

#[test]
fn g7_tal_games_1960_1969() {
    let id = Uuid::from_u128(7);
    let mut spec = base_spec(id, vec![input(&in_file("a.pgn"), 0)]);
    spec.filters.tag_rules = vec![
        TagRule {
            tag: TagName::Player,
            op: TagOp::Prefix,
            value: "Tal".to_string(),
        },
        TagRule {
            tag: TagName::Date,
            op: TagOp::Ge,
            value: "1960".to_string(),
        },
        TagRule {
            tag: TagName::Date,
            op: TagOp::Le,
            value: "1969".to_string(),
        },
    ];
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from(format!(
            "-t{}",
            under(WORKSPACE_ROOT, &["criteria", "tags.txt"])
        )),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
    ];
    assert_eq!(compiled.args, expected);

    assert_eq!(compiled.generated_files.len(), 1);
    assert_eq!(
        compiled.generated_files[0].relative_path,
        "criteria/tags.txt"
    );
    assert_eq!(
        compiled.generated_files[0].content,
        "Player \"Tal\"\nDate >= \"1960.01.01\"\nDate <= \"1969.12.31\"\n"
    );
}

#[test]
fn g8_move_bounds_30_40_order_regression() {
    let id = Uuid::from_u128(8);
    let mut spec = base_spec(id, vec![input(&in_file("a.pgn"), 0)]);
    let bounds = MoveBounds {
        min: Some(30),
        max: Some(40),
    };
    spec.filters.move_bounds = Some(bounds);
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from("--maxmoves"),
        OsString::from("40"),
        OsString::from("--minmoves"),
        OsString::from("30"),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
    ];
    assert_eq!(compiled.args, expected);

    // Explicit regression guard (task section B; DECISIONS-LEDGER.md D-007
    // V-3): --maxmoves MUST precede --minmoves. 30/40 is inside the
    // empirically verified trigger zone (max < 2*min - 1, i.e. 40 < 59) —
    // unlike e.g. min=10/max=20, which would NOT reproduce the engine's
    // silent-upper-bound-drop bug and would give false confidence (the
    // task's own explicit warning about that specific false-negative case).
    // Computed from the fixture's own `bounds`, not restated as literals, so
    // this can't degrade into an always-true constant assertion.
    let min = bounds.min.expect("fixture always sets min");
    let max = bounds.max.expect("fixture always sets max");
    assert!(
        max < 2 * min - 1,
        "test fixture must be inside the verified trigger zone (max < 2*min - 1), got \
         min={min} max={max}"
    );
    let maxmoves_index = compiled
        .args
        .iter()
        .position(|a| a == "--maxmoves")
        .unwrap();
    let minmoves_index = compiled
        .args
        .iter()
        .position(|a| a == "--minmoves")
        .unwrap();
    assert!(
        maxmoves_index < minmoves_index,
        "--maxmoves must be emitted before --minmoves: reversed order silently drops the \
         upper bound and the engine still exits 0 (DECISIONS-LEDGER.md D-007 V-3)"
    );
}

#[test]
fn g9_new_games_against_master() {
    let id = Uuid::from_u128(9);
    let mut spec = base_spec(id, vec![input(&in_file("a.pgn"), 0)]);
    spec.operations.duplicates = DuplicatePolicy::SuppressKeepFirst;
    spec.operations.check_file = Some(PathBuf::from(MASTER_FILE));
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from("-D"),
        OsString::from(format!("-c{MASTER_FILE}")),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
    ];
    assert_eq!(compiled.args, expected);
}

#[test]
fn g10_external_table_and_audit() {
    let id = Uuid::from_u128(10);
    let mut spec = base_spec(id, vec![input(&in_file("a.pgn"), 0)]);
    spec.runtime.use_external_duplicate_table = true;
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();
    let expected: Vec<OsString> = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        OsString::from("-Z"),
        OsString::from(format!("-d{}", tmpd(id))),
        OsString::from(format!("-o{}", tmpu(id))),
        OsString::from(in_file("a.pgn")),
    ];
    assert_eq!(compiled.args, expected);
}

#[test]
fn g11_windows_path_torture() {
    let id = Uuid::from_u128(11);
    // Spaces, an ampersand, parentheses, and Bengali script, all in one
    // path — design-02 §1.8 G-11's own example. Name kept as G-11 for
    // traceability to that section; the fixture itself is per-platform
    // (see `roots`) so the same property is asserted on macOS.
    let torture_path = TORTURE_PATH;
    let spec = base_spec(id, vec![input(torture_path, 0)]);
    let compiled = compile(&spec, &pinned_v26_06(), &layout()).unwrap();

    let expected_token = OsString::from(torture_path);
    assert!(
        compiled.args.iter().any(|a| a == &expected_token),
        "the torture path must appear as a single, byte-for-byte unmodified argv token \
         (no shell ever sees or re-tokenizes it)"
    );
    // Inputs are last (O-11); with one input and no other trailing flags,
    // it must be the final argv element.
    assert_eq!(compiled.args.last(), Some(&expected_token));
}

#[test]
fn g12_unsupported_output_notation_produces_no_argv() {
    let id = Uuid::from_u128(12);
    let mut spec = base_spec(id, vec![input(&in_file("a.pgn"), 0)]);
    spec.operations.output_notation = OutputNotation::Uci;
    let result = compile(&spec, &pinned_v26_06(), &layout());
    match result {
        Err(CompileError::UnsupportedOption { option, .. }) => {
            assert_eq!(option, "operations.outputNotation");
        }
        other => panic!("expected Err(CompileError::UnsupportedOption), got {other:?}"),
    }
}
