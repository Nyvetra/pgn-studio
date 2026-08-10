// SPDX-License-Identifier: GPL-3.0-or-later
//! Phase 0 smoke test.
//!
//! This only proves the Rust integration-test harness itself works
//! (`cargo test` can build and run a test in `src-tauri/tests/` against the
//! compiled `pgn_studio_lib` crate). It is not real coverage - Phase 1+
//! should add proper unit/integration tests for the domain, engine adapter,
//! and job orchestration as they are implemented.

#[test]
fn app_info_exposes_a_non_empty_semantic_version() {
    let info = pgn_studio_lib::build_app_info();

    assert_eq!(info.name, "pgn-studio");
    assert_eq!(
        info.version.split('.').count(),
        3,
        "expected a MAJOR.MINOR.PATCH version, got {:?}",
        info.version
    );
}
