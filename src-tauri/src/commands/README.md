# commands/

Tauri `#[tauri::command]` entry points (the IPC surface listed in
architecture.md §14.1: `get_engine_info`, `select_input_files`,
`validate_job`, `start_job`, `cancel_job`, etc.).

Each command stays a thin wrapper that deserializes/validates its DTO and
delegates to `application/` - it must not contain business logic itself.

Implemented in Phase 2a: all 18 commands from architecture.md §14.1 /
design-02 §4.1, split by concern (`app.rs`, `engine.rs`, `dialogs.rs`,
`inputs.rs`, `jobs.rs`, `settings.rs`), plus the DTOs that have no closer
home (`dto.rs`). `mod.rs`'s `specta_builder()` is the single source of truth
for the exported command/type surface - both the real app and the
`--export-bindings` CLI mode in `lib.rs::run()` call it, so
`src/ipc/generated-types.ts` can never drift from what is actually
registered (design-02 §4.3/D-17; regenerate via
`cargo run -p xtask -- export-bindings`).
