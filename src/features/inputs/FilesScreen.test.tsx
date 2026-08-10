// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { WorkflowProvider } from "../../state/WorkflowContext";
import { useWorkflow } from "../../state/useWorkflow";
import { FilesScreen } from "./FilesScreen";

const selectInputFiles = vi.fn();
const selectInputDirectory = vi.fn();
const selectOutputDirectory = vi.fn();
const inspectInputs = vi.fn();
const scanInputDirectory = vi.fn();

vi.mock("../../ipc/client", () => ({
  selectInputFiles: (...args: unknown[]) => selectInputFiles(...args),
  selectInputDirectory: (...args: unknown[]) => selectInputDirectory(...args),
  selectOutputDirectory: (...args: unknown[]) => selectOutputDirectory(...args),
  inspectInputs: (...args: unknown[]) => inspectInputs(...args),
  scanInputDirectory: (...args: unknown[]) => scanInputDirectory(...args),
}));

function renderFilesScreen() {
  return render(
    <WorkflowProvider>
      <FilesScreen />
    </WorkflowProvider>,
  );
}

/** Drives the reducer directly to simulate "the user already turned on
 * duplicate handling on the Operations screen" — Operations doesn't need to
 * exist yet for this Files-screen test to exercise the §13.2 requirement
 * that the note appears "whenever deduplication is enabled", independent of
 * visit order. */
function TurnOnDuplicateHandlingThenRenderFiles() {
  const { dispatch } = useWorkflow();
  useEffect(() => {
    dispatch({ type: "SET_DUPLICATE_POLICY", policy: "reportAndKeepFirst" });
  }, [dispatch]);
  return <FilesScreen />;
}

beforeEach(() => {
  selectInputFiles.mockReset().mockResolvedValue({ status: "ok", data: [] });
  selectInputDirectory.mockReset().mockResolvedValue({ status: "ok", data: null });
  selectOutputDirectory.mockReset().mockResolvedValue({ status: "ok", data: null });
  inspectInputs.mockReset().mockResolvedValue({ status: "ok", data: [] });
  scanInputDirectory.mockReset().mockResolvedValue({
    status: "ok",
    data: { files: [], recursive: false, directoriesScanned: 1, truncated: false, truncationNotes: [] },
  });
});

describe("FilesScreen", () => {
  it("Next is disabled until there is at least one input, an output folder, and a base name", async () => {
    const user = userEvent.setup();
    renderFilesScreen();
    const next = screen.getByRole("button", { name: "Next: Operations" });
    expect(next).toBeDisabled();

    selectInputFiles.mockResolvedValueOnce({ status: "ok", data: ["C:\\games\\a.pgn"] });
    await user.click(screen.getByRole("button", { name: "Add Files" }));
    await screen.findByText("a.pgn");
    expect(next).toBeDisabled();

    await user.type(screen.getByLabelText(/Output folder/), "C:\\out");
    expect(next).toBeDisabled();
    await user.type(screen.getByLabelText(/Base filename/), "clean");
    expect(next).toBeEnabled();
  });

  it("adds files chosen through the Add Files dialog to the source list, in order", async () => {
    const user = userEvent.setup();
    selectInputFiles.mockResolvedValueOnce({
      status: "ok",
      data: ["C:\\games\\a.pgn", "C:\\games\\b.pgn"],
    });
    renderFilesScreen();
    await user.click(screen.getByRole("button", { name: "Add Files" }));
    expect(await screen.findByText("a.pgn")).toBeInTheDocument();
    expect(screen.getByText("b.pgn")).toBeInTheDocument();
  });

  it("requests inspection for newly added files and shows the returned size", async () => {
    const user = userEvent.setup();
    selectInputFiles.mockResolvedValueOnce({ status: "ok", data: ["C:\\games\\a.pgn"] });
    inspectInputs.mockResolvedValueOnce({
      status: "ok",
      data: [
        {
          path: "C:\\games\\a.pgn",
          displayName: "a.pgn",
          sizeBytes: 4096,
          modifiedAt: null,
          isReadable: true,
          extensionOk: true,
          sha256: null,
          warnings: [],
        },
      ],
    });
    renderFilesScreen();
    await user.click(screen.getByRole("button", { name: "Add Files" }));
    await waitFor(() => expect(inspectInputs).toHaveBeenCalledWith(["C:\\games\\a.pgn"]));
    expect(await screen.findByText("4 KB")).toBeInTheDocument();
  });

  it("Add Folder scans the chosen folder and adds the reviewed files to the source list", async () => {
    const user = userEvent.setup();
    selectInputDirectory.mockResolvedValueOnce({ status: "ok", data: "C:\\collection" });
    scanInputDirectory.mockResolvedValueOnce({
      status: "ok",
      data: {
        files: [
          {
            path: "C:\\collection\\a.pgn",
            displayName: "a.pgn",
            sizeBytes: 1024,
            modifiedAt: null,
            isReadable: true,
            extensionOk: true,
            sha256: null,
            warnings: [],
          },
        ],
        recursive: false,
        directoriesScanned: 1,
        truncated: false,
        truncationNotes: [],
      },
    });
    renderFilesScreen();
    await user.click(screen.getByRole("button", { name: "Add Folder" }));
    expect(scanInputDirectory).toHaveBeenCalledWith("C:\\collection", {
      recursive: false,
      includeAllExtensions: false,
    });
    await user.click(await screen.findByRole("button", { name: "Add 1 File" }));
    expect(await screen.findByText("a.pgn")).toBeInTheDocument();
  });

  it("hides the duplicate-retention-order note when duplicate handling is off (the default)", () => {
    renderFilesScreen();
    expect(screen.queryByText(/Order matters while duplicate handling is on/)).not.toBeInTheDocument();
  });

  it("shows the duplicate-retention-order note whenever duplicate handling is enabled (§13.2 binding requirement)", () => {
    render(
      <WorkflowProvider>
        <TurnOnDuplicateHandlingThenRenderFiles />
      </WorkflowProvider>,
    );
    expect(screen.getByText(/Order matters while duplicate handling is on/)).toBeInTheDocument();
    expect(screen.getByText(/first copy in this list/)).toBeInTheDocument();
  });

  it('never labels duplicate handling as "Keep best copy" (§10.7 binding wording rule)', () => {
    render(
      <WorkflowProvider>
        <TurnOnDuplicateHandlingThenRenderFiles />
      </WorkflowProvider>,
    );
    expect(screen.queryByText(/best copy/i)).not.toBeInTheDocument();
  });
});
