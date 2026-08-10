// SPDX-License-Identifier: GPL-3.0-or-later
//! Phase 3 integration tests (architecture.md §24 Phase 3 exit criterion:
//! "Exact move-sequence duplicate fixtures produce verified unique and
//! duplicate outputs, including annotated-duplicate warnings").
//!
//! Mirrors `job_orchestration_integration.rs`'s pattern exactly: real
//! fixture PGNs, the real orchestrator, the real checksum-verified bundled
//! sidecar - no mocks. Each test resolves its own independent scratch
//! destination/workspace directories via `tempfile`, so they are safe to
//! run in parallel.
//!
//! Organized into three sections matching the task spec:
//!   A. End-to-end duplicate verification (`-d`/`-D`, retention order,
//!      header-independence, negative cases, empty-audit publication).
//!   B. The annotated-duplicate advisory warning, wired end-to-end.
//!   C. The external duplicate table (`-Z`) workspace lifecycle.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use pgn_studio_lib::domain::{
    ArtifactKind, BrokenOutput, CleanupOptions, ConflictPolicy, DuplicateOutput, DuplicatePolicy,
    EcoOptions, EngineCapabilities, ErrorCode, FilterPlan, InputFile, JobMode, JobSpec, JobStatus,
    OperationPlan, OutputNotation, OutputPlan, RuntimeOptions, SetupPolicy, CURRENT_SCHEMA_VERSION,
};
use pgn_studio_lib::engine::sidecar::{startup_check, SidecarLocation};
use pgn_studio_lib::engine::EngineExecutable;
use pgn_studio_lib::filesystem::count_games_in_file;
use pgn_studio_lib::filesystem::workspace::workspace_root_for;
use pgn_studio_lib::jobs::{run_job, AppState, NullEventSink, RunJobContext};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
}

fn dup_fixture(name: &str) -> PathBuf {
    fixtures_dir().join("duplicates").join(name)
}

fn eco_file_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pgn-extract/eco.pgn")
}

async fn resolve_engine() -> (EngineExecutable, EngineCapabilities) {
    let result = startup_check(&SidecarLocation::dev_default()).await.expect(
        "the real, checksum-pinned sidecar must resolve and pass its startup self-test - if \
             this fails, `src-tauri/binaries/pgn-extract-x86_64-pc-windows-msvc.exe` is missing \
             or does not match the pinned checksum",
    );
    (result.engine, result.capabilities)
}

fn sha256_file(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("fixture must be readable");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn sha256_all(paths: &[PathBuf]) -> Vec<String> {
    paths.iter().map(|p| sha256_file(p)).collect()
}

fn read_to_string(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn no_leftover_temp_files(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            !entries.filter_map(|e| e.ok()).any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".pgnstudio-tmp-")
            })
        })
        .unwrap_or(true)
}

