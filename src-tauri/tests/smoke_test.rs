// SPDX-License-Identifier: GPL-3.0-or-later
//! Smoke test proving the Rust integration-test harness itself works
//! (`cargo test` can build and run a test in `src-tauri/tests/` against the
//! compiled `pgn_studio_lib` crate). Originally a Phase 0 placeholder over
//! the old ad hoc `AppInfo`/`build_app_info`; updated for Phase 2a to check
//! the real `get_app_info` IPC command's DTO
//! (`commands::dto::AppInfoDto`/`build_app_info`, design-02 §4.1:
//! `{ appVersion, os, arch }`) instead of a scaffold shape that no longer
//! exists.

#[test]
fn app_info_exposes_a_non_empty_semantic_version_and_platform() {
    let info = pgn_studio_lib::commands::dto::build_app_info();

    assert_eq!(
        info.app_version.split('.').count(),
        3,
        "expected a MAJOR.MINOR.PATCH version, got {:?}",
        info.app_version
    );
    assert!(!info.os.is_empty());
    assert!(!info.arch.is_empty());
}
