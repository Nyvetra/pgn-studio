// SPDX-License-Identifier: GPL-3.0-or-later
//! Application settings (architecture.md §15.1; design-02 §4.1
//! `get_settings`/`update_settings`).
//!
//! Versioned JSON in the platform app-config directory. `SCHEMA_VERSION` is
//! the single source of truth for "what version does this build write and
//! expect"; [`migrate`] is the reserved hook for translating an older
//! on-disk document forward - V1 has no prior version to migrate *from*, so
//! it only has to recognize its own version and fall back to defaults
//! otherwise, but the seam is real: a V2 build would extend `migrate`'s
//! match, not replace this module's public shape.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::{ConflictPolicy, PublicError};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

/// V1 ships exactly one policy (architecture.md §15.1's example config has
/// `"updateChecks": "off"`, and V1 implements no update-check network call
/// at all - architecture.md §16.2 "Do not transmit data or logs in Version
/// 1"). A real closed enum rather than a bare string, matching this
/// project's everywhere-typed convention, with room to add `Manual`/
/// `Automatic` variants in a later version without a breaking wire-shape
/// change (only a widened enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateCheckPolicy {
    Off,
}

/// `get_settings`/`update_settings` response shape (architecture.md §15.1).
///
/// `hash_inputs` (Phase 2a addition, not in architecture.md §15.1's
/// illustrative example config): design-02 §4.1 gates
/// `inspect_inputs`'s optional per-file `sha256` on "`settings.hashInputs`"
/// as though it already existed, but no such field appears anywhere in
/// architecture.md §15.1's settings shape. Rather than silently repurposing
/// an unrelated existing flag (e.g. `rememberRecentFiles`, which is about a
/// recent-files *list*, not hashing) or dropping the feature, this adds the
/// field design-02 already assumes - architecture.md §15.3 independently
/// supports it being optional ("Input hashing can be optional for very
/// large files because it requires a full additional read"). Flagged in
/// this crate's Phase 2a report for the coordinator to confirm or correct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsDto {
    pub schema_version: u32,
    pub theme: Theme,
    pub default_output_directory: Option<PathBuf>,
    pub default_conflict_policy: ConflictPolicy,
    pub remember_recent_files: bool,
    pub max_recent_jobs: u32,
    pub show_advanced_command: bool,
    pub update_checks: UpdateCheckPolicy,
    pub hash_inputs: bool,
}

impl Default for SettingsDto {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            theme: Theme::System,
            default_output_directory: None,
            default_conflict_policy: ConflictPolicy::AddNumericSuffix,
            remember_recent_files: true,
            max_recent_jobs: 50,
            show_advanced_command: false,
            update_checks: UpdateCheckPolicy::Off,
            // Off by default: a full extra read of every input is not
            // something V1 should do silently/by surprise (architecture.md
            // §15.3's own framing - "can be optional for very large files").
            hash_inputs: false,
        }
    }
}

/// `update_settings(patch)` request shape. Every field is optional: absent
/// = leave unchanged. `default_output_directory` is doubly-optional on
/// purpose (`Option<Option<PathBuf>>`) so a patch can distinguish "don't
/// touch this field" (absent) from "explicitly clear it back to null"
/// (`null`) from "set it" (a string) - the one nullable field in
/// [`SettingsDto`] needs the tri-state; every other field is non-nullable so
/// a plain `Option<T>` (absent-vs-present) already says everything needed.
/// Note for the generated TypeScript: `specta` cannot express the inner/
/// outer distinction any more precisely than TS's own type system can, so
/// the exported type reads as roughly `T | null | undefined` - the actual
/// three-way behavior is documented here and exercised by this module's
/// tests, not visible in the type alone.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatchDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_output_directory: Option<Option<PathBuf>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_conflict_policy: Option<ConflictPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remember_recent_files: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_recent_jobs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_advanced_command: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_checks: Option<UpdateCheckPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash_inputs: Option<bool>,
}

fn apply_patch(base: &mut SettingsDto, patch: SettingsPatchDto) {
    if let Some(theme) = patch.theme {
        base.theme = theme;
    }
    if let Some(dir) = patch.default_output_directory {
        base.default_output_directory = dir;
    }
    if let Some(policy) = patch.default_conflict_policy {
        base.default_conflict_policy = policy;
    }
    if let Some(v) = patch.remember_recent_files {
        base.remember_recent_files = v;
    }
    if let Some(v) = patch.max_recent_jobs {
        base.max_recent_jobs = v;
    }
    if let Some(v) = patch.show_advanced_command {
        base.show_advanced_command = v;
    }
    if let Some(v) = patch.update_checks {
        base.update_checks = v;
    }
    if let Some(v) = patch.hash_inputs {
        base.hash_inputs = v;
    }
}

/// Reads `path` and returns a valid [`SettingsDto`], migrating or falling
/// back to [`SettingsDto::default`] rather than ever failing (a corrupt or
/// unreadable settings file must never block the app from starting -
/// nothing in architecture.md §15.1 requires settings to be recoverable,
/// only that a sane default exists).
fn read_or_default(path: &Path) -> SettingsDto {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return SettingsDto::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return SettingsDto::default();
    };
    migrate(value).unwrap_or_default()
}

