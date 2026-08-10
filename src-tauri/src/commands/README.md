# commands/

Tauri `#[tauri::command]` entry points (the IPC surface listed in
architecture.md §14.1: `get_engine_info`, `select_input_files`,
`validate_job`, `start_job`, `cancel_job`, etc.).

Each command should stay a thin wrapper that deserializes/validates its DTO
and delegates to `application/` - it must not contain business logic itself.

Empty in Phase 0. The one command that exists so far
(`get_app_info`) is small enough to live directly in `lib.rs`; it should
move into this module once more commands make a dedicated module worthwhile
(Phase 1).
