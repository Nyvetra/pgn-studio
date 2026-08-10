// SPDX-License-Identifier: GPL-3.0-or-later
//! `clear_logs` (architecture.md §22.1: "Provide 'Clear Logs.'"). A thin
//! wrapper over `observability::clear_logs`, matching `commands::settings`'s
//! own precedent of calling straight into a small, already-self-contained
//! module rather than adding an intermediate `application::logs` layer for
//! a single operation.

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::application::{run_blocking, AppContext};
use crate::domain::PublicError;

/// `clear_logs` response: counts only, never file paths - a cleared log
/// directory has nothing left worth naming, and this keeps the shape
/// trivially stable regardless of how many files existed.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClearLogsResultDto {
    pub deleted_count: u32,
    pub failed_count: u32,
}

#[tauri::command]
#[specta::specta]
pub async fn clear_logs(state: State<'_, AppContext>) -> Result<ClearLogsResultDto, PublicError> {
    let log_dir = state.log_dir.clone();
    let report = run_blocking(move || crate::observability::clear_logs(&log_dir)).await?;
    Ok(ClearLogsResultDto {
        deleted_count: report.deleted.len() as u32,
        failed_count: report.deletion_failures.len() as u32,
    })
}
