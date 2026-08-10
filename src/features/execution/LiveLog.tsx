// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Bounded live log view (architecture.md §13.6, §10.9: keep the most
 * recent ~2,000 rendered lines — the ring buffer itself lives in
 * `jobRunReducer.ts`, this component only ever renders whatever it is
 * given). Not an `aria-live` region: announcing every raw engine line would
 * make the app unusable with a screen reader. Stage/status changes are
 * announced separately and far more sparingly (see `RunScreen.tsx`'s use
 * of `useAnnounce`); a screen-reader user can still read this log by
 * navigating into it normally.
 */
import { useEffect, useRef } from "react";
import type { LogEntry } from "../../state/jobRunReducer";
import "./LiveLog.css";

export interface LiveLogProps {
  logs: LogEntry[];
}

export function LiveLog({ logs }: LiveLogProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const wasAtBottomRef = useRef(true);

  useEffect(() => {
    const el = containerRef.current;
    if (el && wasAtBottomRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, [logs]);

  function handleScroll() {
    const el = containerRef.current;
    if (!el) return;
    wasAtBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
  }

  return (
    <div
      className="live-log"
      ref={containerRef}
      onScroll={handleScroll}
      tabIndex={0}
      role="log"
      aria-label="Engine log"
    >
      {logs.length === 0 ? (
        <p className="live-log__empty">No log output yet.</p>
      ) : (
        <ol className="live-log__lines">
          {logs.map((entry) => (
            <li key={entry.seq} className={`live-log__line live-log__line--${entry.level}`}>
              {entry.line}
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}
