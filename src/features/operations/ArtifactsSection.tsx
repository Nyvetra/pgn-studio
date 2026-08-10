// SPDX-License-Identifier: GPL-3.0-or-later
/** Audit artifacts (architecture.md §13.3): the log file and the
 * processing report. Duplicate-audit publishing has its own toggle inside
 * `DuplicateSection` since it only makes sense alongside that policy. */
import type { ArtifactPreferences } from "../../types/workflow";
import { Checkbox } from "../../components/Checkbox";

export interface ArtifactsSectionProps {
  artifacts: ArtifactPreferences;
  onChange: (patch: Partial<ArtifactPreferences>) => void;
}

export function ArtifactsSection({ artifacts, onChange }: ArtifactsSectionProps) {
  return (
    <section aria-labelledby="operations-artifacts-heading">
      <h3 id="operations-artifacts-heading">Audit artifacts</h3>
      <Checkbox
        label="Save a log file"
        help="A text file with the engine's full output for this job, saved next to your results."
        checked={artifacts.logFile}
        onCheckedChange={(logFile) => onChange({ logFile })}
      />
      <Checkbox
        label="Save a processing report"
        help="A summary of this job — inputs, options, and results, as JSON and as plain text — saved next to your results."
        checked={artifacts.manifest}
        onCheckedChange={(manifest) => onChange({ manifest })}
      />
    </section>
  );
}
