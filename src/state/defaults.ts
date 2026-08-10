// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Default values for a brand-new job draft. Kept dependency-free (no
 * `crypto.randomUUID()` reliance) so id generation behaves identically in
 * the browser, the Tauri webview, and jsdom under Vitest.
 */
import type {
  CleanupOptions,
  EcoOptions,
  OperationPlan,
  RuntimeOptions,
} from "../ipc/client";
import type { ArtifactPreferences, FilterDraft, OutputDestination } from "../types/workflow";

/** RFC-4122-*shaped* v4 id. Not cryptographically strong — job/row ids only
 * need to be unique within one running app, never to resist prediction.
 * Uses exact 32-bit integer arithmetic throughout (`Math.imul`/`>>> 0`, the
 * "mulberry32" generator) rather than the more obvious `seed * bigConstant`
 * approach, which silently overflows past `Number.MAX_SAFE_INTEGER` within
 * a handful of iterations and can degrade to `NaN` hex digits. */
export function generateId(): string {
  let state = (Date.now() ^ Math.floor(Math.random() * 0xffffffff)) >>> 0;
  const next = (): number => {
    state = (state + 0x6d2b79f5) | 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return (t ^ (t >>> 14)) >>> 0;
  };
  const hex = "0123456789abcdef";
  let out = "";
  for (let i = 0; i < 32; i += 1) {
    if (i === 8 || i === 12 || i === 16 || i === 20) out += "-";
    if (i === 12) {
      out += "4"; // version 4
    } else if (i === 16) {
      out += hex[(next() & 0x3) | 0x8]; // RFC 4122 variant
    } else {
      out += hex[next() % 16];
    }
  }
  return out;
}

export function defaultCleanupOptions(): CleanupOptions {
  return {
    removeComments: false,
    removeVariations: false,
    removeNags: false,
    removeMoveNumbers: false,
    removeResults: false,
    removeTags: [],
    rejectBadResults: false,
    fixResultTags: false,
  };
}

export function defaultEcoOptions(): EcoOptions {
  return { enabled: false };
}

export function defaultOperationPlan(): OperationPlan {
  return {
    mode: "process",
    duplicates: "none",
    cleanup: defaultCleanupOptions(),
    broken: "discard",
    eco: defaultEcoOptions(),
    outputNotation: "san",
    checkFile: null,
  };
}

export function defaultOutputDestination(): OutputDestination {
  return {
    directory: "",
    baseName: "",
    conflictPolicy: "addNumericSuffix",
    confirmedReplace: false,
  };
}

export function defaultArtifactPreferences(): ArtifactPreferences {
  return {
    logFile: true,
    manifest: true,
    duplicateGames: "none",
    alwaysCreateAudit: false,
  };
}

export function defaultRuntimeOptions(): RuntimeOptions {
  return {
    useExternalDuplicateTable: false,
    countOutputGames: true,
  };
}

export function defaultFilterDraft(): FilterDraft {
  return {
    player: "",
    white: "",
    black: "",
    resultWhiteWin: false,
    resultBlackWin: false,
    resultDraw: false,
    resultOther: false,
    decisiveOnly: false,
    dateFromYear: "",
    dateToYear: "",
    eloScope: "either",
    eloMin: "",
    eloMax: "",
    ecoEntries: [],
    moveMin: "",
    moveMax: "",
    checkmateOnly: false,
    setupPolicy: "any",
  };
}
