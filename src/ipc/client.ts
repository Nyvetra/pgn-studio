// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Typed wrapper around the Tauri `invoke` bridge.
 *
 * This is the only module the rest of the frontend is allowed to import
 * `@tauri-apps/api/core#invoke` through. Every Rust command exposed to the
 * UI should get a small typed wrapper function here so feature code never
 * passes untyped command names/payloads across the IPC boundary directly.
 *
 * Phase 0 scope: only `getAppInfo`, which exists to prove the IPC boundary
 * works end to end. Phase 1+ will grow this into the full command surface
 * described in architecture.md §14.1 (job lifecycle, engine capabilities,
 * filesystem dialogs, settings, history, etc.).
 */
import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./generated-types";

/** Calls the `get_app_info` Tauri command. */
export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}
