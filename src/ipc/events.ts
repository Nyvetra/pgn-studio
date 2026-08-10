// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Typed `job://*` event listener wrappers (architecture.md §14.2; design-02
 * §4.2).
 *
 * `JobEvent` (imported from `./generated-types`, generated from the single
 * Rust enum `application::events::JobEvent`) is a closed, `type`-tagged
 * union carrying every variant's shape. Each variant is emitted on its own
 * named channel (`job://state`, `job://stage`, `job://log`,
 * `job://metrics`, `job://artifact`, `job://completed`) rather than through
 * `tauri-specta`'s own `Event`/`collect_events!` machinery, which binds one
 * Rust type to exactly one channel — a shape that does not fit "one closed
 * union, six channels" (see the Rust-side doc comment on
 * `application::events::JobEvent` for the full reasoning). This file is
 * therefore hand-written rather than generated, exactly the kind of "one
 * small hand-written typed listen wrapper" design-02 §4.3 already
 * pre-approves for the ts-rs fallback path — here it is only the event
 * side, since every command still gets a fully generated typed wrapper
 * (`./client.ts`).
 *
 * Feature code must import event listeners from here, never call
 * `@tauri-apps/api/event#listen` directly with a raw `"job://..."` string —
 * that would bypass the typing this module exists to provide.
 *
 * **Correlation rule (binding, design-02 §4.2):** the caller must record
 * `activeJobId` at `startJob`'s resolution and drop every incoming event
 * whose `jobId` does not match before any state mutation; within the active
 * job, drop events whose `seq` is `<= lastSeq`. Register listeners *before*
 * calling `startJob` (`client.ts`) so there is no gap — `startJob` itself
 * does not resolve until the backend has committed to running the job (see
 * `application::jobs::start_job`'s doc comment), so awaiting the `listen()`
 * promises before calling `startJob` is sufficient.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { JobEvent } from "./generated-types";

type JobEventOf<Type extends JobEvent["type"]> = Extract<JobEvent, { type: Type }>;

/** `job://state`: `Running` / `Cancelling` / `Succeeded` / `Failed` / `Cancelled`. */
export function onJobState(
  handler: (event: JobEventOf<"state">) => void,
): Promise<UnlistenFn> {
  return listen<JobEventOf<"state">>("job://state", (e) => handler(e.payload));
}

/** `job://stage`: `Preparing` / `Starting` / `Processing` / `Finalizing`. */
export function onJobStage(
  handler: (event: JobEventOf<"stage">) => void,
): Promise<UnlistenFn> {
  return listen<JobEventOf<"stage">>("job://stage", (e) => handler(e.payload));
}

/** `job://log`: one batched engine log line (design-02 §2.3: ~100ms batches). */
export function onJobLog(handler: (event: JobEventOf<"log">) => void): Promise<UnlistenFn> {
  return listen<JobEventOf<"log">>("job://log", (e) => handler(e.payload));
}

/** `job://metrics`: a live progress tick (never a fabricated/guessed value — see `ProcessingMetrics`). */
export function onJobMetrics(
  handler: (event: JobEventOf<"metrics">) => void,
): Promise<UnlistenFn> {
  return listen<JobEventOf<"metrics">>("job://metrics", (e) => handler(e.payload));
}

/** `job://artifact`: one published output artifact, emitted as it lands. */
export function onJobArtifact(
  handler: (event: JobEventOf<"artifact">) => void,
): Promise<UnlistenFn> {
  return listen<JobEventOf<"artifact">>("job://artifact", (e) => handler(e.payload));
}

/** `job://completed`: the terminal `JobResult` — also mirrored by `getJob` for reconciliation after a reload. */
export function onJobCompleted(
  handler: (event: JobEventOf<"completed">) => void,
): Promise<UnlistenFn> {
  return listen<JobEventOf<"completed">>("job://completed", (e) => handler(e.payload));
}
