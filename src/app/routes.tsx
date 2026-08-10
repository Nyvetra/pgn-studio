// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Placeholder for the application's screen/route map.
 *
 * architecture.md §13.1 defines the five-step MVP workflow:
 *   1. Files -> 2. Operations -> 3. Filters -> 4. Review -> 5. Run & Results
 *
 * Phase 0 ships a single diagnostic screen (see `App.tsx`) and does not
 * need real navigation yet. This module exists so Phase 2 has an obvious
 * place to introduce routing/step state instead of inventing a new
 * convention at that point.
 */

/** Screens planned for the Version 1 MVP workflow (architecture.md §13.1). */
export type AppRoute =
  | "diagnostics"
  | "files"
  | "operations"
  | "filters"
  | "review"
  | "run-results";

/** Only "diagnostics" exists until Phase 2 implements the real workflow. */
export const APP_ROUTES: readonly AppRoute[] = ["diagnostics"];
