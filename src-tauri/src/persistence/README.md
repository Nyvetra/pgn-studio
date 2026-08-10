# persistence/

Settings and job-history storage (architecture.md §15): versioned JSON
settings in the platform app-config directory, and either bounded JSON
manifests or a Rust-owned SQLite (`rusqlite`) job-history store. Must never
store complete PGN content in the history database (architecture.md §15.2).

Empty in Phase 0.
