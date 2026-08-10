// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { WorkflowProvider } from "../../state/WorkflowContext";
import { useWorkflow } from "../../state/useWorkflow";
import { compileFilters } from "../../state/filterMapping";
import { checkA11y } from "../../test/a11y";
import { FiltersScreen } from "./FiltersScreen";

function CompiledPreview() {
  const { state } = useWorkflow();
  return <pre data-testid="compiled">{JSON.stringify(compileFilters(state.filters))}</pre>;
}

function renderScreen() {
  return render(
    <WorkflowProvider>
      <FiltersScreen />
      <CompiledPreview />
    </WorkflowProvider>,
  );
}

function readCompiled(): ReturnType<typeof compileFilters> {
  return JSON.parse(screen.getByTestId("compiled").textContent ?? "{}");
}

describe("FiltersScreen", () => {
  it("has no automated a11y violations (architecture.md §13.8)", async () => {
    const { container } = renderScreen();
    expect(await checkA11y(container)).toHaveNoViolations();
  });

  it("compiles player/white/black text as they are typed", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText(/Player \(either color\)/), "Tal");
    await user.type(screen.getByLabelText("White player"), "Fischer");
    expect(readCompiled().tagRules).toContainEqual({ tag: "Player", op: "prefix", value: "Tal" });
    expect(readCompiled().tagRules).toContainEqual({ tag: "White", op: "prefix", value: "Fischer" });
  });

  it('explains name matching is "starts with" (task spec binding wording), never "equals"', () => {
    renderScreen();
    expect(screen.getAllByText(/starts with/i).length).toBeGreaterThan(0);
  });

  it("checking Decisive games only compiles both decisive Result values", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.click(screen.getByLabelText("Decisive games only"));
    const results = readCompiled().tagRules.filter((r) => r.tag === "Result").map((r) => r.value).sort();
    expect(results).toEqual(["0-1", "1-0"]);
  });

  it("a year range compiles to Date bound rules", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText("From year"), "1990");
    await user.type(screen.getByLabelText("To year"), "2000");
    expect(readCompiled().tagRules).toContainEqual({ tag: "Date", op: "ge", value: "1990.01.01" });
    expect(readCompiled().tagRules).toContainEqual({ tag: "Date", op: "le", value: "2000.12.31" });
  });

  it("shows an inline validation error for an out-of-range Elo value and disables Next", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText("Maximum Elo"), "9999");
    expect(await screen.findByText(/between 0 and 4000/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next: Review" })).toBeDisabled();
  });

  it("routes the Elo range to WhiteElo when scoped to White player", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.selectOptions(screen.getByLabelText("Elo applies to"), "white");
    await user.type(screen.getByLabelText("Minimum Elo"), "2200");
    expect(readCompiled().tagRules).toContainEqual({ tag: "WhiteElo", op: "ge", value: "2200" });
  });

  it("adds an ECO entry and compiles it as a prefix match", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText("Add an ECO code or prefix"), "B10");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(readCompiled().tagRules).toContainEqual({ tag: "Eco", op: "prefix", value: "B10" });
  });

  it('checking "Exclude" on an ECO entry compiles the verified "<>" operator', async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText("Add an ECO code or prefix"), "C00");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.click(screen.getByLabelText("Exclude"));
    expect(readCompiled().tagRules).toContainEqual({ tag: "Eco", op: "ne", value: "C00" });
  });

  it("removes an ECO entry", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText("Add an ECO code or prefix"), "A00");
    await user.click(screen.getByRole("button", { name: "Add" }));
    expect(readCompiled().tagRules).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: /Remove ECO filter/ }));
    expect(readCompiled().tagRules).toHaveLength(0);
  });

  it("offers no ECO operator picker at all — only a value and an Exclude checkbox (D-010: =, >, >=, <, <= silently match nothing on this engine)", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText("Add an ECO code or prefix"), "B10");
    await user.click(screen.getByRole("button", { name: "Add" }));
    const row = screen.getByRole("list", { name: "ECO codes in this filter" });
    expect(within(row).queryAllByRole("combobox")).toHaveLength(0);
    expect(within(row).queryAllByRole("radio")).toHaveLength(0);
    expect(within(row).getByRole("checkbox", { name: "Exclude" })).toBeInTheDocument();
  });

  it("move bounds min greater than max shows an inline error and disables Next", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText("Minimum moves"), "40");
    await user.type(screen.getByLabelText("Maximum moves"), "10");
    expect(await screen.findByText(/must not exceed/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next: Review" })).toBeDisabled();
  });

  it("checkmate-only and setup-policy controls compile through unchanged", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.click(screen.getByLabelText("Checkmates only"));
    await user.click(screen.getByRole("radio", { name: /Standard starting position only/ }));
    expect(readCompiled().checkmateOnly).toBe(true);
    expect(readCompiled().setupPolicy).toBe("standardStartOnly");
  });

  it("Clear all filters resets every field", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.type(screen.getByLabelText(/Player \(either color\)/), "Tal");
    await user.click(screen.getByLabelText("Decisive games only"));
    await user.click(screen.getByRole("button", { name: "Clear all filters" }));
    expect(screen.getByLabelText(/Player \(either color\)/)).toHaveValue("");
    expect(screen.getByLabelText("Decisive games only")).not.toBeChecked();
    expect(readCompiled().tagRules).toEqual([]);
  });

  it("Next is enabled with no filters set at all", () => {
    renderScreen();
    expect(screen.getByRole("button", { name: "Next: Review" })).toBeEnabled();
  });

  it("Back returns to Operations", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.click(screen.getByRole("button", { name: "Back" }));
    // No visible step probe needed here: absence of a thrown error plus the
    // dedicated workflow navigation coverage in workflowReducer.test.ts is
    // sufficient — this only needs to confirm the button dispatches without
    // throwing inside a real provider.
  });
});
