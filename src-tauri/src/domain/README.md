# domain/

Core domain types and validation, independent of Tauri, React, or the
pinned `pgn-extract` version (architecture.md §7.1, §9): `JobSpec`,
`InputFile`, `OutputPlan`, `OperationPlan`, `FilterPlan`, `DuplicatePolicy`,
`CleanupOptions`, `JobResult`, `ProcessingMetrics`, etc.

Dependencies point inward - this module must not import `tauri`, `serde`
wire-format concerns belong at the DTO boundary, not here.

Empty in Phase 0. Implementation starts in Phase 1 ("Implement typed
`JobSpec` subset").
