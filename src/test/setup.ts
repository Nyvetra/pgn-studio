// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Vitest global setup, loaded via `test.setupFiles` in vite.config.ts.
 *
 * Registers the jest-dom matchers (`toBeInTheDocument`, `toHaveTextContent`,
 * etc.) on Vitest's `expect`, and mocks the `@tauri-apps/api` core bridge so
 * component tests never try to make a real IPC call outside a Tauri
 * webview.
 */
import "@testing-library/jest-dom/vitest";
import { afterEach, expect, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { toHaveNoViolations } from "jest-axe";

// Registers `toHaveNoViolations()` (architecture.md §13.8: "add automated
// a11y assertions") on Vitest's own `expect` - `jest-axe` ships typed for
// Jest's global `expect`, not Vitest's, but the runtime matcher object
// `expect.extend` accepts is plain and framework-agnostic; the
// corresponding TypeScript augmentation for Vitest's `Assertion` interface
// lives in `src/test/vitest-matchers.d.ts`.
expect.extend(toHaveNoViolations);

// This project's vite.config.ts does not set `test.globals: true`, so
// @testing-library/react's own auto-cleanup (which detects a *global*
// `afterEach`) never registers itself — without this, each `render()` call
// within a test file would keep accumulating in `document.body` instead of
// unmounting after its own test, corrupting every later `getByRole`/query
// in that file with leftover elements from earlier tests.
afterEach(() => {
  cleanup();
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() =>
    Promise.reject(
      new Error(
        "@tauri-apps/api/core#invoke was called without a test-specific mock",
      ),
    ),
  ),
}));
