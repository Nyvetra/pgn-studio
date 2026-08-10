// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Stepper } from "./Stepper";

describe("Stepper", () => {
  it("marks the current step with aria-current='step'", () => {
    render(<Stepper current="operations" farthestStepIndex={2} onSelect={vi.fn()} />);
    const current = screen.getByRole("button", { name: /Operations/ });
    expect(current).toHaveAttribute("aria-current", "step");
    expect(screen.getByRole("button", { name: /Files/ })).not.toHaveAttribute("aria-current");
  });

  it("disables steps beyond the farthest one reached, without removing them from the DOM", () => {
    render(<Stepper current="files" farthestStepIndex={0} onSelect={vi.fn()} />);
    expect(screen.getByRole("button", { name: /Review/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Files/ })).toBeEnabled();
  });

  it("clicking a reachable, earlier step calls onSelect with that step", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    render(<Stepper current="review" farthestStepIndex={3} onSelect={onSelect} />);
    await user.click(screen.getByRole("button", { name: /Files/ }));
    expect(onSelect).toHaveBeenCalledWith("files");
  });

  it("is fully keyboard-reachable in document order via Tab, skipping nothing but disabled steps", async () => {
    const user = userEvent.setup();
    render(<Stepper current="files" farthestStepIndex={1} onSelect={vi.fn()} />);
    await user.tab();
    expect(screen.getByRole("button", { name: /Files/ })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: /Operations/ })).toHaveFocus();
    // The next three steps are disabled (farthestStepIndex is 1); a
    // disabled native <button> is unreachable by Tab, so focus should not
    // land on any of them.
    await user.tab();
    expect(screen.getByRole("button", { name: /Filters/ })).not.toHaveFocus();
    expect(screen.getByRole("button", { name: /Review/ })).not.toHaveFocus();
  });

  it("renders all five workflow steps in order", () => {
    render(<Stepper current="files" farthestStepIndex={4} onSelect={vi.fn()} />);
    const labels = screen.getAllByRole("button").map((b) => b.textContent);
    expect(labels).toEqual([
      expect.stringContaining("Files"),
      expect.stringContaining("Operations"),
      expect.stringContaining("Filters"),
      expect.stringContaining("Review"),
      expect.stringContaining("Run & Results"),
    ]);
  });
});