/// A dedup-oriented [`JobSpec`] builder: `Process` mode, inputs ordered by
/// their position in `inputs` (architecture.md §10.7 - input order is
/// retention priority), `Fail` conflict policy, no cleanup/filters/ECO.
/// Individual tests mutate `operations.duplicates`/`output.duplicate_games`/
/// `output.always_create_audit`/`runtime` as needed.
fn dedup_spec(inputs: Vec<PathBuf>, destination_dir: PathBuf, base_name: &str) -> JobSpec {
    JobSpec {
        schema_version: CURRENT_SCHEMA_VERSION,
        id: Uuid::new_v4(),
        name: "duplicate-integration-test".to_string(),
        inputs: inputs
            .into_iter()
            .enumerate()
            .map(|(i, path)| InputFile {
                display_name: path.display().to_string(),
                path,
                priority: i as u32,
            })
            .collect(),
        output: OutputPlan {
            directory: destination_dir,
            base_name: base_name.to_string(),
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

// ===========================================================================
// A. End-to-end duplicate verification
// ===========================================================================

/// `-d` alone (DECISIONS-LEDGER.md D-007 V-2): main output holds only first
/// copies, the audit file holds later copies. Reproduces the exact
/// verified shape (3 games: two duplicates + one unique -> main gets 2,
/// audit gets 1) using `order-a.pgn`/`order-b.pgn` (identical moves,
/// maximally different headers - Event/Site/Date/Round/White/Black all
/// differ) plus a genuinely unique third game.
#[tokio::test]
async fn report_and_keep_first_diverts_duplicates_and_retains_first_copy() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![
        dup_fixture("order-a.pgn"),
        dup_fixture("order-b.pgn"),
        fixtures_dir().join("valid/single-game.pgn"),
    ];
    let hashes_before = sha256_all(&inputs);

    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs.clone(), dest.path().to_path_buf(), "dedup-a1");
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink)
        .await
        .expect("the slot must be free so the job must be accepted");
    assert_eq!(
        result.status,
        JobStatus::Succeeded,
        "error was {:?}",
        result.error
    );

    let main_path = dest.path().join("dedup-a1.pgn");
    let audit_path = dest.path().join("dedup-a1.duplicates.pgn");
    assert!(main_path.exists(), "main output must be published");
    assert!(audit_path.exists(), "audit output must be published");

    assert_eq!(
        count_games_in_file(&main_path).unwrap(),
        2,
        "2 duplicates + 1 unique -> main output must hold exactly 2 games (first copy + unique)"
    );
    assert_eq!(
        count_games_in_file(&audit_path).unwrap(),
        1,
        "exactly 1 later duplicate copy must be diverted to the audit file"
    );

    let main_text = read_to_string(&main_path);
    let audit_text = read_to_string(&audit_path);
    assert!(
        main_text.contains("Order Fixture Alpha"),
        "the FIRST copy in input order (order-a.pgn) must be the one retained in the main output"
    );
    assert!(
        !main_text.contains("Order Fixture Bravo"),
        "the later duplicate copy must not appear in the main output"
    );
    assert!(
        audit_text.contains("Order Fixture Bravo"),
        "the later duplicate copy must be the one diverted to the audit file"
    );
    assert!(
        !audit_text.contains("Order Fixture Alpha"),
        "the retained first copy must not also appear in the audit file"
    );

    // The "metrics trap" (DECISIONS-LEDGER.md D-007 V-2): the engine's own
    // final-summary line reports diverted duplicates as matched, so
    // input_games (from that summary) and output_games (from actually
    // counting the published main output) must legitimately differ here -
    // this asserts the codebase never derives output_games from the
    // summary line's matched count.
    assert_eq!(result.metrics.input_games, Some(3));
    assert_eq!(result.metrics.output_games, Some(2));
    assert_eq!(result.metrics.duplicate_games, Some(1));
    assert_ne!(
        result.metrics.input_games, result.metrics.output_games,
        "input_games (engine summary) and output_games (actual file count) legitimately \
         disagree here - never conflate them"
    );

    assert_eq!(
        sha256_all(&inputs),
        hashes_before,
        "sources must be untouched"
    );
    assert!(no_leftover_temp_files(dest.path()));
}

/// `-D` alone: same main-output retention as `-d`, but no audit file at
/// all (DECISIONS-LEDGER.md D-007 V-1: `-d`/`-D` are mutually exclusive;
/// the two variants produce byte-identical main outputs).
#[tokio::test]
async fn suppress_keep_first_matches_report_mode_main_output_with_no_audit_file() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![
        dup_fixture("order-a.pgn"),
        dup_fixture("order-b.pgn"),
        fixtures_dir().join("valid/single-game.pgn"),
    ];
    let hashes_before = sha256_all(&inputs);

    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs.clone(), dest.path().to_path_buf(), "dedup-a2");
    spec.operations.duplicates = DuplicatePolicy::SuppressKeepFirst;
    // duplicate_games stays `None` - the compiler rejects `Audit` unless
    // the policy is `ReportAndKeepFirst` (there is no file to publish).

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);

    let main_path = dest.path().join("dedup-a2.pgn");
    assert_eq!(count_games_in_file(&main_path).unwrap(), 2);
    let main_text = read_to_string(&main_path);
    assert!(main_text.contains("Order Fixture Alpha"));
    assert!(!main_text.contains("Order Fixture Bravo"));

    assert!(
        !dest.path().join("dedup-a2.duplicates.pgn").exists(),
        "-D must never produce an audit file"
    );
    assert_eq!(sha256_all(&inputs), hashes_before);
    assert!(no_leftover_temp_files(dest.path()));
}

