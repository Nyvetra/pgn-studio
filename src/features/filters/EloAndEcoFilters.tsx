// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Elo range and ECO code/range filters (architecture.md §13.4).
 *
 * ECO wording constraint (D-010, empirically verified): only prefix
 * ("starts with") and "is not" (`<>`) are reliable operators for ECO on
 * this engine — `=`, `>`, `>=`, `<`, `<=` silently match nothing. This
 * screen therefore never offers those five operators; a "range" is
 * expressed by listing one or more codes/prefixes (e.g. "B" for every
 * B-opening), not by a relational bound.
 */
import { useState } from "react";
import type { EloScope, EcoEntry, FilterDraft } from "../../types/workflow";
import { NumberField } from "../../components/NumberField";
import { TextField } from "../../components/TextField";
import { SelectField, type SelectOption } from "../../components/Select";
import { Checkbox } from "../../components/Checkbox";
import { Button } from "../../components/Button";
import "./EloAndEcoFilters.css";

export interface EloAndEcoFiltersProps {
  filters: FilterDraft;
  problems: { eloMin?: string; eloMax?: string };
  onChange: (patch: Partial<FilterDraft>) => void;
  onAddEcoEntry: (value: string) => void;
  onUpdateEcoEntry: (id: string, patch: Partial<EcoEntry>) => void;
  onRemoveEcoEntry: (id: string) => void;
}

const ELO_SCOPE_OPTIONS: readonly SelectOption<EloScope>[] = [
  { value: "either", label: "Either player" },
  { value: "white", label: "White player" },
  { value: "black", label: "Black player" },
];

export function EloAndEcoFilters({
  filters,
  problems,
  onChange,
  onAddEcoEntry,
  onUpdateEcoEntry,
  onRemoveEcoEntry,
}: EloAndEcoFiltersProps) {
  const [ecoDraft, setEcoDraft] = useState("");

  return (
    <section aria-labelledby="filters-elo-eco-heading">
      <h3 id="filters-elo-eco-heading">Rating &amp; opening</h3>
      <p className="workflow-screen__section-help">
        A game without the relevant tag (e.g. no Elo rating, or no ECO code) is excluded by that
        filter, since there is nothing for it to match against.
      </p>

      <div className="field-row filters-elo-row">
        <SelectField
          label="Elo applies to"
          value={filters.eloScope}
          onValueChange={(eloScope) => onChange({ eloScope })}
          options={ELO_SCOPE_OPTIONS}
        />
        <NumberField
          label="Minimum Elo"
          value={filters.eloMin}
          onValueChange={(eloMin) => onChange({ eloMin })}
          error={problems.eloMin}
          min={0}
          max={4000}
        />
        <NumberField
          label="Maximum Elo"
          value={filters.eloMax}
          onValueChange={(eloMax) => onChange({ eloMax })}
          error={problems.eloMax}
          min={0}
          max={4000}
        />
      </div>

      <h3>ECO opening code</h3>
      <p className="workflow-screen__section-help">
        Enter a full code (e.g. "B10") or just a prefix (e.g. "B" for every B-opening) — codes match
        by "starts with". To exclude a code instead, check "exclude". This engine cannot compare ECO
        codes as a numeric/relational range, so a range is expressed as one or more codes/prefixes.
      </p>
      <div className="filters-eco-add">
        <TextField
          label="Add an ECO code or prefix"
          value={ecoDraft}
          onValueChange={setEcoDraft}
          placeholder="e.g. B10 or B"
        />
        <Button
          type="button"
          variant="secondary"
          onClick={() => {
            const value = ecoDraft.trim();
            if (!value) return;
            onAddEcoEntry(value);
            setEcoDraft("");
          }}
        >
          Add
        </Button>
      </div>
      {filters.ecoEntries.length > 0 && (
        <ul className="filters-eco-list" aria-label="ECO codes in this filter">
          {filters.ecoEntries.map((entry) => (
            <EcoEntryRow
              key={entry.id}
              entry={entry}
              onUpdate={(patch) => onUpdateEcoEntry(entry.id, patch)}
              onRemove={() => onRemoveEcoEntry(entry.id)}
            />
          ))}
        </ul>
      )}
    </section>
  );
}

function EcoEntryRow({
  entry,
  onUpdate,
  onRemove,
}: {
  entry: EcoEntry;
  onUpdate: (patch: Partial<EcoEntry>) => void;
  onRemove: () => void;
}) {
  return (
    <li className="filters-eco-row">
      <TextField
        label={`ECO code`}
        labelHidden
        value={entry.value}
        onValueChange={(value) => onUpdate({ value })}
      />
      <Checkbox label="Exclude" checked={entry.exclude} onCheckedChange={(exclude) => onUpdate({ exclude })} />
      <Button type="button" variant="ghost" onClick={onRemove} aria-label={`Remove ECO filter ${entry.value || "(blank)"}`}>
        Remove
      </Button>
    </li>
  );
}
