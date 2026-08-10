// SPDX-License-Identifier: GPL-3.0-or-later
//! `get_settings` / `update_settings` (design-02 §4.1; architecture.md
//! §15.1). Thin wrappers over `persistence::settings::SettingsStore` - the
//! store itself (not this module) owns patch application and the
//! `schemaVersion` migration hook.

use tauri::State;

use crate::application::AppContext;
use crate::domain::PublicError;
use crate::persistence::settings::{SettingsDto, SettingsPatchDto};

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, AppContext>) -> Result<SettingsDto, PublicError> {
    Ok(state.settings.load())
}

#[tauri::command]
#[specta::specta]
pub async fn update_settings(
    state: State<'_, AppContext>,
    patch: SettingsPatchDto,
) -> Result<SettingsDto, PublicError> {
    state.settings.update(patch)
}
