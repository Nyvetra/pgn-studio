// SPDX-License-Identifier: GPL-3.0-or-later
//! `inspect_inputs` / `scan_input_directory` (design-02 §4.1;
//! architecture.md §13.2).

use tauri::State;

use crate::application::inputs::{
    inspect_inputs as inspect_inputs_impl, scan_input_directory as scan_input_directory_impl,
    DirectoryScanDto, InputInspectionDto, ScanInputDirectoryOptions,
};
use crate::application::AppContext;
use crate::domain::PublicError;

#[tauri::command]
#[specta::specta]
pub async fn inspect_inputs(
    state: State<'_, AppContext>,
    paths: Vec<String>,
) -> Result<Vec<InputInspectionDto>, PublicError> {
    let hash_inputs = state.settings.load().hash_inputs;
    Ok(inspect_inputs_impl(paths, hash_inputs).await)
}

/// "Add Folder" (architecture.md §13.2): scans `directory` for candidate
/// `.pgn` inputs and returns them already inspected, for review before
/// they are added to the source list.
#[tauri::command]
#[specta::specta]
pub async fn scan_input_directory(
    state: State<'_, AppContext>,
    directory: String,
    options: ScanInputDirectoryOptions,
) -> Result<DirectoryScanDto, PublicError> {
    let hash_inputs = state.settings.load().hash_inputs;
    scan_input_directory_impl(directory, options, hash_inputs).await
}
