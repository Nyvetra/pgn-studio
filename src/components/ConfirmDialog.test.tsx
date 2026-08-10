// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

function setup(open: boolean, onConfirm = vi.fn(), onCancel = vi.fn()) {
  return render(
    <ConfirmDialog
      open={open}
      title="Replace existing files?"
      description="This will replace files already at the destination."
      confirmLabel="Replace"
      onConfirm={onConfirm}
      onCancel={onCancel}
    />,
  );
}

describe("ConfirmDialog", () => {
  it("is not exposed to the accessibility tree while closed", () => {
    setup(false);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("becomes an accessible modal dialog when opened, labelled by its title", () => {
    setup(true);
    const dialog = screen.getByRole("dialog", { name: "Replace existing files?" });
    expect(dialog).toBeInTheDocument();
  });

  it("focuses the Cancel button by default (safe action first)", () => {
    setup(true);
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
  });

  it("calls onConfirm when the confirm action is activated", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    setup(true, onConfirm);
    await user.click(screen.getByRole("button", { name: "Replace" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when Cancel is activated via keyboard", async () => {
    const onCancel = vi.fn();
    const user = userEvent.setup();
    setup(true, vi.fn(), onCancel);
    await user.keyboard("{Enter}");
    expect(onCancel).toHaveBeenCalled();
  });
});
