// SPDX-License-Identifier: GPL-3.0-or-later
/** Split out from `JobRunProvider.tsx`/`useJobRunContext.ts` for the same
 * reason as `state/workflowContextInstance.ts` — see that file's doc
 * comment (react-refresh's `only-export-components` rule). */
import { createContext } from "react";
import type { UseJobRun } from "./useJobRun";

export const JobRunContext = createContext<UseJobRun | null>(null);
