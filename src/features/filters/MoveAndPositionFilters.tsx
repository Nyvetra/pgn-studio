// SPDX-License-Identifier: GPL-3.0-or-later
/** Move-count bounds, checkmates-only, and starting-position policy
 * (architecture.md §13.4). */
import type { FilterDraft } from "../../types/workflow";
import type { SetupPolicy } from "../../ipc/client";
import { NumberField } from "../../components/NumberField";
import { Checkbox } from "../../components/Checkbox";
import { RadioGroup, type RadioOption } from "../../components/RadioGroup";

export interface MoveAndPositionFiltersProps {
  filters: FilterDraft;
  problems: { moveMin?: string; moveMax?: string };
  onChange: (patch: Partial<FilterDraft>) => void;
}

const SETUP_OPTIONS: readonly RadioOption<SetupPolicy>[] = [
  {
    value: "any",
    label: "Any starting position",
    help: "Includes games that start from the standard position and games that start from a custom SetUp/FEN position.",
  },
  {
    value: "standardStartOnly",
    label: "Standard starting position only",
    help: "Excludes any game that begins from a custom position.",
  },
  {
    value: "setupOnly",
    label: "Custom starting position only (SetUp/FEN)",
    help: "Includes only games that begin from a custom position.",
  },
];

export function MoveAndPositionFilters({ filters, problems, onChange }: MoveAndPositionFiltersProps) {
  return (
    <section aria-labelledby="filters-moves-heading">
      <h3 id="filters-moves-heading">Moves &amp; position</h3>
      <div className="field-row">
        <NumberField
          label="Minimum moves"
          value={filters.moveMin}
          onValueChange={(moveMin) => onChange({ moveMin })}
          error={problems.moveMin}
          min={1}
          max={4999}
        />
        <NumberField
          label="Maximum moves"
          value={filters.moveMax}
          onValueChange={(moveMax) => onChange({ moveMax })}
          error={problems.moveMax}
          min={1}
          max={4999}
        />
      </div>
      <Checkbox
        label="Checkmates only"
        help="Keeps only games that end in checkmate."
        checked={filters.checkmateOnly}
        onCheckedChange={(checkmateOnly) => onChange({ checkmateOnly })}
      />
      <RadioGroup
        legend="Starting position"
        options={SETUP_OPTIONS}
        value={filters.setupPolicy}
        onValueChange={(setupPolicy) => onChange({ setupPolicy })}
      />
    </section>
  );
}
