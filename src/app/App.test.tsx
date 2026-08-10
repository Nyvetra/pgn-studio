// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * App-level smoke/integration tests. Individual screens have their own
 * focused test suites (`src/features/**\/*.test.tsx`) — this file only
 * checks that the real five-step workflow is wired together correctly:
 * AppShell renders the right screen for the current step, and the Stepper
 * reflects it.
 */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { App } from "./App";

const selectInputFiles = vi.fn();
const getEngineCapabilities = vi.fn();
const inspectInputs = vi.fn();

vi.mock("../ipc/client", async () => {
  const actual = await vi.importActual<typeof import("../ipc/client")>("../ipc/client");
  return {
    ...actual,
    selectInputFiles: (...args: unknown[]) => selectInputFiles(...args),
    getEngineCapabilities: (...args: unknown[]) => getEngineCapabilities(...args),
    inspectInputs: (...args: unknown[]) => inspectInputs(...args),
  };
});

vi.mock("../ipc/events", () => ({
  onJobState: vi.fn().mockResolvedValue(vi.fn()),
  onJobStage: vi.fn().mockResolvedValue(vi.fn()),
  onJobLog: vi.fn().mockResolvedValue(vi.fn()),
  onJobMetrics: vi.fn().mockResolvedValue(vi.fn()),
  onJobArtifact: vi.fn().mockResolvedValue(vi.fn()),
  onJobCompleted: vi.fn().mockResolvedValue(vi.fn()),
}));

beforeEach(() => {
  selectInputFiles.mockReset().mockResolvedValue({ status: "ok", data: ["C:\\games\\a.pgn"] });
  inspectInputs.mockReset().mockResolvedValue({ status: "ok", data: [] });
  getEngineCapabilities.mockReset().mockResolvedValue({
    status: "ok",
    data: {
      identity: { version: "v26-06", sha256: "a".repeat(64), targetTriple: "x86_64-pc-windows-msvc" },
      duplicateDetection: true,
      duplicateAuditFile: true,
      externalDuplicateTable: true,
      checkFile: true,
      ecoClassification: true,
      fenPatterns: true,
      textualVariations: true,
      fixResultTags: true,
      rejectBadResults: true,
      separateBrokenOutput: false,
      supportedOutputFormats: ["san"],
      unicodePaths: false,
    },
  });
});

/** Scopes a query to the Stepper's own `<nav>` landmark — several screens
 * also have a "Next: <NextStepLabel>" button whose accessible name
 * otherwise collides with the step button's own name (e.g. "Next:
 * Operations" vs. the stepper's "Operations" step). */
function stepperButton(name: string | RegExp) {
  return within(screen.getByRole("navigation", { name: "Workflow progress" })).getByRole("button", {
    name,
  });
}

describe("App", () => {
  it("renders the PGN Studio heading and starts on the Files step", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "PGN Studio", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Files", level: 2 })).toBeInTheDocument();
    expect(stepperButton(/Files/)).toHaveAttribute("aria-current", "step");
  });

  it("only the Files step is reachable via the stepper before any progress is made", () => {
    render(<App />);
    expect(stepperButton(/Operations/)).toBeDisabled();
    expect(stepperButton(/Run & Results/)).toBeDisabled();
  });

  it("moving from Files to Operations via Next renders the Operations screen and updates the stepper", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "Add Files" }));
    await screen.findByText("a.pgn");
    await user.type(screen.getByLabelText(/Output folder/), "C:\\out");
    await user.type(screen.getByLabelText(/Base filename/), "clean");

    await user.click(screen.getByRole("button", { name: "Next: Operations" }));

    expect(screen.getByRole("heading", { name: "Operations", level: 2 })).toBeInTheDocument();
    expect(stepperButton(/Operations/)).toHaveAttribute("aria-current", "step");
    // Files is no longer current, but stays reachable (backward nav never
    // loses settings, architecture.md §13.1).
    expect(stepperButton(/Files/)).toBeEnabled();
  });

  it("navigating back to Files via the stepper preserves what was entered", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(screen.getByRole("button", { name: "Add Files" }));
    await screen.findByText("a.pgn");
    await user.type(screen.getByLabelText(/Output folder/), "C:\\out");
    await user.type(screen.getByLabelText(/Base filename/), "keep-me");
    await user.click(screen.getByRole("button", { name: "Next: Operations" }));

    await user.click(stepperButton(/Files/));
    expect(screen.getByRole("heading", { name: "Files", level: 2 })).toBeInTheDocument();
    expect(screen.getByLabelText(/Base filename/)).toHaveValue("keep-me");
    expect(screen.getByText("a.pgn")).toBeInTheDocument();
  });
});
