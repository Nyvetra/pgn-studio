// SPDX-License-Identifier: GPL-3.0-or-later
//! `validate_job` / `compile_job_preview` / `start_job` / `cancel_job` /
//! `get_job` / `list_recent_jobs` / `delete_job_history` /
//! `export_job_manifest` (design-02 §4.1; architecture.md §13.7).
//!
//! Every handler here is a thin wrapper: deserialize (Tauri already did
//! that), delegate to `application::jobs`, return. `start_job`
//! additionally re-validates and re-compiles internally via `jobs::run_job`,
//! since a `Ready` state from the UI is a convenience gate, never a trusted
//! precondition (design-02 §2.1).

use std::path::PathBuf;

use uuid::Uuid;

use tauri::{AppHandle, State};

use crate::application::jobs::{
    self, CommandPreviewDto, JobAcceptedDto, JobRecordDto, ValidationReportDto,
};
use crate::application::AppContext;
use crate::domain::{JobSpec, PublicError};
use crate::persistence::history::JobSummaryDto;

#[tauri::command]
#[specta::specta]
pub async fn validate_job(
    state: State<'_, AppContext>,
    spec: JobSpec,
) -> Result<ValidationReportDto, PublicError> {
    jobs::validate_job(&state, spec).await
}

#[tauri::command]
#[specta::specta]
pub async fn compile_job_preview(
    state: State<'_, AppContext>,
    spec: JobSpec,
) -> Result<CommandPreviewDto, PublicError> {
    jobs::compile_job_preview(&state, spec).await
}

#[tauri::command]
#[specta::specta]
pub async fn start_job(app: AppHandle, spec: JobSpec) -> Result<JobAcceptedDto, PublicError> {
    jobs::start_job(app, spec).await
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_job(state: State<'_, AppContext>, job_id: Uuid) -> Result<(), PublicError> {
    jobs::cancel_job(&state, job_id)
}

#[tauri::command]
#[specta::specta]
pub async fn get_job(
    state: State<'_, AppContext>,
    job_id: Uuid,
) -> Result<JobRecordDto, PublicError> {
    jobs::get_job(&state, job_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_recent_jobs(
    state: State<'_, AppContext>,
    limit: u32,
) -> Result<Vec<JobSummaryDto>, PublicError> {
    Ok(jobs::list_recent_jobs(&state, limit))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_job_history(
    state: State<'_, AppContext>,
    job_id: Uuid,
) -> Result<(), PublicError> {
    jobs::delete_job_history(&state, job_id)
}

/// "Save Job" (architecture.md §13.7): exports a completed job's
/// reproducible manifest to a user-chosen file via the native save dialog.
/// `Ok(None)` means the user cancelled the dialog - not an error.
#[tauri::command]
#[specta::specta]
pub async fn export_job_manifest(
    app: AppHandle,
    state: State<'_, AppContext>,
    job_id: Uuid,
) -> Result<Option<PathBuf>, PublicError> {
    jobs::export_job_manifest(&app, &state, job_id).await
}
