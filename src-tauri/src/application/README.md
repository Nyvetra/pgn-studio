# application/

Application/orchestration layer (architecture.md §7.1): coordinates the
domain model, the engine port, the filesystem port, and the history/settings
port. This is where job validation, job lifecycle transitions
(`Draft -> Validating -> Ready -> Running -> ...`, architecture.md §9.1),
and cancellation orchestration live.

Commands in `commands/` should be thin callers into this layer, not
reimplement it.

Empty in Phase 0.
