# errors/

The public error taxonomy and shape (architecture.md §18): `ErrorCode`
enum (`INPUT_NOT_FOUND`, `ENGINE_TAMPERED`, `JOB_CANCELLED`, etc.) and
`PublicError` (code, title, message, remediation, log path, technical ID).
Responsible for redacting Rust backtraces/internal errors from the default
UI while preserving them in the local log.

Empty in Phase 0.
