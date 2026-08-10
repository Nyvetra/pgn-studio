// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Placeholder for typed Tauri event-listener wrappers.
 *
 * architecture.md §14.2 defines a `JobEvent` union delivered over named
 * channels (`job://state`, `job://stage`, `job://log-line`, `job://metrics`,
 * `job://artifact`, `job://completed`), each carrying a job ID that the
 * frontend must use to ignore events from a stale/obsolete job.
 *
 * No jobs exist yet in Phase 0 - there is nothing to subscribe to. This
 * module reserves the boundary: feature code must never call
 * `@tauri-apps/api/event` directly. Typed helpers (e.g. `onJobStateChanged`)
 * should be added here starting in Phase 2, alongside the generated
 * `JobEvent` type in `generated-types.ts`.
 */
export {};
