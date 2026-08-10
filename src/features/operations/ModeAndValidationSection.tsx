// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * "Merge" + "Validation and error policy" (architecture.md §13.3). The
 * architecture document's illustrative domain model has separate
 * `merge`/`validate` booleans, but the real `OperationPlan` (design-02)
 * replaces both with one `JobMode` — merging is simply what `"process"`
 * mode always does, so there is no separate merge toggle to expose.
 *
 * Wording constraint (task spec, binding): there is no separate
 * broken-games file — the engine cannot produce one in a single pass
 * (D-007 V-5). Only "discard" and "keep in main output" are offered, and
 * both say plainly that games with errors are reported in the log, never a
 * file of their own.
 *
 * Phase 4 corrections (verified against the real pinned engine,
 * `phase4_integration.rs`):
 *  - "Keep games with errors in the main output" no longer claims "nothing
 *    is silently dropped". `--keepbroken` reliably rescues games with an
 *    unplayable/illegal move, but does NOT rescue every malformation — a
 *    game missing its result marker entirely can still be silently dropped
 *    from (or have its move list silently stripped from) the output with
 *    `--keepbroken` on, identically to how it behaves with it off
 *    (`missing_result_marker_survives_keepbroken_identically_to_default` in
 *    `phase4_integration.rs`). Overclaiming "nothing is dropped" here would
 *    violate architecture.md §4.3.
 *  - "Exclude games with inconsistent result tags" (`--nobadresults`) no
 *    longer attributes its check to "the final position's outcome"
 *    (checkmate/stalemate). Reading `apply.c` and reproducing both cases
 *    empirically shows `--nobadresults` only ever rejects a game whose
 *    *movetext's own trailing result token* (e.g. the score's final
 *    "1-0"/"1/2-1/2") textually disagrees with its `Result` tag — a
 *    Result-tag-vs-actual-checkmate mismatch (what `--fixresulttags` repairs)
 *    is only ever warned about, never rejected by `--nobadresults`
 *    (`nobadresults_does_not_exclude_a_checkmate_vs_tag_mismatch` in
 *    `phase4_integration.rs`).
 */
import type { EngineCapabilities, JobMode } from "../../ipc/client";
import type { CleanupOptions, BrokenOutput } from "../../ipc/client";
import { RadioGroup, type RadioOption } from "../../components/RadioGroup";
import { Checkbox } from "../../components/Checkbox";
import { capabilityDisabledReason } from "./capabilityHelp";

const MODE_OPTIONS: readonly RadioOption<JobMode>[] = [
  {
    value: "process",
    label: "Process the files",
    help: "Merge every source in the order set on the Files screen, apply the options below, and write output games.",
  },
  {
    value: "validateOnly",
    label: "Validate only",
    help: "Check every source file for errors and produce a report. No merged/transformed games file is written.",
  },
];

const BROKEN_OPTIONS: readonly RadioOption<BrokenOutput>[] = [
  {
    value: "discard",
    label: "Discard games with errors",
    help: "Games pgn-extract cannot parse cleanly are left out of the output. They are still reported in the log — there is no separate file for them.",
  },
  {
    value: "keepInMainOutput",
    label: "Keep games with errors in the main output",
    help: "Most games pgn-extract cannot parse cleanly (for example one with an illegal move) are still included in the same output file as valid ones instead of being left out. A few kinds of error can still result in a game being left out, or having its moves stripped, even with this on — check the log to see which games had problems, since there is still no separate file for them.",
  },
];

export interface ModeAndValidationSectionProps {
  mode: JobMode;
  broken: BrokenOutput;
  cleanup: CleanupOptions;
  capabilities: EngineCapabilities | null;
  onModeChange: (mode: JobMode) => void;
  onBrokenChange: (value: BrokenOutput) => void;
  onCleanupChange: (patch: Partial<CleanupOptions>) => void;
}

export function ModeAndValidationSection({
  mode,
  broken,
  cleanup,
  capabilities,
  onModeChange,
  onBrokenChange,
  onCleanupChange,
}: ModeAndValidationSectionProps) {
  const loaded = capabilities !== null;
  return (
    <section aria-labelledby="operations-mode-heading">
      <h3 id="operations-mode-heading">Merge &amp; validation</h3>
      <RadioGroup legend="What to do" options={MODE_OPTIONS} value={mode} onValueChange={onModeChange} />

      <h3>Games with errors</h3>
      <RadioGroup
        legend="Games with errors"
        legendHidden
        options={BROKEN_OPTIONS}
        value={broken}
        onValueChange={onBrokenChange}
      />

      <h3>Result tags</h3>
      <Checkbox
        label="Exclude games with inconsistent result tags"
        help='Skips games where the result written at the end of the moves (e.g. a trailing "1-0") disagrees with the Result tag at the top. This does not catch every kind of wrong result — for example a Result tag that disagrees with an actual checkmate on the board is only ever a warning in the log, never excluded by this option.'
        checked={cleanup.rejectBadResults}
        disabled={!loaded || !capabilities?.rejectBadResults}
        onCheckedChange={(rejectBadResults) => onCleanupChange({ rejectBadResults })}
      />
      {(!loaded || !capabilities?.rejectBadResults) && (
        <p className="workflow-screen__section-help">
          {capabilityDisabledReason(loaded, Boolean(capabilities?.rejectBadResults))}
        </p>
      )}
      <Checkbox
        label="Automatically correct resolvable result tags"
        help="Rewrites a Result tag when the true outcome can be determined unambiguously, e.g. from checkmate."
        checked={cleanup.fixResultTags}
        disabled={!loaded || !capabilities?.fixResultTags}
        onCheckedChange={(fixResultTags) => onCleanupChange({ fixResultTags })}
      />
      {(!loaded || !capabilities?.fixResultTags) && (
        <p className="workflow-screen__section-help">
          {capabilityDisabledReason(loaded, Boolean(capabilities?.fixResultTags))}
        </p>
      )}
    </section>
  );
}
