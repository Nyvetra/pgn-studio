// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Duplicate policy (architecture.md §13.3, §10.7).
 *
 * Wording constraints (task spec, binding, restated):
 *  - Always "Keep first copy", never "Keep best copy".
 *  - Duplicate identity is based on the moves played, so games with
 *    different headers, comments, or variations still count as duplicates,
 *    and a later copy may hold better metadata — said explicitly here, not
 *    just on the Files screen.
 */
import { useState } from "react";
import { selectInputFiles } from "../../ipc/client";
import type { DuplicatePolicy, EngineCapabilities, JobMode, RuntimeOptions } from "../../ipc/client";
import type { ArtifactPreferences } from "../../types/workflow";
import { RadioGroup, type RadioOption } from "../../components/RadioGroup";
import { Checkbox } from "../../components/Checkbox";
import { Button } from "../../components/Button";
import { TextField } from "../../components/TextField";
import { Banner } from "../../components/Banner";
import { capabilityDisabledReason } from "./capabilityHelp";

export interface DuplicateSectionProps {
  mode: JobMode;
  duplicates: DuplicatePolicy;
  checkFile: string | null;
  artifacts: ArtifactPreferences;
  runtime: RuntimeOptions;
  capabilities: EngineCapabilities | null;
  onDuplicatesChange: (policy: DuplicatePolicy) => void;
  onCheckFileChange: (path: string | null) => void;
  onArtifactsChange: (patch: Partial<ArtifactPreferences>) => void;
  onRuntimeChange: (patch: Partial<RuntimeOptions>) => void;
}

export function DuplicateSection({
  mode,
  duplicates,
  checkFile,
  artifacts,
  runtime,
  capabilities,
  onDuplicatesChange,
  onCheckFileChange,
  onArtifactsChange,
  onRuntimeChange,
}: DuplicateSectionProps) {
  const [pickingCheckFile, setPickingCheckFile] = useState(false);
  const loaded = capabilities !== null;
  const disabledByMode = mode === "validateOnly";

  const options: readonly RadioOption<DuplicatePolicy>[] = [
    {
      value: "none",
      label: "Do not check for duplicates",
      help: "Every game from every source is kept, even if it appears more than once.",
    },
    {
      value: "reportAndKeepFirst",
      label: "Keep first copy, save the rest to an audit file",
      help: 'Duplicate identity is based on the moves played, not headers, comments, or variations — games with different headers can still count as duplicates. The first copy in your file order (Files screen) is kept in the main output; later copies go to a separate audit file so you can review them. A later copy may actually hold better metadata or annotations.',
      disabled: !loaded || !(capabilities?.duplicateDetection && capabilities?.duplicateAuditFile),
    },
    {
      value: "suppressKeepFirst",
      label: "Keep first copy, discard the rest",
      help: "Same duplicate identity rule as above, but later copies are discarded outright — no audit file is produced.",
      disabled: !loaded || !capabilities?.duplicateDetection,
    },
  ];

  async function handlePickCheckFile() {
    setPickingCheckFile(true);
    try {
      const result = await selectInputFiles();
      if (result.status === "ok" && result.data.length > 0) {
        onCheckFileChange(result.data[0]);
      }
    } finally {
      setPickingCheckFile(false);
    }
  }

  return (
    <section aria-labelledby="operations-duplicates-heading">
      <h3 id="operations-duplicates-heading">Duplicate games</h3>
      {disabledByMode && (
        <Banner tone="info">
          Duplicate handling is not available in Validate Only mode — no games file is written to
          apply it to.
        </Banner>
      )}
      <fieldset disabled={disabledByMode} className="operations-fieldset-reset">
        <RadioGroup
          legend="Duplicate games"
          legendHidden
          options={options}
          value={duplicates}
          onValueChange={onDuplicatesChange}
        />

        {duplicates === "reportAndKeepFirst" && (
          <>
            <Checkbox
              label="Keep the audit file"
              help="Publish the diverted duplicate games to their own file. If unchecked, duplicates are still diverted out of the main output, just not saved anywhere."
              checked={artifacts.duplicateGames === "audit"}
              onCheckedChange={(checked) =>
                onArtifactsChange({ duplicateGames: checked ? "audit" : "none" })
              }
            />
            <Checkbox
              label="Always create the audit file, even if it would be empty"
              checked={artifacts.alwaysCreateAudit}
              disabled={artifacts.duplicateGames !== "audit"}
              onCheckedChange={(alwaysCreateAudit) => onArtifactsChange({ alwaysCreateAudit })}
            />
          </>
        )}

        <Checkbox
          label="Use disk-based duplicate storage for very large collections"
          help="Trades some speed for lower memory use. Only takes effect while duplicate checking above is enabled."
          checked={runtime.useExternalDuplicateTable}
          disabled={duplicates === "none" || !loaded || !capabilities?.externalDuplicateTable}
          onCheckedChange={(useExternalDuplicateTable) => onRuntimeChange({ useExternalDuplicateTable })}
        />
        {duplicates !== "none" && (!loaded || !capabilities?.externalDuplicateTable) && (
          <p className="workflow-screen__section-help">
            {capabilityDisabledReason(loaded, Boolean(capabilities?.externalDuplicateTable))}
          </p>
        )}

        <div className="check-file-picker">
          <TextField
            label="Master/check file (optional)"
            help='Compare against this file too, without adding its games to the output — this is what "New Games Against Master" uses. Requires a duplicate-handling option above.'
            value={checkFile ?? ""}
            onValueChange={() => {
              /* read-only display; chosen via the Browse dialog only */
            }}
            readOnly
            disabled={duplicates === "none" || !loaded || !capabilities?.checkFile}
            placeholder="No master file selected"
          />
          <Button
            type="button"
            variant="secondary"
            busy={pickingCheckFile}
            disabled={duplicates === "none" || !loaded || !capabilities?.checkFile}
            onClick={() => void handlePickCheckFile()}
          >
            Browse…
          </Button>
          {checkFile && (
            <Button type="button" variant="ghost" onClick={() => onCheckFileChange(null)}>
              Clear
            </Button>
          )}
        </div>
        {duplicates !== "none" && (!loaded || !capabilities?.checkFile) && (
          <p className="workflow-screen__section-help">
            {capabilityDisabledReason(loaded, Boolean(capabilities?.checkFile))}
          </p>
        )}
      </fieldset>
    </section>
  );
}
