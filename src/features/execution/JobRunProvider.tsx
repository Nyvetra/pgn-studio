// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Provides one shared `useJobRun()` instance for the whole app, so the
 * Review screen (which starts the job) and the Run/Results screen (which
 * displays it) observe the exact same live state rather than each holding
 * an independent, unsynchronized copy.
 */
import type { ReactNode } from "react";
import { useJobRun } from "./useJobRun";
import { JobRunContext } from "./jobRunContextInstance";

export function JobRunProvider({ children }: { children: ReactNode }) {
  const jobRun = useJobRun();
  return <JobRunContext.Provider value={jobRun}>{children}</JobRunContext.Provider>;
}
