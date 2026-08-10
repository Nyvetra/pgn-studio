// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SourceList } from "./SourceList";
import type { DraftInput } from "../../types/workflow";

function input(overrides: Partial<DraftInput> = {}): DraftInput {
  return {
    id: "1",
    path: "C:\\games\\a.pgn",
    displayName: "a.pgn",
    sizeBytes: null,
    isReadable: null,
    extensionOk: null,
    warnings: [],
    inspected: false,
    ...overrides,
  };
}

describe("SourceList", () => {
  it("shows an empty-state message when there are no inputs", () => {
    render(<SourceList inputs={[]} onMove={vi.fn()} onRemove={vi.fn()} />);
    expect(screen.getByText(/No files added yet/)).toBeInTheDocument();
  });

  it("shows a checking state before inspection completes, then the formatted size", () => {
    const { rerender } = render(
      <SourceList inputs={[input({ inspected: false })]} onMove={vi.fn()} onRemove={vi.fn()} />,
    );
    expect(screen.getByText("Checking…")).toBeInTheDocument();

    rerender(
      <SourceList
        inputs={[input({ inspected: true, sizeBytes: 2048 })]}
        onMove={vi.fn()}
        onRemove={vi.fn()}
      />,
    );
    expect(screen.getByText("2 KB")).toBeInTheDocument();
  });

  it("renders per-file warnings", () => {
    render(
      <SourceList
        inputs={[input({ inspected: true, warnings: ["file does not have a .pgn extension"] })]}
        onMove={vi.fn()}
        onRemove={vi.fn()}
      />,
    );
    expect(screen.getByText(/does not have a \.pgn extension/)).toBeInTheDocument();
  });

  it("disables Move up on the first row and Move down on the last row", () => {
    render(
      <SourceList
        inputs={[input({ id: "1", displayName: "a.pgn" }), input({ id: "2", displayName: "b.pgn" })]}
        onMove={vi.fn()}
        onRemove={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Move a.pgn up" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move a.pgn down" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Move b.pgn down" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Move b.pgn up" })).toBeEnabled();
  });

  it("calls onMove/onRemove with the row's id", async () => {
    const onMove = vi.fn();
    const onRemove = vi.fn();
    const user = userEvent.setup();
    render(
      <SourceList
        inputs={[input({ id: "1", displayName: "a.pgn" }), input({ id: "2", displayName: "b.pgn" })]}
        onMove={onMove}
        onRemove={onRemove}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Move b.pgn up" }));
    expect(onMove).toHaveBeenCalledWith("2", "up");

    await user.click(screen.getByRole("button", { name: "Remove a.pgn from this job" }));
    expect(onRemove).toHaveBeenCalledWith("1");
  });

  it("numbers rows in list order (1-based, matching retention priority)", () => {
    const { container } = render(
      <SourceList
        inputs={[input({ id: "1", displayName: "a.pgn" }), input({ id: "2", displayName: "b.pgn" })]}
        onMove={vi.fn()}
        onRemove={vi.fn()}
      />,
    );
    const priorities = Array.from(container.querySelectorAll(".source-row__priority")).map(
      (el) => el.textContent,
    );
    expect(priorities).toEqual(["1", "2"]);
  });
});
