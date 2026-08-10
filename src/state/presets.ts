// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Built-in presets (architecture.md §12.1, §12.2). Per §12.1, "a preset
 * produces a complete JobSpec diff, not a hidden command string" — each
 * entry below is a plain data object the Operations screen applies to
 * `WorkflowState` and can then be freely edited; nothing is hidden behind a
 * generated command until the Review screen's optional advanced view.
 *
 * Wording constraints (task spec, restated — these are honesty
 * requirements, not style preferences):
 *  - "Lucena-Ready PGN" must not claim targeted removal of clocks/engine
 *    evaluations — the engine can only remove *all* comments (§12.2's own
 *    closing paragraph, architecture.md §12.2).
 *  - No preset may claim a separate broken-games file exists (D-007 V-5):
 *    every preset here uses `broken: "discard"`, and its own description
 *    never mentions a broken-games artifact.
 *
 * Phase 4: "Lucena-Ready PGN"'s description also names NAGs alongside
 * comments/variations, since its `effect.operations.cleanup.removeNags` is
 * `true` — an actual effect the description previously left unmentioned.
 * Every preset here is verified end-to-end against the real engine in
 * `src-tauri/tests/phase4_integration.rs` (§24 Phase 4 exit criterion).
 *
 * **Versioning (§12.1: "Presets must be versioned"; §24 Phase 4: "Presets
 * and versioning").** Each `PresetDefinition` carries its own `version`,
 * independent of `JobSpec.schemaVersion` (which versions the wire DTO
 * shape, not any individual preset's chosen flag combination). All six
 * presets are `version: 1` — this is their first implementation, so none
 * has changed since it was introduced. Bump only the specific preset(s)
 * whose `effect` actually changes in a later release; the rest keep their
 * existing number. This is a UI/state-layer concept only (deliberately not
 * threaded through `JobSpec`/the manifest, which architecture.md §9.2 and
 * §15.3 do not call for) — `OperationSummary.tsx` surfaces it on the Review
 * screen ("Started from preset ... version N") so it is inspectable, per
 * the sibling sentence in §12.1 ("Users can inspect and modify it"), not a
 * hidden implementation detail.
 */
import type { ArtifactPreferences, PresetId } from "../types/workflow";
import { defaultCleanupOptions, defaultOperationPlan } from "./defaults";
import type { OperationPlan } from "../ipc/client";

export interface PresetEffect {
  operations: OperationPlan;
  uniqueGames: boolean;
  artifacts: ArtifactPreferences;
}

export interface PresetDefinition {
  id: Exclude<PresetId, "custom">;
  /** Positive integer, bumped whenever this specific preset's `effect`
   * changes in a later release (see this module's top-level doc comment). */
  version: number;
  label: string;
  description: string;
  effect: PresetEffect;
}

function cleanupOf(overrides: Partial<ReturnType<typeof defaultCleanupOptions>>) {
  return { ...defaultCleanupOptions(), ...overrides };
}

function artifacts(overrides: Partial<ArtifactPreferences> = {}): ArtifactPreferences {
  return {
    logFile: true,
    manifest: true,
    duplicateGames: "none",
    alwaysCreateAudit: false,
    ...overrides,
  };
}

export const PRESETS: readonly PresetDefinition[] = [
  {
    id: "mergeSafely",
    version: 1,
    label: "Merge Safely",
    description:
      "Combine every source file into one PGN. Nothing is removed — comments, variations, NAGs, and results are all kept.",
    effect: {
      operations: { ...defaultOperationPlan(), mode: "process", duplicates: "none" },
      uniqueGames: true,
      artifacts: artifacts(),
    },
  },
  {
    id: "cleanCollection",
    version: 1,
    label: "Clean Collection",
    description:
      "Combine every source file, keep only the first copy of each duplicated game, and save the later copies to a separate audit file. Comments and variations are kept.",
    effect: {
      operations: { ...defaultOperationPlan(), mode: "process", duplicates: "reportAndKeepFirst" },
      uniqueGames: true,
      artifacts: artifacts({ duplicateGames: "audit" }),
    },
  },
  {
    id: "minimalMainline",
    version: 1,
    label: "Minimal Mainline PGN",
    description:
      "Combine sources, remove duplicate games (no audit file), and strip comments, variations, and NAGs, leaving plain mainline move scores.",
    effect: {
      operations: {
        ...defaultOperationPlan(),
        mode: "process",
        duplicates: "suppressKeepFirst",
        cleanup: cleanupOf({
          removeComments: true,
          removeVariations: true,
          removeNags: true,
        }),
      },
      uniqueGames: true,
      artifacts: artifacts(),
    },
  },
  {
    id: "lucenaReady",
    version: 1,
    label: "Lucena-Ready PGN",
    description:
      "Combine sources, keep only unique mainline games, remove comments, variations, and NAGs — this also removes any clock times or engine evaluations stored inside comments, since the engine can only remove comments as a whole, not just specific kinds of note — and add ECO opening codes.",
    effect: {
      operations: {
        ...defaultOperationPlan(),
        mode: "process",
        duplicates: "reportAndKeepFirst",
        cleanup: cleanupOf({
          removeComments: true,
          removeVariations: true,
          removeNags: true,
        }),
        eco: { enabled: true },
      },
      uniqueGames: true,
      artifacts: artifacts({ duplicateGames: "audit" }),
    },
  },
  {
    id: "validateOnly",
    version: 1,
    label: "Validate Only",
    description:
      "Check every source file for errors and produce a report. No merged games file is written.",
    effect: {
      operations: { ...defaultOperationPlan(), mode: "validateOnly", duplicates: "none" },
      uniqueGames: false,
      artifacts: artifacts(),
    },
  },
  {
    id: "newGamesAgainstMaster",
    version: 1,
    label: "New Games Against Master",
    description:
      "Compare one or more files against a master database and keep only the games that are not already in it. The master file itself is never included in the output. Choose the master file below after applying this preset.",
    effect: {
      operations: { ...defaultOperationPlan(), mode: "process", duplicates: "suppressKeepFirst" },
      uniqueGames: true,
      artifacts: artifacts(),
    },
  },
];

export function getPreset(id: Exclude<PresetId, "custom">): PresetDefinition {
  const found = PRESETS.find((p) => p.id === id);
  if (!found) {
    throw new Error(`unknown preset id: ${id}`);
  }
  return found;
}

/**
 * Whether `current` still matches `presetId`'s own effect exactly. Used to
 * decide whether the Operations screen shows a preset as still-selected or
 * has quietly become `"custom"` after a manual edit; `checkFile` is excluded
 * because it is never part of a preset's own effect (see this module's
 * top-level doc comment).
 */
/** `checkFile` is never part of a preset's own effect (see this module's
 * top-level doc comment), so comparison normalizes it to a fixed sentinel
 * on both sides rather than comparing its real value. */
function checkFileNormalizedJson(operations: OperationPlan): string {
  return JSON.stringify({ ...operations, checkFile: null });
}

export function matchesPreset(
  presetId: Exclude<PresetId, "custom">,
  current: { operations: OperationPlan; uniqueGames: boolean; artifacts: ArtifactPreferences },
): boolean {
  const preset = getPreset(presetId);
  return (
    checkFileNormalizedJson(current.operations) === checkFileNormalizedJson(preset.effect.operations) &&
    current.uniqueGames === preset.effect.uniqueGames &&
    JSON.stringify(current.artifacts) === JSON.stringify(preset.effect.artifacts)
  );
}
