// SPDX-License-Identifier: GPL-3.0-or-later
//! Phase 1b integration tests - the Phase 1 exit criterion (task
//! "Definition of done"): real fixture PGNs, run through the real
//! orchestrator against the real, checksum-verified bundled sidecar, with
//! source-file byte-identity asserted before and after every run.
//!
//! These tests spawn the actual `pgn-extract-x86_64-pc-windows-msvc.exe`
//! sidecar (via [`pgn_studio_lib::engine::sidecar::startup_check`]) - they
//! are genuine end-to-end tests, not mocks. Each test resolves its own
//! independent scratch destination/workspace directories via `tempfile`, so
//! they are safe to run in parallel (the default `cargo test` behavior).

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use pgn_studio_lib::domain::{
    BrokenOutput, CleanupOptions, ConflictPolicy, DuplicateOutput, DuplicatePolicy, EcoOptions,
    EngineCapabilities, ErrorCode, FilterPlan, InputFile, JobMode, JobSpec, JobStatus,
    OperationPlan, OutputNotation, OutputPlan, RuntimeOptions, CURRENT_SCHEMA_VERSION,
};
use pgn_studio_lib::engine::sidecar::{startup_check, SidecarLocation};
use pgn_studio_lib::engine::EngineExecutable;
use pgn_studio_lib::filesystem::count_games_in_file;
use pgn_studio_lib::jobs::{run_job, AppState, NullEventSink, RunJobContext};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
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

