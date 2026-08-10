// SPDX-License-Identifier: GPL-3.0-or-later
// `PublicError` (errors/, domain::result) is deliberately a rich,
// redaction-safe struct (code + title + message + remediation + log_path +
// technical_id) per architecture.md §18.2/design-02 §5.2 - that richness is
// the point (it is what lets a `technical_id` join a safe user-facing
// message to full internal detail in the local log), not an oversight to
// fix by boxing. Every `Result<_, PublicError>` in this crate is a cold
// error path (job setup/validation/cancellation), never a hot loop, so the
// stack-size micro-optimization `clippy::result_large_err` exists for does
// not apply here; boxing it at every one of the many call sites across
// `errors/`, `jobs/`, and `filesystem/` would add indirection without a
// measurable benefit. Same reasoning covers `filesystem::publish::
// PublishFailure`, which is intentionally rich for the same reason
// (architecture.md §18.3's "never claim deletion that did not happen").
#![allow(clippy::result_large_err)]
//! PGN Studio Tauri application library.
//!
//! Phase 1a implemented the domain model ([`domain`]) and the pure
//! `pgn-extract` command compiler ([`engine::command_compiler`]). Phase 1b
//! adds job orchestration ([`jobs`]), filesystem safety ([`filesystem`]),
//! the public error taxonomy ([`errors`]), and engine sidecar
//! resolution/self-test ([`engine::sidecar`]). Persistence, reporting, and
//! the Tauri IPC command surface are still not implemented - see the
//! `README.md` in each remaining module directory (`commands/`,
//! `application/`, `persistence/`, `reporting/`) for what belongs there in
//! Phase 2, per `PGN-Studio-architecture.md` §24.

pub mod domain;
pub mod engine;
pub mod errors;
pub mod filesystem;
pub mod jobs;

use serde::Serialize;

/// Response shape for the `get_app_info` command.
///
/// Field names are serialized as camelCase to match the hand-written mirror
/// type in `src/ipc/generated-types.ts`. A real Rust-to-TypeScript type
/// generator should replace this manual pairing once the Phase 1 `JobSpec`
/// DTOs (architecture.md §9) make hand-maintaining types impractical.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: String,
}

/// Pure, testable constructor for [`AppInfo`].
///
/// Kept free of any `tauri::AppHandle`/`State` dependency so it can be unit
/// tested without a running Tauri application context.
pub fn build_app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tauri_version: tauri::VERSION.to_string(),
    }
}

/// The only command exposed to the frontend in Phase 0.
///
/// Proves the IPC boundary end to end: `src/ipc/client.ts` calls this via
/// `invoke("get_app_info")` and the diagnostic screen renders the result.
#[tauri::command]
fn get_app_info() -> AppInfo {
    build_app_info()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_app_info_reports_the_cargo_package_identity() {
        let info = build_app_info();
        assert_eq!(info.name, "pgn-studio");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert!(
            !info.tauri_version.is_empty(),
            "tauri_version should never be empty"
        );
    }
}
