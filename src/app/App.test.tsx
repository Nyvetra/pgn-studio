// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Phase 2a smoke test.
 *
 * Proves the test harness (Vitest + React Testing Library, jsdom, TSX
 * rendering, mocking the typed IPC client) still works against the real
 * `AppInfoDto` shape (`{ appVersion, os, arch }`, design-02 §4.1) instead
 * of the old Phase 0 placeholder shape. Not real feature coverage — Phase
 * 2b should add proper coverage for the real screens as they land.
 */
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("../ipc/client", () => ({
  getAppInfo: vi.fn().mockResolvedValue({
    appVersion: "0.1.0",
    os: "windows",
    arch: "x86_64",
  }),
}));

describe("App", () => {
  it("renders the PGN Studio heading", () => {
    render(<App />);
    expect(
      screen.getByRole("heading", { name: "PGN Studio" }),
    ).toBeInTheDocument();
  });

  it("shows a loading status before the backend responds", () => {
    render(<App />);
    expect(screen.getByRole("status")).toHaveTextContent(
      "Contacting Rust backend",
    );
  });
});