/// THE single most consequential duplicate behavior for users
/// (architecture.md §10.7: "input order is a retention priority"):
/// forward order keeps `order-a.pgn`'s copy.
#[tokio::test]
async fn input_order_forward_keeps_alpha_and_diverts_bravo() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![dup_fixture("order-a.pgn"), dup_fixture("order-b.pgn")];
    let hashes_before = sha256_all(&inputs);
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs.clone(), dest.path().to_path_buf(), "order-forward");
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);

    let main_text = read_to_string(&dest.path().join("order-forward.pgn"));
    let audit_text = read_to_string(&dest.path().join("order-forward.duplicates.pgn"));
    assert!(
        main_text.contains("Order Fixture Alpha") && !main_text.contains("Order Fixture Bravo"),
        "listed first -> order-a.pgn's copy must be the one kept"
    );
    assert!(
        audit_text.contains("Order Fixture Bravo") && !audit_text.contains("Order Fixture Alpha"),
        "listed second -> order-b.pgn's copy must be the one diverted"
    );
    assert_eq!(
        sha256_all(&inputs),
        hashes_before,
        "sources must be untouched"
    );
}

/// The mirror of the previous test with input order reversed: this alone
/// changes which copy survives, with no other setting touched. If this
/// test and `input_order_forward_keeps_alpha_and_diverts_bravo` both pass,
/// retention is proven to track input order, not file content or name.
#[tokio::test]
async fn input_order_reversed_keeps_bravo_and_diverts_alpha() {
    let (engine, caps) = resolve_engine().await;
    // Only the order is swapped relative to the forward test.
    let inputs = vec![dup_fixture("order-b.pgn"), dup_fixture("order-a.pgn")];
    let hashes_before = sha256_all(&inputs);
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs.clone(), dest.path().to_path_buf(), "order-reversed");
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);

    let main_text = read_to_string(&dest.path().join("order-reversed.pgn"));
    let audit_text = read_to_string(&dest.path().join("order-reversed.duplicates.pgn"));
    assert!(
        main_text.contains("Order Fixture Bravo") && !main_text.contains("Order Fixture Alpha"),
        "listed first this time -> order-b.pgn's copy must now be the one kept"
    );
    assert!(
        audit_text.contains("Order Fixture Alpha") && !audit_text.contains("Order Fixture Bravo"),
        "listed second this time -> order-a.pgn's copy must now be the one diverted"
    );
    assert_eq!(
        sha256_all(&inputs),
        hashes_before,
        "sources must be untouched"
    );
}

/// Headers do not affect duplicate identity (architecture.md §10.7), using
/// the pre-existing `same-moves-different-headers.pgn` fixture (identical
/// moves, different Event/Site/Date/Round) within a single input file, in
/// its natural order.
#[tokio::test]
async fn duplicate_identity_ignores_event_site_round_header_differences() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![dup_fixture("same-moves-different-headers.pgn")];
    let hashes_before = sha256_all(&inputs);
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(
        inputs.clone(),
        dest.path().to_path_buf(),
        "headers-irrelevant",
    );
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);

    let main_path = dest.path().join("headers-irrelevant.pgn");
    let audit_path = dest.path().join("headers-irrelevant.duplicates.pgn");
    assert_eq!(
        count_games_in_file(&main_path).unwrap(),
        1,
        "same moves, different Event/Site/Date/Round -> still a duplicate pair, main keeps 1"
    );
    assert_eq!(count_games_in_file(&audit_path).unwrap(), 1);
    assert!(read_to_string(&main_path).contains("Fixture Open 2025"));
    assert!(read_to_string(&audit_path).contains("Fixture Winter Invitational"));
    assert_eq!(
        sha256_all(&inputs),
        hashes_before,
        "sources must be untouched"
    );
}

