// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Checkbox } from "./Checkbox";
import { RadioGroup } from "./RadioGroup";

describe("Checkbox", () => {
  it("is a semantic checkbox associated with its visible label", () => {
    render(<Checkbox label="Remove comments" checked={false} onCheckedChange={vi.fn()} />);
    expect(screen.getByRole("checkbox", { name: "Remove comments" })).toBeInTheDocument();
  });

  it("toggles via mouse click", async () => {
    const onCheckedChange = vi.fn();
    const user = userEvent.setup();
    render(<Checkbox label="Remove comments" checked={false} onCheckedChange={onCheckedChange} />);
    await user.click(screen.getByRole("checkbox", { name: "Remove comments" }));
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it("is reachable by Tab and toggles via the keyboard (Space), matching native checkbox semantics", async () => {
    const onCheckedChange = vi.fn();
    const user = userEvent.setup();
    render(<Checkbox label="Remove comments" checked={false} onCheckedChange={onCheckedChange} />);
    const checkbox = screen.getByRole("checkbox", { name: "Remove comments" });

    await user.tab();
    expect(checkbox).toHaveFocus();
    await user.keyboard(" ");
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });

  it("disabled checkboxes are excluded from the tab order", async () => {
    const user = userEvent.setup();
    render(<Checkbox label="Add ECO tags" checked={false} onCheckedChange={vi.fn()} disabled />);
    await user.tab();
    expect(screen.getByRole("checkbox", { name: "Add ECO tags" })).not.toHaveFocus();
  });

  it("wires help text through aria-describedby", () => {
    render(
      <Checkbox
        label="Always create the audit file, even if it would be empty"
        help="Only meaningful once the audit file is being kept."
        checked={false}
        onCheckedChange={vi.fn()}
      />,
    );
    const checkbox = screen.getByRole("checkbox");
    const describedBy = checkbox.getAttribute("aria-describedby");
    expect(describedBy).toBeTruthy();
    expect(document.getElementById(describedBy!)).toHaveTextContent(/Only meaningful/);
  });
});

describe("RadioGroup", () => {
  const options = [
    { value: "any", label: "Any starting position" },
    { value: "standardStartOnly", label: "Standard starting position only" },
  ] as const;

  it("groups options under one legend, exposed as a fieldset/legend pair", () => {
    render(<RadioGroup legend="Starting position" options={options} value="any" onValueChange={vi.fn()} />);
    expect(screen.getByRole("group", { name: "Starting position" })).toBeInTheDocument();
  });

  it("only one option is checked at a time, and arrow-key navigation moves between them (native radio group behavior)", async () => {
    const onValueChange = vi.fn();
    const user = userEvent.setup();
    render(<RadioGroup legend="Starting position" options={options} value="any" onValueChange={onValueChange} />);

    const first = screen.getByRole("radio", { name: "Any starting position" });
    const second = screen.getByRole("radio", { name: "Standard starting position only" });
    expect(first).toBeChecked();
    expect(second).not.toBeChecked();

    first.focus();
    await user.keyboard("{ArrowDown}");
    expect(onValueChange).toHaveBeenCalledWith("standardStartOnly");
  });

  it("marks a disabled option as unselectable without removing it from view", () => {
    const withDisabled = [...options, { value: "setupOnly", label: "Custom only", disabled: true }] as const;
    render(<RadioGroup legend="Starting position" options={withDisabled} value="any" onValueChange={vi.fn()} />);
    expect(screen.getByRole("radio", { name: "Custom only" })).toBeDisabled();
  });
});
