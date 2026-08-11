// SPDX-License-Identifier: GPL-3.0-or-later
//! Sidecar path resolution, the two-gate integrity check, the startup
//! self-test, and the Unicode-path capability probe (architecture.md
//! §10.1-§10.4; design-02 §1.7, §2.2, Decision D-3; task section A).
//!
//! **Two-gate integrity (binding order):** gate 1 (streamed SHA-256 vs. the
//! pinned identity) must pass *before* gate 2 ever executes the binary -
//! this codebase never runs an unverified executable, even to ask its
//! version. Gate 2 spawns `--version` as an argument array (never a shell)
//! and requires exit 0 **and** stderr - not stdout, see below - trimmed to
//! exactly `pgn-extract v26-06`.
//!
//! **Empirically verified, not assumed** (this task's own instruction):
//! running the real bundled sidecar's `--version` was captured directly for
//! this task with stdout/stderr piped to separate files. Result: exit 0,
//! **stdout empty**, stderr bytes `70 67 6e 2d 65 78 74 72 61 63 74 20 76
//! 32 36 2d 30 36 0d 0a` = `"pgn-extract v26-06\r\n"`. This confirms
//! design-02's own note ("`--version` writes to stderr, not stdout") and
//! pins the exact trailing CRLF this module must trim. `--help` is never
//! parsed for identity (its banner contains a build-date placeholder,
//! DECISIONS-LEDGER.md D-009).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::{EngineCapabilities, EngineIdentity, PublicError};
use crate::errors;

use super::process::run_to_completion;
use super::EngineExecutable;

/// Where to look for the sidecar binary (task section A: "Resolve the
/// bundled sidecar path (Tauri resource dir in production;
/// `src-tauri/binaries/` in dev)").
///
/// On Windows (the only platform verifiable on this development machine -
/// see the crate-level report), Tauri's own `resource_dir()` "resolves to
/// the directory that contains the main executable" (confirmed by reading
/// the vendored `tauri` 2.11.5 source, `src/path/desktop.rs`), and
/// `src-tauri/binaries/README.md` independently states "Tauri installs the
/// sidecar next to the app executable with the target-triple suffix
/// stripped" - so for this build, `Bundled` and "the Tauri resource dir"
/// name the same directory. `Bundled` takes a plain `PathBuf` rather than a
/// `tauri::AppHandle` so this module stays independently testable and so
/// Phase 2's command layer can hand it whatever `app.path().resource_dir()`
/// returns without this module depending on `tauri`'s path API at all.
#[derive(Debug, Clone)]
pub enum SidecarLocation {
    /// Development: the target-triple-suffixed binary in `src-tauri/binaries/`.
    Dev { binaries_dir: PathBuf },
    /// Production/installed: the sidecar next to the app executable, with
    /// the target-triple suffix stripped by Tauri's bundler at package time.
    Bundled { resource_dir: PathBuf },
}

impl SidecarLocation {
    /// Resolves `src-tauri/binaries/` relative to *this crate's own*
    /// manifest directory at compile time (`env!("CARGO_MANIFEST_DIR")`,
    /// the same pattern `engine::capability` already uses for
    /// `include_str!`), so it is correct regardless of the process's
    /// current working directory when running `cargo test`/`cargo tauri
    /// dev` from any location.
    pub fn dev_default() -> Self {
        SidecarLocation::Dev {
            binaries_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries"),
        }
    }
}

/// Re-exported from [`crate::engine::capability`] rather than restated, so
/// the triple this crate ships for is written in exactly one place. That
/// module has to know it anyway (it selects which `build-info-<triple>.json`
/// to embed), and a second copy here could silently disagree with the
/// binary actually on disk.
use crate::engine::capability::TARGET_TRIPLE;

/// `".exe"` on Windows, `""` everywhere else. `std::env::consts` resolves
/// against the *compilation target*, which is exactly the question being
/// asked: `scripts/build-pgn-extract.ps1` installs
/// `pgn-extract-<triple>.exe` while `scripts/build-pgn-extract.sh` installs
/// `pgn-extract-<triple>` with no extension, and Tauri's own
/// `external_binaries` helper applies the same rule when it stages the
/// bundled copy.
const EXE_SUFFIX: &str = std::env::consts::EXE_SUFFIX;

/// The expected sidecar path for a given location - pure path arithmetic,
/// no filesystem access, so it can be asserted against in tests without
/// touching disk.
pub fn expected_sidecar_path(location: &SidecarLocation) -> PathBuf {
    match location {
        SidecarLocation::Dev { binaries_dir } => {
            binaries_dir.join(format!("pgn-extract-{TARGET_TRIPLE}{EXE_SUFFIX}"))
        }
        SidecarLocation::Bundled { resource_dir } => {
            resource_dir.join(format!("pgn-extract{EXE_SUFFIX}"))
        }
    }
}

