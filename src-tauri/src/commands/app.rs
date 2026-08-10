// SPDX-License-Identifier: GPL-3.0-or-later
//! `get_app_info` (design-02 §4.1).

use super::dto::{build_app_info, AppInfoDto};

#[tauri::command]
#[specta::specta]
pub async fn get_app_info() -> AppInfoDto {
    build_app_info()
}
