// SPDX-License-Identifier: GPL-3.0-or-later
//! DTOs whose only reasonable home is `commands/` itself (design-02 §4.1).
//! Everything else that crosses the IPC boundary already has a home closer
//! to the logic that produces it (`application::jobs::{ValidationReportDto,
//! CommandPreviewDto, JobAcceptedDto, JobRecordDto}`,
//! `application::inputs::InputInspectionDto`,
//! `application::events::JobEvent`, `persistence::history::JobSummaryDto`,
//! `persistence::settings::{SettingsDto, SettingsPatchDto}`) or reuses a
//! `domain::`/`filesystem::manifest::` type directly - see this crate's
//! Phase 2a report for why that mirrors the project's established "no
//! separate wire vs domain type" convention (`domain::job_spec`'s own doc
//! comment) rather than a second `Dto`-suffixed mirror of every type.

use serde::Serialize;
use specta::Type;

/// `get_app_info` response (design-02 §4.1: `{ appVersion, os, arch }`).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub app_version: String,
    pub os: String,
    pub arch: String,
}

/// Pure, testable constructor - mirrors `build_app_info`'s existing Phase 0
/// precedent (kept free of any `tauri::AppHandle`/`State` dependency).
pub fn build_app_info() -> AppInfoDto {
    AppInfoDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_app_info_reports_a_non_empty_semantic_version_and_platform() {
        let info = build_app_info();
        assert_eq!(info.app_version, env!("CARGO_PKG_VERSION"));
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
    }
}