fn hash_file_sync(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Streams the sidecar's SHA-256 on a blocking-pool thread (the file is
/// ~400 KiB today, but this must not assume "small" - the same pattern
/// would apply to a much larger future binary without changing callers).
///
/// `pub(crate)` (Phase 2a addition): `application::inputs::inspect_inputs`
/// (design-02 §4.1's optional `sha256` on `InputInspectionDto`, gated by
/// `settings.hashInputs`) reuses this exact streamed-hash routine rather
/// than duplicating it, so there is one tested implementation of
/// "streamed SHA-256 of an arbitrary-size file on a blocking-pool thread"
/// in the crate, not two.
pub(crate) async fn hash_file_streaming(path: &Path) -> std::io::Result<String> {
    let owned = path.to_path_buf();
    match tokio::task::spawn_blocking(move || hash_file_sync(&owned)).await {
        Ok(inner) => inner,
        Err(join_err) => Err(std::io::Error::other(join_err)),
    }
}

/// Runs the two-gate integrity check and, only if both pass, returns a
/// verified [`EngineExecutable`]. This is the **only** function in the
/// crate that can produce one outside of tests (design-02 §2.2).
///
/// Gate 1 (hash) always runs before gate 2 (spawn) - a tampered binary is
/// never executed, not even for `--version`.
pub async fn resolve_and_verify(
    location: &SidecarLocation,
    pinned: &EngineIdentity,
) -> Result<EngineExecutable, PublicError> {
    let path = expected_sidecar_path(location);

    // Gate 1: streamed SHA-256 vs. pinned identity.
    let actual_sha256 = match hash_file_streaming(&path).await {
        Ok(hash) => hash,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(errors::engine_missing(&path));
        }
        Err(e) => return Err(errors::engine_start_failed_io(&e)),
    };
    if !actual_sha256.eq_ignore_ascii_case(&pinned.sha256) {
        return Err(errors::engine_tampered(&pinned.sha256, &actual_sha256));
    }

    // Gate 2: spawn `--version` (argument array, no shell) and require
    // exit 0 with stderr trimmed to exactly "pgn-extract <version>".
    let candidate = EngineExecutable::new_verified(path.clone());
    let probe_cwd = path.parent().unwrap_or_else(|| Path::new("."));
    let args = vec![OsString::from("--version")];
    let run = run_to_completion(&candidate, &args, probe_cwd)
        .await
        .map_err(|e| errors::engine_start_failed_io(&e))?;

    let expected_stderr = format!("pgn-extract {}", pinned.version);
    let stderr_trimmed = run.stderr.trim();
    let stdout_trimmed = run.stdout.trim();
    if run.exit_code != Some(0) || stderr_trimmed != expected_stderr || !stdout_trimmed.is_empty() {
        return Err(errors::engine_start_failed_bad_probe(&format!(
            "--version probe: expected exit 0 and stderr {expected_stderr:?} with empty \
             stdout, got exit {:?}, stdout {stdout_trimmed:?}, stderr {stderr_trimmed:?}",
            run.exit_code
        )));
    }

    Ok(candidate)
}

/// Embedded ASCII fixture for the capability self-test (design-02 §1.7 item
/// (c): "a micro self-test merge of an embedded 2-game fixture in a
/// scratch workspace"). Embedded via `include_str!` at *compile* time, so
/// no filesystem access to the `fixtures/` dev-tree is needed at *run*
/// time - an installed application does not carry that directory at all.
/// One game rather than design-02's illustrative two: what matters is that
/// a real merge through the real engine produces the expected count, not
/// the specific number; inventing a second embedded game not backed by a
/// real reviewed fixture would violate the project's never-invent rule.
const SELF_TEST_FIXTURE: &str = include_str!("../../../fixtures/valid/single-game.pgn");
const SELF_TEST_EXPECTED_GAMES: u64 = 1;

/// Embedded Bengali fixture for the Unicode-path probe (design-02 §1.7 item
/// (d), Decision D-3). Only the *content* is reused from the committed
/// fixture; [`probe_unicode_paths`] writes it under a freshly generated
/// non-ASCII scratch path each run (not this file's own on-disk location),
/// both because `fixtures/` will not exist in an installed build and
/// because a fresh name each run genuinely exercises "can this machine,
/// right now, create and have the engine address a Unicode path."
const UNICODE_PROBE_FIXTURE: &str = include_str!("../../../fixtures/unicode-paths/দাবা-খেলা.pgn");

fn attached_output_flag(path: &Path) -> OsString {
    let mut flag = OsString::from("-o");
    flag.push(path);
    flag
}

