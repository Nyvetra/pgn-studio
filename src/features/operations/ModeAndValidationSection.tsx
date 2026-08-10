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
    help: "Broken games are included in the same output file as valid ones, so nothing is silently dropped. There is still no separate file for them; check the log to see which games had problems.",
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
        help="Skips games where the final position's outcome does not match the Result tag."
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
