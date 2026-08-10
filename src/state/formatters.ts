// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Presentation-only formatting helpers shared across screens. No IPC, no
 * React — kept pure so they can be unit tested directly.
 */
import type { ArtifactKind } from "../ipc/client";

/** Human labels for `ArtifactKind`, shared by the Review screen's
 * destination-artifacts list and the Results screen's artifact list so the
 * two never describe the same kind differently. */
export const ARTIFACT_KIND_LABELS: Record<ArtifactKind, string> = {
  uniqueGames: "Main output (unique games)",
  duplicateGames: "Duplicate games audit",
  reportJson: "Processing report (JSON)",
  reportText: "Processing report (text)",
  logText: "Log file",
};

/** The one, only string allowed to represent a metric that could not be
 * measured (architecture.md §9.3, §13.7, §25: "unknown values shown as
 * 'Not available' — never as 0"). Every place that renders an
 * `Option<u64>`-shaped metric must go through `formatCount`/`Metric` rather
 * than inlining a fallback, so this wording can never drift or regress to a
 * bare `0`. */
export const NOT_AVAILABLE = "Not available";

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) return NOT_AVAILABLE;
  if (bytes < 0) return NOT_AVAILABLE;
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exponent;
  const decimals = exponent === 0 ? 0 : value < 10 ? 2 : value < 100 ? 1 : 0;
  const trimmed = value.toFixed(decimals).replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, "");
  return `${trimmed} ${units[exponent]}`;
}

export function formatCount(value: number | null | undefined): string {
  if (value === null || value === undefined) return NOT_AVAILABLE;
  return value.toLocaleString("en-US");
}

export function formatDuration(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || ms < 0) return NOT_AVAILABLE;
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) {
    return `${hours}h ${String(minutes).padStart(2, "0")}m ${String(seconds).padStart(2, "0")}s`;
  }
  if (minutes > 0) {
    return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  }
  return `${seconds}s`;
}

export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return NOT_AVAILABLE;
  const parsed = new Date(iso);
  if (Number.isNaN(parsed.getTime())) return NOT_AVAILABLE;
  return parsed.toLocaleString();
}

/** File name from a full path, tolerant of both `\` and `/` separators
 * (Windows paths dominate this project, but source strings may arrive with
 * either). */
export function fileNameFromPath(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}
