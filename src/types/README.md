# types/

General-purpose TypeScript types used across the frontend that are *not*
part of the Tauri IPC contract (UI-only enums, view-model shapes, workflow
step types, etc.).

Types that mirror Rust DTOs sent over IPC belong in `src/ipc/generated-types.ts`
instead, so the wire contract stays in one place. This directory is empty in
Phase 0.
