// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * Pure translation from Filters-screen UI state (`FilterDraft`) to the typed
 * `FilterPlan` wire shape (architecture.md §13.4; design-02 §1.5.1).
 *
 * Binding rule (task spec, restated): React must never compose criteria-file
 * *syntax*. Nothing below builds a string of criteria-file text — every
 * output is a typed `TagRule`/`MoveBounds`/`FenPatternFilter` value. Only
 * Rust (`engine::criteria`) ever renders the actual file content.
 *
 * "A year/date range is expressed as two bound rules on the same tag" and
 * "the same tag ORs together, different tags AND together" are the engine's
 * own semantics (verified, see `src-tauri/src/domain/filters.rs`'s doc
 * comment on `FilterPlan::tag_rules`) — this module follows that contract
 * literally rather than re-deriving it.
 */
import type { FilterPlan, MoveBounds, TagRule } from "../ipc/client";
import type { EloScope, FilterDraft } from "../types/workflow";

const ELO_MIN = 0;
const ELO_MAX = 4000;
const MOVE_MIN = 1;
const MOVE_MAX = 4999;

function eloTagFor(scope: EloScope): TagRule["tag"] {
  if (scope === "white") return "WhiteElo";
  if (scope === "black") return "BlackElo";
  return "Elo";
}

/** Four-digit year → a whole-year `Date` bound, formatted the way the PGN
 * standard's `Date` tag is written (`YYYY.MM.DD`). */
function yearBound(year: string, edge: "start" | "end"): string {
  const padded = year.trim().padStart(4, "0");
  return edge === "start" ? `${padded}.01.01` : `${padded}.12.31`;
}

export function compileFilters(draft: FilterDraft): FilterPlan {
  const tagRules: TagRule[] = [];

  if (draft.player.trim()) {
    tagRules.push({ tag: "Player", op: "prefix", value: draft.player.trim() });
  }
  if (draft.white.trim()) {
    tagRules.push({ tag: "White", op: "prefix", value: draft.white.trim() });
  }
  if (draft.black.trim()) {
    tagRules.push({ tag: "Black", op: "prefix", value: draft.black.trim() });
  }

  const results = new Set<string>();
  if (draft.resultWhiteWin || draft.decisiveOnly) results.add("1-0");
  if (draft.resultBlackWin || draft.decisiveOnly) results.add("0-1");
  if (draft.resultDraw) results.add("1/2-1/2");
  if (draft.resultOther) results.add("*");
  for (const value of results) {
    tagRules.push({ tag: "Result", op: "eq", value });
  }

  if (draft.dateFromYear.trim()) {
    tagRules.push({ tag: "Date", op: "ge", value: yearBound(draft.dateFromYear, "start") });
  }
  if (draft.dateToYear.trim()) {
    tagRules.push({ tag: "Date", op: "le", value: yearBound(draft.dateToYear, "end") });
  }

  const eloTag = eloTagFor(draft.eloScope);
  if (draft.eloMin.trim()) {
    tagRules.push({ tag: eloTag, op: "ge", value: draft.eloMin.trim() });
  }
  if (draft.eloMax.trim()) {
    tagRules.push({ tag: eloTag, op: "le", value: draft.eloMax.trim() });
  }

  for (const entry of draft.ecoEntries) {
    const value = entry.value.trim().toUpperCase();
    if (!value) continue;
    tagRules.push({ tag: "Eco", op: entry.exclude ? "ne" : "prefix", value });
  }

  const min = draft.moveMin.trim() ? Number(draft.moveMin) : null;
  const max = draft.moveMax.trim() ? Number(draft.moveMax) : null;
  const moveBounds: MoveBounds | null = min !== null || max !== null ? { min, max } : null;

  return {
    tagRules,
    moveBounds,
    checkmateOnly: draft.checkmateOnly,
    setupPolicy: draft.setupPolicy,
    fenPattern: null,
    textualVariations: [],
    advancedArgs: [],
  };
}

/** One inline validation problem, keyed by the field it belongs to so a
 * form can look up `problems.eloMin` etc. Mirrors (never replaces) the
 * backend's own authoritative checks in `filesystem::validate` and
 * `engine::command_compiler` — this is immediate client-side feedback only;
 * `validate_job` remains the gate that actually unlocks Run. */
export interface FilterProblems {
  eloMin?: string;
  eloMax?: string;
  moveMin?: string;
  moveMax?: string;
  dateFromYear?: string;
  dateToYear?: string;
}

function numberInRange(raw: string, lo: number, hi: number): string | undefined {
  if (!raw.trim()) return undefined;
  const n = Number(raw);
  if (!Number.isFinite(n) || !Number.isInteger(n)) return "must be a whole number";
  if (n < lo || n > hi) return `must be between ${lo} and ${hi}`;
  return undefined;
}

function yearProblem(raw: string): string | undefined {
  if (!raw.trim()) return undefined;
  const n = Number(raw);
  if (!Number.isFinite(n) || !Number.isInteger(n)) return "must be a four-digit year";
  if (n < 0 || n > 9999) return "must be a four-digit year";
  return undefined;
}

export function validateFilterDraft(draft: FilterDraft): FilterProblems {
  const problems: FilterProblems = {};

  const eloMin = numberInRange(draft.eloMin, ELO_MIN, ELO_MAX);
  if (eloMin) problems.eloMin = eloMin;
  const eloMax = numberInRange(draft.eloMax, ELO_MIN, ELO_MAX);
  if (eloMax) problems.eloMax = eloMax;
  if (
    !problems.eloMin &&
    !problems.eloMax &&
    draft.eloMin.trim() &&
    draft.eloMax.trim() &&
    Number(draft.eloMin) > Number(draft.eloMax)
  ) {
    problems.eloMin = "minimum must not be greater than maximum";
  }

  const moveMin = numberInRange(draft.moveMin, MOVE_MIN, MOVE_MAX);
  if (moveMin) problems.moveMin = moveMin;
  const moveMax = numberInRange(draft.moveMax, MOVE_MIN, MOVE_MAX);
  if (moveMax) problems.moveMax = moveMax;
  if (
    !problems.moveMin &&
    !problems.moveMax &&
    draft.moveMin.trim() &&
    draft.moveMax.trim() &&
    Number(draft.moveMin) > Number(draft.moveMax)
  ) {
    problems.moveMin = "minimum must not exceed maximum";
  }

  const dateFrom = yearProblem(draft.dateFromYear);
  if (dateFrom) problems.dateFromYear = dateFrom;
  const dateTo = yearProblem(draft.dateToYear);
  if (dateTo) problems.dateToYear = dateTo;

  return problems;
}

export function filterDraftHasProblems(draft: FilterDraft): boolean {
  return Object.keys(validateFilterDraft(draft)).length > 0;
}

/** `--detag` tag-name rule the compiler enforces (design-02 row 11):
 * ASCII letter, then ASCII letters/digits. */
export const TAG_IDENTIFIER_PATTERN = /^[A-Za-z][A-Za-z0-9]*$/;

export function isValidTagIdentifier(value: string): boolean {
  return TAG_IDENTIFIER_PATTERN.test(value);
}
