// SPDX-License-Identifier: GPL-3.0-or-later
//! `select_input_files` / `select_input_directory` / `select_output_directory`
//! / `reveal_path` / `open_path` (design-02 §4.1).
//!
//! The dialog/opener plugins are called only from these Rust handlers -
//! never directly by the frontend (architecture.md §16.2: "the frontend is
//! outside the trust boundary"). `blocking_pick_*` is the dialog plugin's
//! own documented pattern for `async fn` Tauri commands (its doc comment's
//! own example is literally `async fn my_command(app: tauri::AppHandle) {
//! app.dialog().file().blocking_pick_file() }` - it blocks a tokio
//! worker thread, never the main/UI thread, which is exactly what an async
//! command handler runs on).

use std::path::PathBuf;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::application::{paths, AppContext};
use crate::domain::PublicError;
use crate::errors;

fn file_path_to_string(fp: tauri_plugin_dialog::FilePath) -> Option<String> {
    fp.simplified()
        .into_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
#[specta::specta]
pub async fn select_input_files(app: AppHandle) -> Result<Vec<String>, PublicError> {
    let picked = app
        .dialog()
        .file()
        .add_filter("PGN files", &["pgn"])
        .blocking_pick_files();
    Ok(picked
        .into_iter()
        .flatten()
        .filter_map(file_path_to_string)
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn select_input_directory(app: AppHandle) -> Result<Option<String>, PublicError> {
    Ok(app
        .dialog()
        .file()
        .blocking_pick_folder()
        .and_then(file_path_to_string))
}

#[tauri::command]
#[specta::specta]
pub async fn select_output_directory(app: AppHandle) -> Result<Option<String>, PublicError> {
    Ok(app
        .dialog()
        .file()
        .blocking_pick_folder()
        .and_then(file_path_to_string))
}

/// Maps an opener-plugin I/O failure onto the closed taxonomy. Neither the
/// "reveal in file manager" nor "open with default app" action has a
/// natural home in design-02 §5.1's table (that table is scoped to job
/// lifecycle errors); `UNKNOWN_INTERNAL_ERROR` is the pre-agreed escape
/// hatch for exactly this shape of gap (see the identical reasoning in
/// `persistence::settings::JsonSettingsStore::update`).
fn opener_failure(
    action: &str,
    path: &std::path::Path,
    source: impl std::fmt::Display,
) -> PublicError {
    #[allow(deprecated)]
    errors::unknown_internal_error(anyhow::anyhow!(
        "{action} failed for {}: {source}",
        path.display()
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn reveal_path(
    app: AppHandle,
    state: State<'_, AppContext>,
    path: String,
) -> Result<(), PublicError> {
    let allowed = paths::resolve_allowed_path(&state, &PathBuf::from(&path))?;
    app.opener()
        .reveal_item_in_dir(&allowed)
        .map_err(|e| opener_failure("revealing the path", &allowed, e))
}

#[tauri::command]
#[specta::specta]
pub async fn open_path(
    app: AppHandle,
    state: State<'_, AppContext>,
    path: String,
) -> Result<(), PublicError> {
    let allowed = paths::resolve_allowed_path(&state, &PathBuf::from(&path))?;
    app.opener()
        .open_path(allowed.to_string_lossy(), None::<&str>)
        .map_err(|e| opener_failure("opening the path", &allowed, e))
}