/// A minimal, valid merge [`JobSpec`]: emits the unique-games output only,
/// no cleanup/filters/ECO/duplicate handling, `Fail` conflict policy.
/// Individual tests mutate the fields they care about.
fn merge_spec(inputs: Vec<PathBuf>, destination_dir: PathBuf, base_name: &str) -> JobSpec {
    JobSpec {
        schema_version: CURRENT_SCHEMA_VERSION,
        id: Uuid::new_v4(),
        name: "integration-test".to_string(),
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
            setup_policy: pgn_studio_lib::domain::SetupPolicy::Any,
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

// ===========================================================================
// 1. The headline test (task "Definition of done" 1-6 + architecture §25)
// ===========================================================================

#[tokio::test]
async fn real_merge_preserves_sources_and_publishes_expected_output() {
    let (engine, caps) = resolve_engine().await;
    let source_a = fixtures_dir().join("golden/merge-source-a.pgn");
    let source_b = fixtures_dir().join("golden/merge-source-b.pgn");
    assert!(
        source_a.is_file() && source_b.is_file(),
        "golden fixtures must exist"
    );

    // 2. Record SHA-256 before the run.
    let sha_a_before = sha256_file(&source_a);
    let sha_b_before = sha256_file(&source_b);

    let dest = tempfile::tempdir().unwrap();
    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let spec = merge_spec(
        vec![source_a.clone(), source_b.clone()],
        dest.path().to_path_buf(),
        "merged",
    );

    // 3. Run a real merge through the orchestrator using the real bundled
    // sidecar.
    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink)
        .await
        .expect("the slot must be free (no other job) so the job must be accepted");

    assert_eq!(
        result.status,
        JobStatus::Succeeded,
        "job must succeed; error was {:?}",
        result.error
    );

    // 4. Assert the merged output exists and contains the expected game
    // count. merge-source-a.pgn has 2 games, merge-source-b.pgn has 2
    // games (one byte-identical to a.pgn's first game, by design - see
    // fixtures/golden/README.md); this spec performs a plain merge with no
    // deduplication, so the output must contain all 4.
    let output_path = dest.path().join("merged.pgn");
    assert!(output_path.exists(), "merged.pgn must be published");
    assert_eq!(
        count_games_in_file(&output_path).unwrap(),
        4,
        "plain merge of a 2-game and a 2-game fixture, no dedup, must yield 4 games"
    );
    assert_eq!(
        result.metrics.input_games,
        Some(4),
        "the engine's own final-summary count must also report 4"
    );

    // 5. Assert both source files are byte-identical afterwards
    // (architecture §25's headline safety criterion).
    assert_eq!(
        sha256_file(&source_a),
        sha_a_before,
        "merge-source-a.pgn must be byte-identical after the run"
    );
    assert_eq!(
        sha256_file(&source_b),
        sha_b_before,
        "merge-source-b.pgn must be byte-identical after the run"
    );

    // 6. No temp files remain in the destination directory.
    assert!(
        no_leftover_temp_files(dest.path()),
        "no .pgnstudio-tmp-* files may remain in the destination directory"
    );
}

// ===========================================================================
// 2. Input/output collision rejection
// ===========================================================================

#[tokio::test]
async fn input_output_collision_is_rejected_and_source_is_untouched() {
    let (engine, caps) = resolve_engine().await;
    let dir = tempfile::tempdir().unwrap();
    // The output would be `<dir>/collide.pgn` - make the *input* be exactly
    // that same file, so publication would otherwise overwrite its own
    // source.
    let input_path = dir.path().join("collide.pgn");
    let original_content = b"[Event \"Original\"]\n\n1. e4 e5 1-0\n".to_vec();
    std::fs::write(&input_path, &original_content).unwrap();
    let original_hash = sha256_file(&input_path);

    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let spec = merge_spec(
        vec![input_path.clone()],
        dir.path().to_path_buf(),
        "collide",
    );

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();

    assert_eq!(result.status, JobStatus::Failed);
    let error = result.error.expect("a collision must produce an error");
    assert_eq!(error.code(), ErrorCode::InputOutputCollision);

    // The source must be completely untouched - byte-identical, not just
    // "still exists".
    assert_eq!(std::fs::read(&input_path).unwrap(), original_content);
    assert_eq!(sha256_file(&input_path), original_hash);
    assert!(no_leftover_temp_files(dir.path()));
}

// ===========================================================================
// 3. `Fail` conflict policy must not overwrite
// ===========================================================================

#[tokio::test]
async fn fail_conflict_policy_never_overwrites_an_existing_output() {
    let (engine, caps) = resolve_engine().await;
    let source = fixtures_dir().join("valid/single-game.pgn");
    let dest = tempfile::tempdir().unwrap();

    let precious_path = dest.path().join("precious.pgn");
    let precious_content = b"THIS CONTENT MUST NEVER BE OVERWRITTEN".to_vec();
    std::fs::write(&precious_path, &precious_content).unwrap();

    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let mut spec = merge_spec(vec![source], dest.path().to_path_buf(), "precious");
    spec.output.conflict_policy = ConflictPolicy::Fail;

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();

    assert_eq!(result.status, JobStatus::Failed);
    let error = result
        .error
        .expect("a pre-existing output must produce an error");
    assert_eq!(error.code(), ErrorCode::OutputExists);

    // The pre-existing file must be byte-for-byte untouched - this is the
    // core safety property `Fail` exists to guarantee.
    assert_eq!(std::fs::read(&precious_path).unwrap(), precious_content);
    assert!(no_leftover_temp_files(dest.path()));
}

// ===========================================================================
// 4. Cancellation leaves sources and prior outputs untouched
// ===========================================================================

/// Writes a large-enough synthetic PGN (never committed to the repo - pure
/// test scratch data, generated fresh each run) that a real merge through
/// the real sidecar takes over a second, so cancellation can be triggered
/// reliably (a comfortable margin, not a hair-trigger race). Calibrated
/// empirically against the real sidecar for this task: 300,000 minimal
/// games (~44 MB) took ~4 real-world seconds for a plain merge, i.e.
/// roughly 13 µs/game; 100,000 games (~1.3 s of expected engine time)
/// leaves a >4x safety margin over the 300 ms delay this test waits
/// before cancelling, while keeping the file (and this test's SHA-256
/// verification of it, which is unoptimized in debug builds) small.
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

#[tokio::test]
async fn cancellation_leaves_sources_and_prior_outputs_untouched() {
    let (engine, caps) = resolve_engine().await;

    let src_dir = tempfile::tempdir().unwrap();
    let big_input = src_dir.path().join("large-synthetic.pgn");
    write_large_synthetic_fixture(&big_input, 100_000);
    let original_hash = sha256_file(&big_input);

    let dest = tempfile::tempdir().unwrap();
    let prior_output = dest.path().join("prior-run-output.txt");
    let prior_content = b"output from an earlier, unrelated run - must survive".to_vec();
    std::fs::write(&prior_output, &prior_content).unwrap();

    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    let spec = merge_spec(
        vec![big_input.clone()],
        dest.path().to_path_buf(),
        "cancel-me",
    );
    let job_id = spec.id;

    let state = std::sync::Arc::new(AppState::new());
    let state_for_task = state.clone();
    let dest_path = dest.path().to_path_buf();
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
    let _ = &dest_path; // (kept for readability of intent above; used below via `dest`)

    // Give the job a moment to actually start the engine, then cancel it
    // well before the ~2-4s it needs to finish processing 200,000 games.
    tokio::time::sleep(Duration::from_millis(300)).await;
    state
        .request_cancel(job_id)
        .expect("the job must still be active 300ms into a multi-second run");

    let result = handle
        .await
        .expect("run_job task must not panic")
        .expect("the slot was free at start_job time, so this must be Ok");

    assert_eq!(result.status, JobStatus::Cancelled);

    // The source is never opened for write by this codebase at all, but
    // assert it explicitly anyway - this is the property that matters.
    assert_eq!(sha256_file(&big_input), original_hash);

    // A previously-published, unrelated file in the destination directory
    // must survive completely untouched.
    assert_eq!(std::fs::read(&prior_output).unwrap(), prior_content);

    // The cancelled job's own output must not have been published at all.
    assert!(!dest.path().join("cancel-me.pgn").exists());

    // No temp files left behind (design-02 §2.5 step 6).
    assert!(
        no_leftover_temp_files(dest.path()),
        "cancellation must delete its own unpublished temp outputs"
    );
}

// ===========================================================================
// 5. Unicode / Bengali path round trip through the full orchestrator
// ===========================================================================

#[tokio::test]
async fn unicode_bengali_input_and_destination_round_trip() {
    let (engine, caps) = resolve_engine().await;
    assert!(
        caps.unicode_paths,
        "the real sidecar's UTF-8 manifest must make the startup probe report true"
    );

    let src_root = tempfile::tempdir().unwrap();
    let bengali_source_dir = src_root.path().join("বাংলা-উৎস-ফোল্ডার");
    std::fs::create_dir_all(&bengali_source_dir).unwrap();
    let input_path = bengali_source_dir.join("দাবা-খেলা.pgn");
    std::fs::copy(fixtures_dir().join("unicode-paths/দাবা-খেলা.pgn"), &input_path).unwrap();
    let original_hash = sha256_file(&input_path);

    let dest_root = tempfile::tempdir().unwrap();
    let bengali_dest_dir = dest_root.path().join("বাংলা-গন্তব্য-ফোল্ডার");
    std::fs::create_dir_all(&bengali_dest_dir).unwrap();

    let jobs_root = tempfile::tempdir().unwrap();
    let eco_file = eco_file_path();
    // A Bengali *base name* too, so the published artifact's own file name
    // is non-ASCII, not just its containing directories.
    let spec = merge_spec(vec![input_path.clone()], bengali_dest_dir.clone(), "ফলাফল");

    let state = AppState::new();
    let ctx = RunJobContext {
        caps: &caps,
        engine: &engine,
        jobs_root: jobs_root.path(),
        eco_file: &eco_file,
    };
    let result = run_job(spec, &ctx, &state, &NullEventSink).await.unwrap();

    assert_eq!(
        result.status,
        JobStatus::Succeeded,
        "Bengali directory+filename round trip must succeed; error was {:?}",
        result.error
    );
    let output_path = bengali_dest_dir.join("ফলাফল.pgn");
    assert!(
        output_path.exists(),
        "Bengali-named output must be published"
    );
    assert_eq!(count_games_in_file(&output_path).unwrap(), 1);

    // The Bengali-named source must be untouched.
    assert_eq!(sha256_file(&input_path), original_hash);
    assert!(no_leftover_temp_files(&bengali_dest_dir));
}
