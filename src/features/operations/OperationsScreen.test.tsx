// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, type ReactNode } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { WorkflowProvider } from "../../state/WorkflowContext";
import { useWorkflow } from "../../state/useWorkflow";
import type { EngineCapabilities } from "../../ipc/client";
import { OperationsScreen } from "./OperationsScreen";

const selectInputFiles = vi.fn();
vi.mock("../../ipc/client", async () => {
  const actual = await vi.importActual<typeof import("../../ipc/client")>("../../ipc/client");
  return { ...actual, selectInputFiles: (...args: unknown[]) => selectInputFiles(...args) };
});

function fullCapabilities(overrides: Partial<EngineCapabilities> = {}): EngineCapabilities {
  return {
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
    ...overrides,
  };
}

function SeedCapabilities({ capabilities }: { capabilities: EngineCapabilities }) {
  const { dispatch } = useWorkflow();
  useEffect(() => {
    dispatch({ type: "SET_CAPABILITIES", capabilities });
  }, [dispatch, capabilities]);
  return null;
}

function renderScreen(capabilities?: EngineCapabilities) {
  return render(
    <WorkflowProvider>
      {capabilities && <SeedCapabilities capabilities={capabilities} />}
      <OperationsScreen />
    </WorkflowProvider>,
  );
}

beforeEach(() => {
  selectInputFiles.mockReset().mockResolvedValue({ status: "ok", data: [] });
});

describe("OperationsScreen", () => {
  it("applying a preset updates the visible controls", async () => {
    const user = userEvent.setup();
    renderScreen(fullCapabilities());
    await user.click(screen.getByRole("button", { name: /Minimal Mainline PGN/ }));
    expect(screen.getByLabelText("Remove comments")).toBeChecked();
    expect(screen.getByLabelText("Remove variations")).toBeChecked();
  });

  it("marks the applied preset as pressed and shows Custom once a control is edited by hand", async () => {
    const user = userEvent.setup();
    renderScreen(fullCapabilities());
    const mergeSafely = screen.getByRole("button", { name: /Merge Safely/ });
    expect(mergeSafely).toHaveAttribute("aria-pressed", "true"); // matches the default draft

    await user.click(screen.getByLabelText("Remove comments"));
    expect(mergeSafely).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByText("Custom")).toBeInTheDocument();
  });

  it('never renders "keep best copy" anywhere on this screen (§10.7 binding wording rule)', () => {
    renderScreen(fullCapabilities());
    expect(screen.queryByText(/best copy/i)).not.toBeInTheDocument();
  });

  it("states plainly that there is no separate broken-games file", () => {
    renderScreen(fullCapabilities());
    expect(screen.getAllByText(/no separate file for them/).length).toBeGreaterThan(0);
    expect(screen.queryByText(/broken\.pgn/)).not.toBeInTheDocument();
  });

  it("disables capability-gated options while capabilities are still loading", () => {
    renderScreen(undefined);
    expect(screen.getByLabelText("Add ECO opening classification tags")).toBeDisabled();
    expect(screen.getAllByText("Checking what this engine build supports…").length).toBeGreaterThan(0);
  });

  it("disables an option the engine build does not support, with an explanation", () => {
    renderScreen(fullCapabilities({ ecoClassification: false }));
    expect(screen.getByLabelText("Add ECO opening classification tags")).toBeDisabled();
    expect(screen.getByText("Not supported by this engine build.")).toBeInTheDocument();
  });

  it("enables ECO once capabilities confirm support", () => {
    renderScreen(fullCapabilities({ ecoClassification: true }));
    expect(screen.getByLabelText("Add ECO opening classification tags")).toBeEnabled();
  });

  it("always shows UCI notation as a disabled, explained option rather than hiding it", () => {
    renderScreen(fullCapabilities());
    const uci = screen.getByRole("radio", { name: /UCI notation/ });
    expect(uci).toBeDisabled();
    expect(screen.getByRole("radio", { name: /Standard Algebraic Notation/ })).toBeEnabled();
  });

  it("switching to validateOnly mode disables the duplicate-policy controls", async () => {
    const user = userEvent.setup();
    renderScreen(fullCapabilities());
    await user.click(screen.getByRole("radio", { name: /Validate only/ }));
    const duplicateFieldset = screen.getByRole("radio", { name: /Do not check for duplicates/ }).closest("fieldset");
    expect(duplicateFieldset).toBeDisabled();
  });

  it('selecting "keep first, save an audit file" checks the publish-audit checkbox by default', async () => {
    const user = userEvent.setup();
    renderScreen(fullCapabilities());
    await user.click(screen.getByRole("radio", { name: /Keep first copy, save the rest to an audit file/ }));
    expect(screen.getByLabelText("Keep the audit file")).toBeChecked();
  });

  it("picking a check file calls select_input_files and displays the chosen path", async () => {
    const user = userEvent.setup();
    selectInputFiles.mockResolvedValueOnce({ status: "ok", data: ["C:\\master.pgn"] });
    renderScreen(fullCapabilities());
    await user.click(screen.getByRole("radio", { name: /Keep first copy, discard the rest/ }));
    await user.click(screen.getByRole("button", { name: "Browse…" }));
    expect(await screen.findByDisplayValue("C:\\master.pgn")).toBeInTheDocument();
  });

  it("adds and removes a header tag to remove, validating the identifier shape", async () => {
    const user = userEvent.setup();
    renderScreen(fullCapabilities());
    await user.click(screen.getByText("More cleanup options"));

    const tagField = screen.getByLabelText(/Remove specific header tags/);
    await user.type(tagField, "1Bad");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByText(/must start with a letter/)).toBeInTheDocument();

    await user.clear(tagField);
    await user.type(tagField, "Annotator");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(screen.getByText("Annotator")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Stop removing the Annotator tag" }));
    expect(screen.queryByText("Annotator")).not.toBeInTheDocument();
  });

  it("Back returns to the Files step and Next advances to Filters", async () => {
    const user = userEvent.setup();
    function Probe() {
      const { state } = useWorkflow();
      return <p data-testid="step">{state.step}</p>;
    }
    // In the real app, AppShell only ever mounts OperationsScreen once
    // state.step already equals "operations" (reached via FilesScreen's own
    // Next button) — this test recreates that precondition explicitly
    // rather than asserting on `dispatch({ type: "GO_NEXT" })` composing
    // correctly from screen to screen, which is App-level, not this
    // screen's own concern.
    function AdvanceToOperationsFirst({ children }: { children: ReactNode }) {
      const { state, dispatch } = useWorkflow();
      // Mount-only: this is a one-time "skip the Files step" for this test,
      // not a standing rule — it must not re-fire later when the real
      // "Back" navigation under test legitimately returns to "files".
      useEffect(() => {
        dispatch({ type: "GO_NEXT" });
        // eslint-disable-next-line react-hooks/exhaustive-deps
      }, []);
      return state.step === "files" ? null : <>{children}</>;
    }
    render(
      <WorkflowProvider>
        <SeedCapabilities capabilities={fullCapabilities()} />
        <AdvanceToOperationsFirst>
          <OperationsScreen />
        </AdvanceToOperationsFirst>
        <Probe />
      </WorkflowProvider>,
    );
    expect(screen.getByTestId("step")).toHaveTextContent("operations");
    await user.click(screen.getByRole("button", { name: "Next: Filters" }));
    expect(screen.getByTestId("step")).toHaveTextContent("filters");
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByTestId("step")).toHaveTextContent("files");
  });
});
