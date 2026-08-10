// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Phase 0 smoke test.
 *
 * This only proves the test harness itself works (Vitest + React Testing
 * Library configured, jsdom environment, TSX rendering, mocking the IPC
 * client). It is not a real feature test - Phase 1+ should add proper
 * coverage for the real screens as they land.
 */
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("../ipc/client", () => ({
  getAppInfo: vi.fn().mockResolvedValue({
    name: "pgn-studio",
    version: "0.1.0",
    tauriVersion: "2.0.0",
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
