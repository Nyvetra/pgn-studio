// SPDX-License-Identifier: GPL-3.0-or-later
/** ECO classification + output notation (architecture.md §13.3). Output
 * notation only ever offers what `EngineCapabilities.supportedOutputFormats`
 * advertises — the pinned build supports SAN only, so UCI is shown but
 * disabled with an explanation rather than hidden (task spec: "disable the
 * rest with an explanation"). */
import type { EngineCapabilities, OutputNotation } from "../../ipc/client";
import { Checkbox } from "../../components/Checkbox";
import { RadioGroup, type RadioOption } from "../../components/RadioGroup";
import { capabilityDisabledReason } from "./capabilityHelp";

export interface EcoOutputSectionProps {
  ecoEnabled: boolean;
  outputNotation: OutputNotation;
  capabilities: EngineCapabilities | null;
  onEcoChange: (enabled: boolean) => void;
  onNotationChange: (notation: OutputNotation) => void;
}

export function EcoOutputSection({
  ecoEnabled,
  outputNotation,
  capabilities,
  onEcoChange,
  onNotationChange,
}: EcoOutputSectionProps) {
  const loaded = capabilities !== null;
  const ecoSupported = Boolean(capabilities?.ecoClassification);

  const notationOptions: readonly RadioOption<OutputNotation>[] = [
    {
      value: "san",
      label: "Standard Algebraic Notation (SAN)",
      help: 'e.g. "Nf3" — the standard PGN move format, and the only one this engine build can write.',
    },
    {
      value: "uci",
      label: "UCI notation",
      help: 'e.g. "g1f3" — not offered by this engine build.',
      disabled: true,
    },
  ];

  return (
    <section aria-labelledby="operations-eco-heading">
      <h3 id="operations-eco-heading">ECO classification &amp; output notation</h3>
      <Checkbox
        label="Add ECO opening classification tags"
        help="Adds an ECO tag (and opening name, when known) to each game's headers, using the bundled ECO reference file."
        checked={ecoEnabled}
        disabled={!loaded || !ecoSupported}
        onCheckedChange={onEcoChange}
      />
      {(!loaded || !ecoSupported) && (
        <p className="workflow-screen__section-help">{capabilityDisabledReason(loaded, ecoSupported)}</p>
      )}

      <RadioGroup
        legend="Output move notation"
        options={notationOptions}
        value={outputNotation}
        onValueChange={onNotationChange}
      />
    </section>
  );
}
