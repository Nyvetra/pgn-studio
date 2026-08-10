// SPDX-License-Identifier: GPL-3.0-or-later
//! `inspect_inputs` (design-02 §4.1).

use tauri::State;

use crate::application::inputs::{inspect_inputs as inspect_inputs_impl, InputInspectionDto};
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