/// Negative case: same players across two rounds, but genuinely different
/// openings/moves/results, must NOT be flagged as a duplicate despite
/// matching headers (`same-players-different-games.pgn`).
#[tokio::test]
async fn negative_case_same_players_different_games_are_not_flagged_as_duplicates() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![dup_fixture("same-players-different-games.pgn")];
    let hashes_before = sha256_all(&inputs);
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(
        inputs.clone(),
        dest.path().to_path_buf(),
        "negative-players",
    );
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;
    spec.output.always_create_audit = true; // force publication so "empty" is observable

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);

    let main_path = dest.path().join("negative-players.pgn");
    assert_eq!(
        count_games_in_file(&main_path).unwrap(),
        2,
        "both games must survive - matching White/Black headers alone must not trigger dedup"
    );
    let audit_path = dest.path().join("negative-players.duplicates.pgn");
    assert!(
        audit_path.exists(),
        "always_create_audit publishes even an empty audit file"
    );
    assert_eq!(std::fs::metadata(&audit_path).unwrap().len(), 0);
    assert_eq!(result.metrics.duplicate_games, Some(0));
    assert_eq!(
        sha256_all(&inputs),
        hashes_before,
        "sources must be untouched"
    );
}

/// Negative case: a truncated score sharing its exact opening prefix with
/// a complete game must NOT be treated as a duplicate of it
/// (`truncated-vs-complete.pgn`).
#[tokio::test]
async fn negative_case_truncated_score_is_not_flagged_as_duplicate_of_its_complete_prefix() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![dup_fixture("truncated-vs-complete.pgn")];
    let hashes_before = sha256_all(&inputs);
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(
        inputs.clone(),
        dest.path().to_path_buf(),
        "negative-truncated",
    );
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;
    spec.output.always_create_audit = true;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);

    let main_path = dest.path().join("negative-truncated.pgn");
    assert_eq!(
        count_games_in_file(&main_path).unwrap(),
        2,
        "a truncated score sharing an opening prefix must not be merged away as a duplicate"
    );
    let audit_path = dest.path().join("negative-truncated.duplicates.pgn");
    assert_eq!(std::fs::metadata(&audit_path).unwrap().len(), 0);
    assert_eq!(
        sha256_all(&inputs),
        hashes_before,
        "sources must be untouched"
    );
}

/// §11.6: "Do not create empty duplicate/broken files unless the user
/// enabled 'Always create audit artifacts.'" Proven against the REAL
/// engine's own diverted-duplicates temp file (which the engine always
/// creates for `-d<path>`, even when it ends up empty), not just the
/// synthetic-temp-file unit test in `filesystem::publish`.
#[tokio::test]
async fn empty_duplicates_audit_is_not_published_by_default() {
    let (engine, caps) = resolve_engine().await;
    // Two genuinely different games -> zero duplicates found.
    let inputs = vec![
        dup_fixture("order-a.pgn"),
        fixtures_dir().join("valid/single-game.pgn"),
    ];
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs, dest.path().to_path_buf(), "no-empty-audit");
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;
    spec.output.always_create_audit = false;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);
    assert_eq!(
        count_games_in_file(&dest.path().join("no-empty-audit.pgn")).unwrap(),
        2
    );
    assert!(
        !dest.path().join("no-empty-audit.duplicates.pgn").exists(),
        "an empty audit file must not be published when always_create_audit is off"
    );
    assert!(!result
        .artifacts
        .iter()
        .any(|a| a.kind == ArtifactKind::DuplicateGames));
}

#[tokio::test]
async fn empty_duplicates_audit_is_published_when_always_create_audit_is_set() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![
        dup_fixture("order-a.pgn"),
        fixtures_dir().join("valid/single-game.pgn"),
    ];
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs, dest.path().to_path_buf(), "with-empty-audit");
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;
    spec.output.always_create_audit = true;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);
    let audit_path = dest.path().join("with-empty-audit.duplicates.pgn");
    assert!(
        audit_path.exists(),
        "always_create_audit must publish it even when empty"
    );
    assert_eq!(std::fs::metadata(&audit_path).unwrap().len(), 0);
    let audit_artifact = result
        .artifacts
        .iter()
        .find(|a| a.kind == ArtifactKind::DuplicateGames)
        .expect("the (empty) audit artifact must be reported on the result");
    assert_eq!(audit_artifact.size_bytes, 0);
}

// ===========================================================================
// B. Annotated-duplicate advisory warning, wired end-to-end
// ===========================================================================

