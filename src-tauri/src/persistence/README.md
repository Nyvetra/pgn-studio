# persistence/

Settings and job-history storage (architecture.md §15): versioned JSON
settings in the platform app-config directory, and either bounded JSON
manifests or a Rust-owned SQLite (`rusqlite`) job-history store. Must never
store complete PGN content in the history database (architecture.md §15.2).

Implemented in Phase 2a as bounded JSON (architecture.md §15.2 permits this
for the MVP instead of SQLite):
- `settings.rs` - `SettingsDto`/`SettingsPatchDto` and the `SettingsStore`
  trait + `JsonSettingsStore` implementation, with a `schemaVersion`
  migration hook.
- `history.rs` - `JobSummaryDto` and the `HistoryStore` trait +
  `JsonHistoryStore` implementation: a bounded *index* only (never full job
  results - the authoritative per-job record stays the workspace's own
  `filesystem::manifest::FinalManifest`, matching architecture.md §15.2's
  own suggested schema, where the `jobs` table stores a `manifest_path`
  pointer rather than inline content).

Both stores are deliberately kept behind small traits so a future SQLite
implementation can swap in without touching `application/`/`commands/`
(task instruction: "Keep the storage layer small and behind a trait so
Phase 6 can swap it").
