// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Moves keyboard/screen-reader focus to an element once, the moment it
 * mounts (architecture.md §13.8: keyboard navigation + screen-reader
 * announcements for stage/status changes).
 *
 * Every one of the five workflow steps (`AppShell.tsx`'s `state.step ===
 * "..."` branches) is a full-subtree swap, not a real page navigation, and
 * the Run -> Results swap inside `RunResultsStep` is the same kind of
 * swap one level down. Without this, focus silently stays wherever it was
 * (usually the "Next"/"Back" button that just got removed from the DOM,
 * which drops focus to `<body>`) and nothing tells either a sighted
 * keyboard user or a screen-reader user that the step actually changed —
 * they would have to notice the new heading on their own. Attaching the
 * ref this hook returns to each screen's own `<h2>` (with `tabIndex={-1}`,
 * so it is programmatically focusable without joining the normal Tab
 * order) and moving focus there on mount is the standard remediation for
 * exactly this SPA/wizard pattern.
 */
import { useEffect, useRef } from "react";

export function useFocusOnMount<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  useEffect(() => {
    ref.current?.focus();
    // Deliberately fires once per mount, not on every re-render — the
    // point is "this step/screen just appeared", not "something inside it
    // changed". `ref` is a stable object identity across renders, so an
    // empty dependency array is correct here, not a suppressed warning.
  }, []);
  return ref;
}
