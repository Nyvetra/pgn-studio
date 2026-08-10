// SPDX-License-Identifier: GPL-3.0-or-later
import { useContext } from "react";
import { JobRunContext } from "./jobRunContextInstance";
import type { UseJobRun } from "./useJobRun";

export function useJobRunContext(): UseJobRun {
  const ctx = useContext(JobRunContext);
  if (!ctx) {
    throw new Error("useJobRunContext must be used within a JobRunProvider");
  }
  return ctx;
}
