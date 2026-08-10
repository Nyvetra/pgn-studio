// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Hand-written placeholder for Rust-to-TypeScript generated types.
 *
 * architecture.md §14.3 requires the Tauri command/event DTOs to be kept in
 * sync through a generator (or a checked-in JSON Schema step in CI) rather
 * than maintained by hand indefinitely. Phase 0 only exposes one trivial
 * command (`get_app_info`), so its response shape is still hand-written
 * here. Introduce the real generator (for example `tauri-specta`, or a
 * `ts-rs`/JSON-Schema export step wired into CI) in Phase 1 when the real
 * `JobSpec` / `JobResult` DTOs from architecture.md §9 land, and regenerate
 * this file instead of extending it by hand.
 *
 * Field names use camelCase to match normal TypeScript/JSON conventions.
 * The corresponding Rust struct in `src-tauri/src/lib.rs` is annotated with
 * `#[serde(rename_all = "camelCase")]` so the wire format matches exactly.
 */

/** Mirrors the Rust `AppInfo` struct returned by the `get_app_info` command. */
export interface AppInfo {
  name: string;
  version: string;
  tauriVersion: string;
}
