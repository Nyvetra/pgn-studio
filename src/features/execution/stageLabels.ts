// SPDX-License-Identifier: GPL-3.0-or-later
import type { JobStage } from "../../ipc/client";

/** Friendly, stable titles for each `JobStage` — independent of the
 * backend's own short `message` string (e.g. "Preparing workspace"), which
 * is still shown alongside this as supporting detail. */
export const STAGE_LABELS: Record<JobStage, string> = {
  preparing: "Preparing",
  starting: "Starting the engine",
  processing: "Processing games",
  finalizing: "Finishing up",
};
