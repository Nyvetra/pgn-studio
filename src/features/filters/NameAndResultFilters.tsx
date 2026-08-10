// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Player/White/Black, result, and date/year range (architecture.md §13.4).
 *
 * Wording constraint (task spec, binding): name matching is "starts with",
 * never "equals" (design-02 §1.5.1 — `strncmp`-based prefix matching means
 * "Tal" also matches "Talbot").
 */
import type { FilterDraft } from "../../types/workflow";
import { TextField } from "../../components/TextField";
import { NumberField } from "../../components/NumberField";
import { Checkbox } from "../../components/Checkbox";

export interface NameAndResultFiltersProps {
  filters: FilterDraft;
  problems: { dateFromYear?: string; dateToYear?: string };
  onChange: (patch: Partial<FilterDraft>) => void;
}

export function NameAndResultFilters({ filters, problems, onChange }: NameAndResultFiltersProps) {
  return (
    <section aria-labelledby="filters-names-heading">
      <h3 id="filters-names-heading">Players</h3>
      <div className="field-row">
        <TextField
          label="Player (either color)"
          help='Matches by "starts with", not exact equality — e.g. "Tal" also matches "Talbot". Matches either the White or Black player.'
          value={filters.player}
          onValueChange={(player) => onChange({ player })}
        />
        <TextField
          label="White player"
          help='Matches by "starts with" against the White player only.'
          value={filters.white}
          onValueChange={(white) => onChange({ white })}
        />
        <TextField
          label="Black player"
          help='Matches by "starts with" against the Black player only.'
          value={filters.black}
          onValueChange={(black) => onChange({ black })}
        />
      </div>

      <h3>Result</h3>
      <fieldset className="field-group">
        <legend>Result</legend>
        <Checkbox
          label="White wins (1-0)"
          checked={filters.resultWhiteWin}
          onCheckedChange={(resultWhiteWin) => onChange({ resultWhiteWin })}
        />
        <Checkbox
          label="Black wins (0-1)"
          checked={filters.resultBlackWin}
          onCheckedChange={(resultBlackWin) => onChange({ resultBlackWin })}
        />
        <Checkbox
          label="Draw (1/2-1/2)"
          checked={filters.resultDraw}
          onCheckedChange={(resultDraw) => onChange({ resultDraw })}
        />
        <Checkbox
          label="Other / unfinished (*)"
          checked={filters.resultOther}
          onCheckedChange={(resultOther) => onChange({ resultOther })}
        />
        <Checkbox
          label="Decisive games only"
          help="Shorthand for White wins or Black wins, combined with anything already checked above."
          checked={filters.decisiveOnly}
          onCheckedChange={(decisiveOnly) => onChange({ decisiveOnly })}
        />
      </fieldset>

      <h3>Date</h3>
      <div className="field-row">
        <NumberField
          label="From year"
          value={filters.dateFromYear}
          onValueChange={(dateFromYear) => onChange({ dateFromYear })}
          error={problems.dateFromYear}
          min={0}
          max={9999}
        />
        <NumberField
          label="To year"
          value={filters.dateToYear}
          onValueChange={(dateToYear) => onChange({ dateToYear })}
          error={problems.dateToYear}
          min={0}
          max={9999}
        />
      </div>
    </section>
  );
}
