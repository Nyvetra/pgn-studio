// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Filters screen (architecture.md §13.4). Every control here writes typed
 * `FilterDraft` fields; `state/filterMapping.ts#compileFilters` is the only
 * place that turns them into `TagRule[]`/`MoveBounds` — React never
 * composes criteria-file syntax.
 */
import { useWorkflow } from "../../state/useWorkflow";
import { validateFilterDraft } from "../../state/filterMapping";
import { defaultFilterDraft } from "../../state/defaults";
import { StepNav } from "../../components/StepNav";
import { Button } from "../../components/Button";
import { useFocusOnMount } from "../../components/useFocusOnMount";
import "../../components/workflow-screen.css";
import { NameAndResultFilters } from "./NameAndResultFilters";
import { EloAndEcoFilters } from "./EloAndEcoFilters";
import { MoveAndPositionFilters } from "./MoveAndPositionFilters";

export function FiltersScreen() {
  const { state, dispatch } = useWorkflow();
  const problems = validateFilterDraft(state.filters);
  const hasProblems = Object.keys(problems).length > 0;
  const headingRef = useFocusOnMount<HTMLHeadingElement>();

  return (
    <section className="workflow-screen" aria-labelledby="filters-heading">
      <h2 id="filters-heading" ref={headingRef} tabIndex={-1}>
        Filters
      </h2>
      <p className="workflow-screen__intro">
        Every filter below is optional. Leave a field blank to not filter on it. A game that is
        missing the relevant tag entirely (for example, no Elo rating recorded) is excluded by any
        filter on that tag, since there is nothing to compare.
      </p>

      <NameAndResultFilters filters={state.filters} problems={problems} onChange={(patch) => dispatch({ type: "SET_FILTERS", patch })} />

      <EloAndEcoFilters
        filters={state.filters}
        problems={problems}
        onChange={(patch) => dispatch({ type: "SET_FILTERS", patch })}
        onAddEcoEntry={(value) => dispatch({ type: "ADD_ECO_ENTRY", value })}
        onUpdateEcoEntry={(id, patch) => dispatch({ type: "UPDATE_ECO_ENTRY", id, patch })}
        onRemoveEcoEntry={(id) => dispatch({ type: "REMOVE_ECO_ENTRY", id })}
      />

      <MoveAndPositionFilters
        filters={state.filters}
        problems={problems}
        onChange={(patch) => dispatch({ type: "SET_FILTERS", patch })}
      />

      <p>
        <Button variant="ghost" onClick={() => dispatch({ type: "SET_FILTERS", patch: defaultFilterDraft() })}>
          Clear all filters
        </Button>
      </p>

      <StepNav
        onBack={() => dispatch({ type: "GO_TO_STEP", step: "operations" })}
        onNext={() => dispatch({ type: "GO_NEXT" })}
        nextDisabled={hasProblems}
        nextLabel="Next: Review"
      />
    </section>
  );
}
