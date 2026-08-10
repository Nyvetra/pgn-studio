// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { PRESETS, getPreset, matchesPreset } from "./presets";
import { defaultArtifactPreferences, defaultCleanupOptions, defaultOperationPlan } from "./defaults";
import type { PresetEffect } from "./presets";

describe("presets", () => {
  it("defines every preset id referenced by architecture.md §12.2", () => {
    const ids = PRESETS.map((p) => p.id).sort();
    expect(ids).toEqual(
      [
        "cleanCollection",
        "lucenaReady",
        "mergeSafely",
        "minimalMainline",
        "newGamesAgainstMaster",
        "validateOnly",
      ].sort(),
    );
  });

  it("never claims a separate broken-games file (D-007 V-5)", () => {
    for (const preset of PRESETS) {
      expect(preset.effect.operations.broken).toBe("discard");
      expect(preset.description.toLowerCase()).not.toContain("broken");
    }
  });

  it("every preset is versioned with a positive integer (architecture.md §12.1)", () => {
    for (const preset of PRESETS) {
      expect(Number.isInteger(preset.version)).toBe(true);
      expect(preset.version).toBeGreaterThanOrEqual(1);
    }
  });

  it("all six presets start at version 1 (first implementation, none has changed since)", () => {
    for (const preset of PRESETS) {
      expect(preset.version).toBe(1);
    }
  });

  it('Lucena-Ready describes the actual comment-removal effect, not targeted clock/eval removal', () => {
    const lucena = getPreset("lucenaReady");
    expect(lucena.description.toLowerCase()).not.toMatch(/only (the )?clock/);
    expect(lucena.description).toMatch(/comment/i);
  });

  it("duplicate policy wording never says \"keep best copy\" (§10.7)", () => {
    for (const preset of PRESETS) {
      expect(preset.description.toLowerCase()).not.toContain("best copy");
    }
  });

  it("validateOnly forces uniqueGames false and duplicates none (compiler contract)", () => {
    const preset = getPreset("validateOnly");
    expect(preset.effect.uniqueGames).toBe(false);
    expect(preset.effect.operations.duplicates).toBe("none");
    expect(preset.effect.operations.mode).toBe("validateOnly");
  });

  it("cleanCollection and lucenaReady publish the duplicates audit file, matching their own duplicate policy", () => {
    for (const id of ["cleanCollection", "lucenaReady"] as const) {
      const preset = getPreset(id);
      expect(preset.effect.operations.duplicates).toBe("reportAndKeepFirst");
      expect(preset.effect.artifacts.duplicateGames).toBe("audit");
    }
  });

  it("every preset's OperationPlan is internally consistent with output.duplicateGames (compile() invariant)", () => {
    for (const preset of PRESETS) {
      if (preset.effect.artifacts.duplicateGames === "audit") {
        expect(preset.effect.operations.duplicates).toBe("reportAndKeepFirst");
      }
    }
  });

  describe("exact JobSpec diff per preset (architecture.md §12.1: a preset is a complete, inspectable JobSpec diff)", () => {
    // Every expected value below is built only from `defaults.ts` (a
    // separately-tested module, not `presets.ts`'s own internal `cleanupOf`/
    // `artifacts` helpers), spread/overridden explicitly per preset. This is
    // deliberate: reusing `presets.ts`'s own helpers to check `presets.ts`'s
    // own output would be circular and could never catch a drift in either
    // a shared default or an individual preset's override.
    const baseCleanup = defaultCleanupOptions();
    const baseArtifacts = defaultArtifactPreferences();
    const strippedCleanup = { ...baseCleanup, removeComments: true, removeVariations: true, removeNags: true };

    const expected: Record<(typeof PRESETS)[number]["id"], PresetEffect> = {
      mergeSafely: {
        operations: { ...defaultOperationPlan(), mode: "process", duplicates: "none" },
        uniqueGames: true,
        artifacts: { ...baseArtifacts },
      },
      cleanCollection: {
        operations: { ...defaultOperationPlan(), mode: "process", duplicates: "reportAndKeepFirst" },
        uniqueGames: true,
        artifacts: { ...baseArtifacts, duplicateGames: "audit" },
      },
      minimalMainline: {
        operations: {
          ...defaultOperationPlan(),
          mode: "process",
          duplicates: "suppressKeepFirst",
          cleanup: strippedCleanup,
        },
        uniqueGames: true,
        artifacts: { ...baseArtifacts },
      },
      lucenaReady: {
        operations: {
          ...defaultOperationPlan(),
          mode: "process",
          duplicates: "reportAndKeepFirst",
          cleanup: strippedCleanup,
          eco: { enabled: true },
        },
        uniqueGames: true,
        artifacts: { ...baseArtifacts, duplicateGames: "audit" },
      },
      validateOnly: {
        operations: { ...defaultOperationPlan(), mode: "validateOnly", duplicates: "none" },
        uniqueGames: false,
        artifacts: { ...baseArtifacts },
      },
      newGamesAgainstMaster: {
        operations: { ...defaultOperationPlan(), mode: "process", duplicates: "suppressKeepFirst" },
        uniqueGames: true,
        artifacts: { ...baseArtifacts },
      },
    };

    for (const id of Object.keys(expected) as (keyof typeof expected)[]) {
      it(`${id} produces exactly its documented JobSpec diff`, () => {
        expect(getPreset(id).effect).toEqual(expected[id]);
      });
    }
  });

  describe("matchesPreset", () => {
    it("matches a freshly-applied preset's own effect", () => {
      const preset = getPreset("mergeSafely");
      expect(
        matchesPreset("mergeSafely", {
          operations: preset.effect.operations,
          uniqueGames: preset.effect.uniqueGames,
          artifacts: preset.effect.artifacts,
        }),
      ).toBe(true);
    });

    it("stops matching once a field is edited away from the preset", () => {
      const preset = getPreset("mergeSafely");
      const edited = {
        operations: { ...preset.effect.operations, duplicates: "suppressKeepFirst" as const },
        uniqueGames: preset.effect.uniqueGames,
        artifacts: preset.effect.artifacts,
      };
      expect(matchesPreset("mergeSafely", edited)).toBe(false);
    });

    it("ignores checkFile when comparing (never part of a preset's own effect)", () => {
      const preset = getPreset("mergeSafely");
      const withCheckFile = {
        operations: { ...preset.effect.operations, checkFile: "C:\\master.pgn" },
        uniqueGames: preset.effect.uniqueGames,
        artifacts: preset.effect.artifacts,
      };
      expect(matchesPreset("mergeSafely", withCheckFile)).toBe(true);
    });

    it("the freshly-created default draft is exactly the Merge Safely preset by construction", () => {
      // Documents an intentional property, not a coincidence: a brand-new
      // draft should already describe a safe, non-destructive default
      // (merge everything, transform nothing), which is precisely what
      // "Merge Safely" is. If this ever breaks, either `defaults.ts` or the
      // "Merge Safely" preset changed and the two need to be reconciled.
      const current = {
        operations: defaultOperationPlan(),
        uniqueGames: true,
        artifacts: defaultArtifactPreferences(),
      };
      expect(matchesPreset("mergeSafely", current)).toBe(true);
    });
  });
});
