// SPDX-License-Identifier: GPL-3.0-or-later
/** Split out from `LiveAnnouncer.tsx`/`useAnnounce.ts` for the same reason
 * as `state/workflowContextInstance.ts` — see that file's doc comment. */
import { createContext } from "react";

export type Announce = (message: string) => void;

export const AnnouncerContext = createContext<Announce | null>(null);
