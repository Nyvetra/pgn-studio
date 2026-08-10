// SPDX-License-Identifier: GPL-3.0-or-later
import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { TextField } from "./TextField";
import { NumberField } from "./NumberField";

/** A minimal controlled wrapper — `TextField` itself takes `value` from its
 * caller and never mutates it, so exercising realistic typing (where each
 * keystroke builds on the last) needs something to feed `onValueChange`
 * back in, exactly like every real caller in this app (`useWorkflow`'s
 * dispatch/state cycle) already does. */
function ControlledTextField(props: { label: string; onValueChange: (value: string) => void }) {
  const [value, setValue] = useState("");
  return (
    <TextField
      label={props.label}
      value={value}
      onValueChange={(next) => {
        setValue(next);
        props.onValueChange(next);
      }}
    />
  );
}

describe("TextField", () => {
  it("associates the visible label with the input (semantic labels, §13.8)", () => {
    render(<TextField label="Base filename" value="" onValueChange={vi.fn()} />);
    expect(screen.getByLabelText("Base filename")).toBeInTheDocument();
  });

  it("wires help text through aria-describedby, not just visually", () => {
    render(<TextField label="Player" help="Matches by starts with." value="" onValueChange={vi.fn()} />);
    const input = screen.getByLabelText("Player");
    const describedBy = input.getAttribute("aria-describedby");
    expect(describedBy).toBeTruthy();
    expect(document.getElementById(describedBy!)).toHaveTextContent("Matches by starts with.");
  });

  it("marks an invalid field with aria-invalid and exposes the error as role=alert, still reachable via aria-describedby", () => {
    render(<TextField label="From year" error="must be a four-digit year" value="abcd" onValueChange={vi.fn()} />);
    const input = screen.getByLabelText("From year");
    expect(input).toHaveAttribute("aria-invalid", "true");
    const error = screen.getByRole("alert");
    expect(error).toHaveTextContent("must be a four-digit year");
    expect(input.getAttribute("aria-describedby")).toContain(error.id);
  });

  it("has no aria-invalid when there is no error", () => {
    render(<TextField label="Player" value="Tal" onValueChange={vi.fn()} />);
    expect(screen.getByLabelText("Player")).not.toHaveAttribute("aria-invalid");
  });

  it("is reachable by Tab and calls onValueChange as the user types", async () => {
    const onValueChange = vi.fn();
    const user = userEvent.setup();
    render(<ControlledTextField label="Player" onValueChange={onValueChange} />);
    await user.tab();
    expect(screen.getByLabelText("Player")).toHaveFocus();
    await user.keyboard("Tal");
    expect(onValueChange).toHaveBeenCalledWith("T");
    expect(onValueChange).toHaveBeenCalledWith("Ta");
    expect(onValueChange).toHaveBeenCalledWith("Tal");
    expect(screen.getByLabelText("Player")).toHaveValue("Tal");
  });

  it("announces a required field to assistive technology, not just visually", () => {
    render(<TextField label="Base filename" value="" onValueChange={vi.fn()} required />);
    const input = screen.getByLabelText(/Base filename/);
    expect(input).toHaveAttribute("aria-required", "true");
  });
});

describe("NumberField", () => {
  it("is a semantic number input associated with its label", () => {
    render(<NumberField label="Minimum Elo" value="" onValueChange={vi.fn()} />);
    const input = screen.getByLabelText("Minimum Elo");
    expect(input).toHaveAttribute("type", "number");
  });

  it("shows a validation error via role=alert and aria-invalid", () => {
    render(<NumberField label="Maximum Elo" value="9999" error="must be between 0 and 4000" onValueChange={vi.fn()} />);
    expect(screen.getByRole("alert")).toHaveTextContent("must be between 0 and 4000");
    expect(screen.getByLabelText("Maximum Elo")).toHaveAttribute("aria-invalid", "true");
  });

  it("stays a plain string value, so an empty field is not coerced to 0", () => {
    const onValueChange = vi.fn();
    render(<NumberField label="Minimum moves" value="" onValueChange={onValueChange} />);
    expect(screen.getByLabelText("Minimum moves")).toHaveValue(null);
  });
});
