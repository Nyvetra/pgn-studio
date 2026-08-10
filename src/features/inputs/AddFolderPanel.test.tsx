// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { AddFolderPanel } from "./AddFolderPanel";
import type { InputInspectionDto } from "../../ipc/client";

const selectInputDirectory = vi.fn();
const scanInputDirectory = vi.fn();

vi.mock("../../ipc/client", () => ({
  selectInputDirectory: (...args: unknown[]) => selectInputDirectory(...args),
  scanInputDirectory: (...args: unknown[]) => scanInputDirectory(...args),
}));

function inspectedFile(overrides: Partial<InputInspectionDto> = {}): InputInspectionDto {
  return {
    path: "C:\\collection\\a.pgn",
    displayName: "a.pgn",
    sizeBytes: 1024,
    modifiedAt: null,
    isReadable: true,
    extensionOk: true,
    sha256: null,
    warnings: [],
    ...overrides,
  };
}

beforeEach(() => {
  selectInputDirectory.mockReset().mockResolvedValue({ status: "ok", data: "C:\\collection" });
  scanInputDirectory.mockReset().mockResolvedValue({
    status: "ok",
    data: { files: [inspectedFile()], recursive: false, directoriesScanned: 1, truncated: false, truncationNotes: [] },
  });
});

describe("AddFolderPanel", () => {
  it("does nothing when the user cancels the folder picker", async () => {
    selectInputDirectory.mockResolvedValueOnce({ status: "ok", data: null });
    const user = userEvent.setup();
    const onFilesChosen = vi.fn();
    render(<AddFolderPanel onFilesChosen={onFilesChosen} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));
    expect(scanInputDirectory).not.toHaveBeenCalled();
    expect(screen.queryByText(/Found/)).not.toBeInTheDocument();
  });

  it("scans non-recursively by default and shows a review before anything is added", async () => {
    const user = userEvent.setup();
    render(<AddFolderPanel onFilesChosen={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));

    expect(await screen.findByText("Found 1 file in this folder.")).toBeInTheDocument();
    expect(scanInputDirectory).toHaveBeenCalledWith("C:\\collection", {
      recursive: false,
      includeAllExtensions: false,
    });
    expect(screen.getByText("a.pgn")).toBeInTheDocument();
    // Nothing has been added to the job yet - review is a distinct step.
    expect(screen.getByRole("button", { name: "Add 1 File" })).toBeInTheDocument();
  });

  it("Include subfolders rescans immediately with recursive: true", async () => {
    const user = userEvent.setup();
    render(<AddFolderPanel onFilesChosen={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));
    await screen.findByText("Found 1 file in this folder.");

    scanInputDirectory.mockResolvedValueOnce({
      status: "ok",
      data: {
        files: [inspectedFile(), inspectedFile({ path: "C:\\collection\\sub\\b.pgn", displayName: "b.pgn" })],
        recursive: true,
        directoriesScanned: 2,
        truncated: false,
        truncationNotes: [],
      },
    });
    await user.click(screen.getByRole("checkbox", { name: /Include subfolders/ }));

    expect(scanInputDirectory).toHaveBeenLastCalledWith("C:\\collection", {
      recursive: true,
      includeAllExtensions: false,
    });
    expect(await screen.findByText(/Found 2 files, including subfolders/)).toBeInTheDocument();
  });

  it("Include files without a .pgn extension rescans with includeAllExtensions: true", async () => {
    const user = userEvent.setup();
    render(<AddFolderPanel onFilesChosen={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));
    await screen.findByText("Found 1 file in this folder.");

    await user.click(screen.getByRole("checkbox", { name: /without a \.pgn extension/ }));

    await waitFor(() =>
      expect(scanInputDirectory).toHaveBeenLastCalledWith("C:\\collection", {
        recursive: false,
        includeAllExtensions: true,
      }),
    );
  });

  it("shows every truncation note and still allows adding the files that were found", async () => {
    scanInputDirectory.mockResolvedValue({
      status: "ok",
      data: {
        files: [inspectedFile()],
        recursive: false,
        directoriesScanned: 1,
        truncated: true,
        truncationNotes: ["Stopped after finding 10000 matching files; some files may be missing from this list."],
      },
    });
    const user = userEvent.setup();
    render(<AddFolderPanel onFilesChosen={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));

    expect(await screen.findByText(/Stopped after finding 10000 matching files/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add 1 File" })).toBeEnabled();
  });

  it("Add N Files calls onFilesChosen with every matched path and resets the panel", async () => {
    scanInputDirectory.mockResolvedValueOnce({
      status: "ok",
      data: {
        files: [inspectedFile(), inspectedFile({ path: "C:\\collection\\b.pgn", displayName: "b.pgn" })],
        recursive: false,
        directoriesScanned: 1,
        truncated: false,
        truncationNotes: [],
      },
    });
    const user = userEvent.setup();
    const onFilesChosen = vi.fn();
    render(<AddFolderPanel onFilesChosen={onFilesChosen} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));
    await user.click(await screen.findByRole("button", { name: "Add 2 Files" }));

    expect(onFilesChosen).toHaveBeenCalledWith(["C:\\collection\\a.pgn", "C:\\collection\\b.pgn"]);
    // Panel resets back to its idle state after adding.
    expect(screen.queryByText(/Found/)).not.toBeInTheDocument();
  });

  it("Cancel dismisses the review without adding anything", async () => {
    const user = userEvent.setup();
    const onFilesChosen = vi.fn();
    render(<AddFolderPanel onFilesChosen={onFilesChosen} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));
    await screen.findByText("Found 1 file in this folder.");

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onFilesChosen).not.toHaveBeenCalled();
    expect(screen.queryByText(/Found/)).not.toBeInTheDocument();
  });

  it("shows an empty-result message and disables Add when nothing matched", async () => {
    scanInputDirectory.mockResolvedValueOnce({
      status: "ok",
      data: { files: [], recursive: false, directoriesScanned: 1, truncated: false, truncationNotes: [] },
    });
    const user = userEvent.setup();
    render(<AddFolderPanel onFilesChosen={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));

    expect(await screen.findByText("No matching files were found.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Add \d/ })).not.toBeInTheDocument();
  });

  it("shows a dismissible error banner when the scan itself fails", async () => {
    scanInputDirectory.mockResolvedValueOnce({
      status: "error",
      error: { code: "INPUT_NOT_READABLE", message: "The folder could not be read." },
    });
    const user = userEvent.setup();
    render(<AddFolderPanel onFilesChosen={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Add Folder" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The folder could not be read.");
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