/// The positive case: `annotated-vs-plain.pgn`'s plain copy is listed
/// first (kept), its annotated copy is listed second (diverted to the
/// audit file) - the warning must fire, name the count, and identify the
/// game.
#[tokio::test]
async fn annotated_duplicate_in_audit_file_produces_a_warning() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![dup_fixture("annotated-vs-plain.pgn")];
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs, dest.path().to_path_buf(), "annotated-warn");
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);
    assert_eq!(
        count_games_in_file(&dest.path().join("annotated-warn.pgn")).unwrap(),
        1
    );
    assert_eq!(
        count_games_in_file(&dest.path().join("annotated-warn.duplicates.pgn")).unwrap(),
        1
    );

    let warning = result
        .warnings
        .iter()
        .find(|w| w.code() == ErrorCode::AnnotatedDuplicatesSuppressed)
        .expect("a warning must be emitted when the audit file holds an annotated duplicate");
    assert!(warning.message().contains("1 suppressed duplicate game"));
    assert!(
        warning.message().contains("Wagner, Wolf vs Xu, Xia"),
        "the warning should identify which game, got: {}",
        warning.message()
    );
    assert!(
        !warning.message().to_lowercase().contains("is worse"),
        "must never claim the discarded copy is worse (architecture.md §3.3/§10.7, ADR-009)"
    );
}

/// The false-positive-avoidance case: `annotated-first-then-plain.pgn` has
/// its ANNOTATED copy listed first (kept) and its PLAIN copy listed second
/// (diverted). The audit file therefore holds no annotations at all, even
/// though the collection as a whole does - no warning must fire. This is
/// what proves the scan is honestly scoped to what was actually
/// discarded, not "was there ever an annotation anywhere."
#[tokio::test]
async fn no_annotation_warning_when_only_the_kept_copy_has_annotations() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![dup_fixture("annotated-first-then-plain.pgn")];
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs, dest.path().to_path_buf(), "no-warn-kept-annotated");
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);
    assert_eq!(
        count_games_in_file(&dest.path().join("no-warn-kept-annotated.duplicates.pgn")).unwrap(),
        1
    );
    assert!(
        !result
            .warnings
            .iter()
            .any(|w| w.code() == ErrorCode::AnnotatedDuplicatesSuppressed),
        "the discarded copy is plain, so no warning may fire even though the KEPT copy (not \
         scanned) has annotations"
    );
}

/// `-D` produces no audit file at all, so there is nothing to scan - the
/// warning must never fire regardless of the input's content.
#[tokio::test]
async fn no_annotation_warning_when_duplicates_are_suppressed_without_an_audit_file() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![dup_fixture("annotated-vs-plain.pgn")];
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(inputs, dest.path().to_path_buf(), "no-warn-suppress");
    spec.operations.duplicates = DuplicatePolicy::SuppressKeepFirst;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);
    assert!(!dest.path().join("no-warn-suppress.duplicates.pgn").exists());
    assert!(!result
        .warnings
        .iter()
        .any(|w| w.code() == ErrorCode::AnnotatedDuplicatesSuppressed));
}

// ===========================================================================
// C. External duplicate table (`-Z`) workspace lifecycle
// ===========================================================================

/// Normal completion: `-Z` produces the same correct dedup result as
/// without it (`identical-pair.pgn`, reused from the existing fixture set),
/// and `virtual.tmp` is gone from the job workspace afterward - the
/// engine's own `clear_duplicate_hash_table()` unlinks it on normal exit
/// (pgn-extract `hashing.c`), not any PGN Studio cleanup code.
#[tokio::test]
async fn external_duplicate_table_normal_run_cleans_up_virtual_tmp() {
    let (engine, caps) = resolve_engine().await;
    let inputs = vec![dup_fixture("identical-pair.pgn")];
    let hashes_before = sha256_all(&inputs);
    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(
        inputs.clone(),
        dest.path().to_path_buf(),
        "external-table-normal",
    );
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;
    spec.runtime.use_external_duplicate_table = true;
    let job_id = spec.id;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();
    assert_eq!(result.status, JobStatus::Succeeded, "{:?}", result.error);

    // Same correctness as the non-`-Z` path (identical-pair.pgn is two
    // byte-identical games): main keeps 1, audit gets 1.
    assert_eq!(
        count_games_in_file(&dest.path().join("external-table-normal.pgn")).unwrap(),
        1
    );
    assert_eq!(
        count_games_in_file(&dest.path().join("external-table-normal.duplicates.pgn")).unwrap(),
        1
    );

    let virtual_tmp = workspace_root_for(jobs_root.path(), job_id).join("virtual.tmp");
    assert!(
        !virtual_tmp.exists(),
        "the engine must have unlinked its own virtual.tmp on normal exit"
    );
    assert_eq!(
        sha256_all(&inputs),
        hashes_before,
        "sources must be untouched"
    );
}

