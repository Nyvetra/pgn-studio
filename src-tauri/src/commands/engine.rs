// SPDX-License-Identifier: GPL-3.0-or-later
//! `get_engine_info` / `get_engine_capabilities` (design-02 §4.1).
//!
//! Both simply surface `AppContext::engine`, computed once at startup by
//! `application::startup::initialize` (`engine::sidecar::startup_check`'s
//! two-gate verify + self-test + Unicode-path probe) - re-probing on every
//! call would be pointless (identity/capabilities do not change while the
//! app is running) and would re-run the self-test merge unnecessarily.

use tauri::State;

use crate::application::AppContext;
use crate::domain::{EngineCapabilities, EngineIdentity, PublicError};

#[tauri::command]
#[specta::specta]
pub async fn get_engine_info(state: State<'_, AppContext>) -> Result<EngineIdentity, PublicError> {
    state
        .engine_bundle()
        .map(|bundle| bundle.capabilities.identity.clone())
}

#[tauri::command]
#[specta::specta]
pub async fn get_engine_capabilities(
    state: State<'_, AppContext>,
) -> Result<EngineCapabilities, PublicError> {
    state
        .engine_bundle()
        .map(|bundle| bundle.capabilities.clone())
}
