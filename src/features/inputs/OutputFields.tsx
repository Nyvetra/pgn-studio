// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Output folder picker, base filename, and conflict policy (architecture.md
 * §13.2, §11.5). The actual replace confirmation dialog is shown on the
 * Review screen right before the job runs, not here — by then the user can
 * see the exact destination artifacts that would be affected
 * (architecture.md §13.5).
 */
import { selectOutputDirectory, type ConflictPolicy } from "../../ipc/client";
import { Button } from "../../components/Button";
import { TextField } from "../../components/TextField";
import { RadioGroup, type RadioOption } from "../../components/RadioGroup";
import "./OutputFields.css";

export interface OutputFieldsProps {
  directory: string;
  baseName: string;
  conflictPolicy: ConflictPolicy;
  onDirectoryChange: (directory: string) => void;
  onBaseNameChange: (baseName: string) => void;
  onConflictPolicyChange: (policy: ConflictPolicy) => void;
}

const CONFLICT_OPTIONS: readonly RadioOption<ConflictPolicy>[] = [
  {
    value: "addNumericSuffix",
    label: "Add a number to the new file's name",
    help: 'If "clean.pgn" already exists, PGN Studio writes "clean (1).pgn" instead. Nothing existing is touched.',
  },
  {
    value: "fail",
    label: "Stop instead of writing over anything",
    help: "If any planned output file already exists, the job is refused before it starts.",
  },
  {
    value: "replaceAfterConfirmation",
    label: "Replace the existing file, after confirming",
    help: "You will be asked to confirm on the Review screen. The previous file is renamed to a timestamped .bak copy first — nothing is silently overwritten or deleted.",
  },
];

export function OutputFields({
  directory,
  baseName,
  conflictPolicy,
  onDirectoryChange,
  onBaseNameChange,
  onConflictPolicyChange,
}: OutputFieldsProps) {
  async function handleBrowse() {
    const result = await selectOutputDirectory();
    if (result.status === "ok" && result.data) {
      onDirectoryChange(result.data);
    }
  }

  return (
    <div className="output-fields">
      <div className="output-fields__directory">
        <TextField
          label="Output folder"
          help="Where the new files this job creates will be written. Your source files are never modified."
          value={directory}
          onValueChange={onDirectoryChange}
          placeholder="Choose a folder…"
          required
        />
        <Button type="button" variant="secondary" onClick={() => void handleBrowse()}>
          Browse…
        </Button>
      </div>
      <TextField
        label="Base filename"
        help='Output files are named from this, e.g. "clean.pgn", "clean.duplicates.pgn".'
        value={baseName}
        onValueChange={onBaseNameChange}
        placeholder="clean"
        required
      />
      <RadioGroup
        legend="If an output file already exists"
        options={CONFLICT_OPTIONS}
        value={conflictPolicy}
        onValueChange={onConflictPolicyChange}
      />
    </div>
  );
}
