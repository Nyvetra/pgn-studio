// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { PRESETS, getPreset, matchesPreset } from "./presets";
import { defaultArtifactPreferences, defaultOperationPlan } from "./defaults";

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
