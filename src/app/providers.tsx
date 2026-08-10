// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Composition root for React context providers.
 *
 * Phase 0 has no global state yet (no settings store, theme, or
 * toast/notification host), so this currently just passes children
 * through unchanged. Later phases should register new providers here
 * rather than nesting them ad hoc inside `App.tsx`, so the provider stack
 * stays in one reviewable place.
 */
import type { JSX, PropsWithChildren } from "react";

export function AppProviders({ children }: PropsWithChildren): JSX.Element {
  return <>{children}</>;
}
