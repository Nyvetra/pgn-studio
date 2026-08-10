// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Metric, MetricList } from "./Metric";
import { formatCount, NOT_AVAILABLE } from "../state/formatters";

describe("Metric", () => {
  it("renders a known value as-is", () => {
    render(
      <MetricList>
        <Metric label="Output games" value={formatCount(1234)} />
      </MetricList>,
    );
    expect(screen.getByText("Output games")).toBeInTheDocument();
    expect(screen.getByText("1,234")).toBeInTheDocument();
  });

  it('renders an unmeasured metric as "Not available", never as 0 (§9.3, §25)', () => {
    render(
      <MetricList>
        <Metric label="Broken games" value={formatCount(null)} />
      </MetricList>,
    );
    expect(screen.getByText(NOT_AVAILABLE)).toBeInTheDocument();
    expect(screen.queryByText("0")).not.toBeInTheDocument();
  });

  it("renders a genuine zero distinctly from the unknown state", () => {
    render(
      <MetricList>
        <Metric label="Duplicate games" value={formatCount(0)} />
      </MetricList>,
    );
    expect(screen.getByText("0")).toBeInTheDocument();
  });

  it("uses <dt>/<dd> pairing inside a <dl> so the label/value relationship is programmatically exposed", () => {
    const { container } = render(
      <MetricList>
        <Metric label="Input games" value="10" />
      </MetricList>,
    );
    expect(container.querySelector("dl")).not.toBeNull();
    expect(container.querySelector("dt")?.textContent).toBe("Input games");
    expect(container.querySelector("dd")?.textContent).toBe("10");
  });
});