/// A large-enough synthetic fixture that a real `-Z` run through the real
/// sidecar takes a comfortable multi-second margin to complete - see
/// `job_orchestration_integration.rs`'s own calibration note for this
/// exact technique and its measured timing (100,000 games ~1.3s of engine
/// time). Not committed to the repo; pure test scratch data.
fn write_large_synthetic_fixture(path: &Path, game_count: u32) {
    use std::io::Write;
    let file = std::fs::File::create(path).unwrap();
    let mut writer = std::io::BufWriter::new(file);
    for i in 0..game_count {
        writeln!(
            writer,
            "[Event \"Synthetic\"]\n[Site \"Test\"]\n[Date \"2026.01.01\"]\n[Round \"{i}\"]\n\
             [White \"A{i}\"]\n[Black \"B{i}\"]\n[Result \"1-0\"]\n\n\
             1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 1-0\n"
        )
        .unwrap();
    }
    writer.flush().unwrap();
}

/// Proves both remaining task-C claims at once: `virtual.tmp` really is
/// created in the per-job workspace (observed directly, mid-run, by
/// polling for it), and cancellation - not the engine, which never gets to
/// its own normal-exit cleanup when killed - is what removes it
/// (`jobs::run`'s `EngineRunResult::Cancelled` arm explicitly adds
/// `workspace.virtual_tmp_path()` to its cleanup list).
#[tokio::test]
async fn external_duplicate_table_creates_virtual_tmp_and_cancellation_removes_it() {
    let (engine, caps) = resolve_engine().await;

    let src_dir = tempfile::tempdir().unwrap();
    let big_input = src_dir.path().join("large-synthetic.pgn");
    write_large_synthetic_fixture(&big_input, 100_000);
    let original_hash = sha256_file(&big_input);

    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = dedup_spec(
        vec![big_input.clone()],
        dest.path().to_path_buf(),
        "external-table-cancel",
    );
    spec.operations.duplicates = DuplicatePolicy::ReportAndKeepFirst;
    spec.output.duplicate_games = DuplicateOutput::Audit;
    spec.runtime.use_external_duplicate_table = true;
    let job_id = spec.id;

    let state = std::sync::Arc::new(AppState::new());
    let state_for_task = state.clone();
    let jobs_root_path = jobs_root.path().to_path_buf();

    let handle = tokio::spawn(async move {
        let ctx = RunJobContext {
            caps: &caps,
            engine: &engine,
            jobs_root: &jobs_root_path,
            eco_file: &eco_file,
        };
        run_job(spec, &ctx, &state_for_task, &NullEventSink).await
    });

    // Poll for virtual.tmp's existence rather than a blind sleep: the
    // engine creates it during argument parsing / hash-table init, before
    // any game is processed (pgn-extract `main.c`/`hashing.c`), so it
    // should appear within milliseconds - well before this 100,000-game
    // run would naturally finish (~1-4s).
    let virtual_tmp = workspace_root_for(jobs_root.path(), job_id).join("virtual.tmp");
    let mut observed = false;
    for _ in 0..150 {
        if virtual_tmp.exists() {
            observed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        observed,
        "virtual.tmp must appear in the job workspace ({}) almost immediately after spawn when \
         -Z is set",
        virtual_tmp.display()
    );

    state
        .request_cancel(job_id)
        .expect("the job must still be active well before a 100,000-game run finishes");

    let result = handle
        .await
        .expect("run_job task must not panic")
        .expect("the slot was free at start_job time");
    assert_eq!(result.status, JobStatus::Cancelled);

    assert!(
        !virtual_tmp.exists(),
        "cancellation must remove virtual.tmp (the engine itself never reached its own \
         normal-exit cleanup, since it was terminated)"
    );
    assert_eq!(
        sha256_file(&big_input),
        original_hash,
        "source must be untouched"
    );
    assert!(!dest.path().join("external-table-cancel.pgn").exists());
    assert!(no_leftover_temp_files(dest.path()));
}
