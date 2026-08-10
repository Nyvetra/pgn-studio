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
import { vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() =>
    Promise.reject(
      new Error(
        "@tauri-apps/api/core#invoke was called without a test-specific mock",
      ),
    ),
  ),
}));