/// Runs the capability self-test (design-02 §1.7 item (c)) against a
/// verified engine: merges the embedded fixture in a fresh scratch
/// workspace and checks the real produced game count.
pub async fn run_self_test(engine: &EngineExecutable) -> Result<(), PublicError> {
    let scratch =
        std::env::temp_dir().join(format!("pgnstudio-selftest-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&scratch).map_err(|e| errors::engine_start_failed_io(&e))?;
    let result = run_self_test_in(engine, &scratch).await;
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

async fn run_self_test_in(engine: &EngineExecutable, scratch: &Path) -> Result<(), PublicError> {
    let input_path = scratch.join("self-test-input.pgn");
    std::fs::write(&input_path, SELF_TEST_FIXTURE)
        .map_err(|e| errors::engine_start_failed_io(&e))?;
    let output_path = scratch.join("self-test-output.pgn");
    let args = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        attached_output_flag(&output_path),
        input_path.into_os_string(),
    ];
    let run = run_to_completion(engine, &args, scratch)
        .await
        .map_err(|e| errors::engine_start_failed_io(&e))?;
    if run.exit_code != Some(0) {
        return Err(errors::engine_start_failed_bad_probe(&format!(
            "self-test merge exited with {:?}",
            run.exit_code
        )));
    }
    let count = crate::filesystem::count_games_in_file(&output_path)
        .map_err(|e| errors::engine_start_failed_io(&e))?;
    if count != SELF_TEST_EXPECTED_GAMES {
        return Err(errors::engine_start_failed_bad_probe(&format!(
            "self-test expected {SELF_TEST_EXPECTED_GAMES} game(s), engine produced {count}"
        )));
    }
    Ok(())
}

/// Runs the engine against a freshly generated non-ASCII (Bengali)
/// directory name *and* file name and reports whether the round trip
/// succeeded, so [`crate::domain::EngineCapabilities::unicode_paths`] is set
/// from a real, just-observed result (task section A: "Phase 1a correctly
/// left it `false` in the static map because it is a runtime fact...
/// set it from the probe, don't hardcode"). Never panics or propagates an
/// error - an inability to even set up the probe (e.g. an unwritable temp
/// directory) is itself evidence the capability should be reported `false`.
pub async fn probe_unicode_paths(engine: &EngineExecutable) -> bool {
    let base = std::env::temp_dir().join(format!(
        "pgnstudio-unicodeprobe-{}",
        Uuid::new_v4().simple()
    ));
    let result = probe_unicode_paths_in(engine, &base).await;
    let _ = std::fs::remove_dir_all(&base);
    result.unwrap_or(false)
}

async fn probe_unicode_paths_in(engine: &EngineExecutable, base: &Path) -> std::io::Result<bool> {
    // Bengali directory name AND file name (D-009's verification note:
    // "Bengali filenames AND Bengali directory names work").
    let probe_dir = base.join("ফাইল-পরীক্ষা");
    std::fs::create_dir_all(&probe_dir)?;
    let input_path = probe_dir.join("দাবা-খেলা.pgn");
    std::fs::write(&input_path, UNICODE_PROBE_FIXTURE)?;
    let output_path = probe_dir.join("ফলাফল.pgn");
    let args = vec![
        OsString::from("-s"),
        OsString::from("--summary"),
        attached_output_flag(&output_path),
        input_path.into_os_string(),
    ];
    let run = run_to_completion(engine, &args, &probe_dir).await?;
    if run.exit_code != Some(0) {
        return Ok(false);
    }
    let produced_nonempty_output = output_path.metadata().map(|m| m.len() > 0).unwrap_or(false);
    Ok(produced_nonempty_output)
}

/// Everything a Phase 2 `get_engine_info`/`get_engine_capabilities` command
/// needs: a verified engine plus capabilities with `unicode_paths` set from
/// the real probe result, not the conservative static default.
#[derive(Debug)]
pub struct StartupCheckResult {
    pub engine: EngineExecutable,
    pub capabilities: EngineCapabilities,
}

/// Runs the complete startup sequence (task section A): resolve, two-gate
/// verify, self-test, Unicode probe. The single entry point Phase 2's
/// application-wiring layer should call once at launch.
pub async fn startup_check(location: &SidecarLocation) -> Result<StartupCheckResult, PublicError> {
    let mut capabilities = super::capability::pinned_v26_06();
    let engine = resolve_and_verify(location, &capabilities.identity).await?;
    run_self_test(&engine).await?;
    capabilities.unicode_paths = probe_unicode_paths(&engine).await;
    Ok(StartupCheckResult {
        engine,
        capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ErrorCode;

    fn real_pinned_identity() -> EngineIdentity {
        super::super::capability::pinned_v26_06().identity
    }

    #[test]
    fn expected_sidecar_path_dev_matches_the_real_binaries_directory() {
        let location = SidecarLocation::dev_default();
        let path = expected_sidecar_path(&location);
        // Built from Path components, not a literal with an embedded
        // separator: `Path::ends_with` matches whole components, so a
        // hardcoded "binaries\\..." can never match on a platform whose
        // separator is `/`.
        assert!(path.ends_with(
            Path::new("binaries").join(format!("pgn-extract-{TARGET_TRIPLE}{EXE_SUFFIX}"))
        ));
        assert!(
            path.is_file(),
            "dev_default() must resolve to the real committed sidecar at {path:?}"
        );
    }

    #[test]
    fn expected_sidecar_path_bundled_strips_the_target_triple() {
        // An absolute path with a space in it, spelled per-platform: the
        // point of the fixture is "a realistic install directory", and on
        // macOS `C:\...` is a *relative* single-component path, which would
        // quietly stop testing what this asserts.
        #[cfg(windows)]
        let resource_dir = PathBuf::from(r"C:\Program Files\PGN Studio");
        #[cfg(not(windows))]
        let resource_dir = PathBuf::from("/Applications/PGN Studio.app/Contents/Resources");

        let location = SidecarLocation::Bundled {
            resource_dir: resource_dir.clone(),
        };
        let path = expected_sidecar_path(&location);
        assert_eq!(path, resource_dir.join(format!("pgn-extract{EXE_SUFFIX}")));
    }

    #[tokio::test]
    async fn resolve_and_verify_succeeds_against_the_real_sidecar() {
        let location = SidecarLocation::dev_default();
        let pinned = real_pinned_identity();
        let engine = resolve_and_verify(&location, &pinned)
            .await
            .expect("the real, checksum-pinned sidecar must pass both gates");
        assert_eq!(engine.path(), expected_sidecar_path(&location));
    }

    #[tokio::test]
    async fn resolve_and_verify_reports_engine_missing_for_a_nonexistent_path() {
        let location = SidecarLocation::Dev {
            binaries_dir: PathBuf::from(r"C:\this\path\does\not\exist\at\all"),
        };
        let pinned = real_pinned_identity();
        let err = resolve_and_verify(&location, &pinned).await.unwrap_err();
        assert_eq!(err.code(), ErrorCode::EngineMissing);
    }

    #[tokio::test]
    async fn resolve_and_verify_reports_engine_tampered_for_a_modified_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let real_path = expected_sidecar_path(&SidecarLocation::dev_default());
        let mut bytes = std::fs::read(&real_path).unwrap();
        // Flip one byte, deep enough in the file to be past any header a
        // naive check might special-case, but well within the binary.
        let flip_at = bytes.len() / 2;
        bytes[flip_at] ^= 0xFF;
        // Must match what expected_sidecar_path() will look for in this
        // temp Dev location, or the test asserts ENGINE_MISSING instead of
        // the ENGINE_TAMPERED it is actually about.
        let tampered_path = tmp
            .path()
            .join(format!("pgn-extract-{TARGET_TRIPLE}{EXE_SUFFIX}"));
        std::fs::write(&tampered_path, &bytes).unwrap();

        let location = SidecarLocation::Dev {
            binaries_dir: tmp.path().to_path_buf(),
        };
        let pinned = real_pinned_identity();
        let err = resolve_and_verify(&location, &pinned).await.unwrap_err();
        assert_eq!(err.code(), ErrorCode::EngineTampered);
    }

    #[tokio::test]
    async fn run_self_test_passes_against_the_real_sidecar() {
        let location = SidecarLocation::dev_default();
        let pinned = real_pinned_identity();
        let engine = resolve_and_verify(&location, &pinned).await.unwrap();
        run_self_test(&engine)
            .await
            .expect("the real sidecar must pass the embedded-fixture self-test");
    }

    #[tokio::test]
    async fn probe_unicode_paths_is_true_for_the_real_utf8_manifest_sidecar() {
        let location = SidecarLocation::dev_default();
        let pinned = real_pinned_identity();
        let engine = resolve_and_verify(&location, &pinned).await.unwrap();
        assert!(
            probe_unicode_paths(&engine).await,
            "the real sidecar embeds a UTF-8 activeCodePage manifest (D-009) and must pass \
             the Bengali directory+filename round trip"
        );
    }

    #[tokio::test]
    async fn startup_check_end_to_end_against_the_real_sidecar() {
        let location = SidecarLocation::dev_default();
        let result = startup_check(&location)
            .await
            .expect("full startup sequence must succeed against the real, pinned sidecar");
        assert!(result.capabilities.unicode_paths);
        assert_eq!(result.capabilities.identity.version, "v26-06");
    }
}
