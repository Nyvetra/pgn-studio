// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * A single shared polite live region, mounted once near the app root, for
 * screen-reader announcements of stage/status changes (architecture.md
 * §13.8). Feature code never renders its own `aria-live` region for this
 * purpose — it calls `useAnnounce()` (see `useAnnounce.ts`) so every
 * announcement funnels through one place and can never collide with
 * another.
 */
import { useCallback, useRef, useState, type ReactNode } from "react";
import { AnnouncerContext } from "./announcerContextInstance";

export function LiveAnnouncerProvider({ children }: { children: ReactNode }) {
  const [message, setMessage] = useState("");
  const timeoutRef = useRef<number | undefined>(undefined);

  const announce = useCallback((next: string) => {
    window.clearTimeout(timeoutRef.current);
    // Clear first so a repeated announcement (identical text twice in a
    // row) still produces a DOM mutation and gets re-announced.
    setMessage("");
    timeoutRef.current = window.setTimeout(() => setMessage(next), 50);
  }, []);

  return (
    <AnnouncerContext.Provider value={announce}>
      {children}
      <div className="visually-hidden" role="status" aria-live="polite" aria-atomic="true">
        {message}
      </div>
    </AnnouncerContext.Provider>
  );
}