/// The migration hook (task ask: "versioned JSON... with schemaVersion and
/// a migration hook"). V1 has exactly one recognized `schemaVersion` (`1`)
/// and migrates nothing else - a future V2 extends the match arms here,
/// translating older documents forward, rather than changing
/// [`SettingsDto`]'s own shape or this function's signature.
fn migrate(value: serde_json::Value) -> Option<SettingsDto> {
    match value.get("schemaVersion").and_then(|v| v.as_u64()) {
        Some(1) => serde_json::from_value(value).ok(),
        _ => None,
    }
}

/// Storage seam (task ask: "Keep the storage layer small and behind a trait
/// so Phase 6 can swap it"). Synchronous by design, matching
/// `filesystem::validate::validate_job`'s own precedent: callers on the
/// async runtime run these through `tokio::task::spawn_blocking`.
pub trait SettingsStore: Send + Sync {
    fn load(&self) -> SettingsDto;
    fn update(&self, patch: SettingsPatchDto) -> Result<SettingsDto, PublicError>;
}

/// Bounded single-file JSON settings store.
pub struct JsonSettingsStore {
    path: PathBuf,
    cache: RwLock<SettingsDto>,
}

impl JsonSettingsStore {
    /// Reads `path` (or falls back to defaults) once, caching the result so
    /// repeated `get_settings` calls are pure memory reads.
    pub fn load_or_default(path: PathBuf) -> Self {
        let initial = read_or_default(&path);
        Self {
            path,
            cache: RwLock::new(initial),
        }
    }
}

impl SettingsStore for JsonSettingsStore {
    fn load(&self) -> SettingsDto {
        self.cache.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn update(&self, patch: SettingsPatchDto) -> Result<SettingsDto, PublicError> {
        let mut guard = self.cache.write().unwrap_or_else(|p| p.into_inner());
        let mut next = guard.clone();
        apply_patch(&mut next, patch);
        super::write_json_atomic(&self.path, &next).map_err(|e| {
            // No `ErrorCode` in the closed §18.1 taxonomy names "settings
            // persistence failure" specifically (design-02 §5.1's table is
            // scoped to job-lifecycle errors); `HISTORY_WRITE_FAILED`'s own
            // wording is manifest-specific and would be misleading here.
            // `UNKNOWN_INTERNAL_ERROR` is the pre-agreed escape hatch for
            // exactly this shape of gap (errors::unknown_internal_error's
            // own doc comment: "it exists for Phase 2's command boundary and
            // any future truly-unanticipated failure").
            #[allow(deprecated)]
            crate::errors::unknown_internal_error(anyhow::anyhow!(
                "writing settings to {}: {e}",
                self.path.display()
            ))
        })?;
        *guard = next.clone();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_round_trip_through_migrate() {
        let value = serde_json::to_value(SettingsDto::default()).unwrap();
        assert_eq!(migrate(value), Some(SettingsDto::default()));
    }

    #[test]
    fn unrecognized_schema_version_falls_back_to_none() {
        let value = serde_json::json!({"schemaVersion": 999});
        assert_eq!(migrate(value), None);
    }

    #[test]
    fn read_or_default_survives_a_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("does-not-exist.json");
        assert_eq!(read_or_default(&path), SettingsDto::default());
    }

    #[test]
    fn read_or_default_survives_corrupt_json() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(read_or_default(&path), SettingsDto::default());
    }

    #[test]
    fn store_load_reflects_an_existing_valid_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        let custom = SettingsDto {
            max_recent_jobs: 7,
            ..SettingsDto::default()
        };
        std::fs::write(&path, serde_json::to_vec(&custom).unwrap()).unwrap();
        let store = JsonSettingsStore::load_or_default(path);
        assert_eq!(store.load().max_recent_jobs, 7);
    }

    #[test]
    fn update_applies_only_patched_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonSettingsStore::load_or_default(tmp.path().join("settings.json"));
        let updated = store
            .update(SettingsPatchDto {
                max_recent_jobs: Some(10),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(updated.max_recent_jobs, 10);
        assert_eq!(
            updated.theme,
            Theme::System,
            "unpatched fields must be untouched"
        );
        assert_eq!(
            store.load().max_recent_jobs,
            10,
            "cache must reflect the write"
        );
    }

    #[test]
    fn update_persists_to_disk_and_survives_reload() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        let store = JsonSettingsStore::load_or_default(path.clone());
        store
            .update(SettingsPatchDto {
                show_advanced_command: Some(true),
                ..Default::default()
            })
            .unwrap();
        let reloaded = JsonSettingsStore::load_or_default(path);
        assert!(reloaded.load().show_advanced_command);
    }

    #[test]
    fn update_can_explicitly_clear_the_nullable_directory_field() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonSettingsStore::load_or_default(tmp.path().join("settings.json"));
        store
            .update(SettingsPatchDto {
                default_output_directory: Some(Some(PathBuf::from(r"C:\out"))),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            store.load().default_output_directory,
            Some(PathBuf::from(r"C:\out"))
        );
        store
            .update(SettingsPatchDto {
                default_output_directory: Some(None),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            store.load().default_output_directory,
            None,
            "an explicit null in the patch must clear the field"
        );
    }

    #[test]
    fn update_patch_with_field_absent_leaves_directory_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let store = JsonSettingsStore::load_or_default(tmp.path().join("settings.json"));
        store
            .update(SettingsPatchDto {
                default_output_directory: Some(Some(PathBuf::from(r"C:\out"))),
                ..Default::default()
            })
            .unwrap();
        store
            .update(SettingsPatchDto {
                max_recent_jobs: Some(3),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            store.load().default_output_directory,
            Some(PathBuf::from(r"C:\out")),
            "a patch that omits the field entirely must not clear it"
        );
    }
}
