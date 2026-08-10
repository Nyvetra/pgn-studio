// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { defaultFilterDraft } from "./defaults";
import {
  compileFilters,
  isValidTagIdentifier,
  validateFilterDraft,
} from "./filterMapping";

describe("compileFilters", () => {
  it("produces an empty, engine-neutral FilterPlan for an untouched draft", () => {
    const plan = compileFilters(defaultFilterDraft());
    expect(plan).toEqual({
      tagRules: [],
      moveBounds: null,
      checkmateOnly: false,
      setupPolicy: "any",
      fenPattern: null,
      textualVariations: [],
      advancedArgs: [],
    });
  });

  it("never populates advancedArgs (reserved for V1.1, must stay empty)", () => {
    const plan = compileFilters({ ...defaultFilterDraft(), player: "Tal" });
    expect(plan.advancedArgs).toEqual([]);
  });

  it("compiles player/white/black as prefix ('starts with') rules, never equals", () => {
    const plan = compileFilters({
      ...defaultFilterDraft(),
      player: "Tal",
      white: "Fischer",
      black: "Karpov",
    });
    expect(plan.tagRules).toContainEqual({ tag: "Player", op: "prefix", value: "Tal" });
    expect(plan.tagRules).toContainEqual({ tag: "White", op: "prefix", value: "Fischer" });
    expect(plan.tagRules).toContainEqual({ tag: "Black", op: "prefix", value: "Karpov" });
  });

  it("trims whitespace and ignores blank name fields", () => {
    const plan = compileFilters({ ...defaultFilterDraft(), player: "  ", white: "  Botvinnik  " });
    expect(plan.tagRules).toEqual([{ tag: "White", op: "prefix", value: "Botvinnik" }]);
  });

  it('decisive-only unions in exactly the two decisive Result values, using "prefix" not "eq"', () => {
    // Correction, with evidence (Phase 5 task): this test used to assert
    // `op === "eq"`. Empirical testing against the real engine proved that
    // `Result = "1-0"` silently matches ZERO games, even when games with
    // exactly that Result exist — Result hits the same non-numeric "gate"
    // ECO does (D-010), which no unit test could catch since it is a fact
    // about the *engine's* interpretation, not this renderer. "prefix" (no
    // operator) is exactly equivalent to equality for these four literal
    // values (none is a textual prefix of another) and is empirically
    // verified to work. See `engine::criteria::tag_is_numeric`'s doc
    // comment (Rust) and `phase5_filters_integration.rs` for the real-engine
    // proof.
    const plan = compileFilters({ ...defaultFilterDraft(), decisiveOnly: true });
    const values = plan.tagRules.filter((r) => r.tag === "Result").map((r) => r.value).sort();
    expect(values).toEqual(["0-1", "1-0"]);
    expect(plan.tagRules.every((r) => r.op === "prefix")).toBe(true);
  });

  it('compiles a single Result checkbox with "prefix", never "eq" (D-010-class engine gate)', () => {
    const plan = compileFilters({ ...defaultFilterDraft(), resultDraw: true });
    expect(plan.tagRules).toEqual([{ tag: "Result", op: "prefix", value: "1/2-1/2" }]);
  });

  it("decisive-only and an explicit draw checkbox union together without duplicating", () => {
    const plan = compileFilters({
      ...defaultFilterDraft(),
      decisiveOnly: true,
      resultWhiteWin: true, // already implied by decisiveOnly
      resultDraw: true,
    });
    const values = plan.tagRules.filter((r) => r.tag === "Result").map((r) => r.value).sort();
    expect(values).toEqual(["0-1", "1-0", "1/2-1/2"]);
  });

  it("expands a year range into two Date bound rules (design-02: same-tag range expansion)", () => {
    const plan = compileFilters({ ...defaultFilterDraft(), dateFromYear: "1990", dateToYear: "2000" });
    expect(plan.tagRules).toContainEqual({ tag: "Date", op: "ge", value: "1990.01.01" });
    expect(plan.tagRules).toContainEqual({ tag: "Date", op: "le", value: "2000.12.31" });
  });

  it("supports an open-ended date range (only one bound set)", () => {
    const plan = compileFilters({ ...defaultFilterDraft(), dateFromYear: "2015" });
    expect(plan.tagRules).toEqual([{ tag: "Date", op: "ge", value: "2015.01.01" }]);
  });

  it("routes the Elo range to the pseudo-tag Elo by default (either player)", () => {
    const plan = compileFilters({ ...defaultFilterDraft(), eloMin: "2600", eloMax: "2800" });
    expect(plan.tagRules).toContainEqual({ tag: "Elo", op: "ge", value: "2600" });
    expect(plan.tagRules).toContainEqual({ tag: "Elo", op: "le", value: "2800" });
  });

  it("routes the Elo range to WhiteElo/BlackElo when scoped", () => {
    const white = compileFilters({ ...defaultFilterDraft(), eloScope: "white", eloMin: "2000" });
    expect(white.tagRules).toContainEqual({ tag: "WhiteElo", op: "ge", value: "2000" });
    const black = compileFilters({ ...defaultFilterDraft(), eloScope: "black", eloMax: "2000" });
    expect(black.tagRules).toContainEqual({ tag: "BlackElo", op: "le", value: "2000" });
  });

  it("compiles ECO entries as prefix matches by default, uppercased", () => {
    const plan = compileFilters({
      ...defaultFilterDraft(),
      ecoEntries: [{ id: "1", value: "b1", exclude: false }],
    });
    expect(plan.tagRules).toContainEqual({ tag: "Eco", op: "prefix", value: "B1" });
  });

  it('compiles excluded ECO entries with the verified "<>" operator (D-010), never =, >, >=, <, <=', () => {
    const plan = compileFilters({
      ...defaultFilterDraft(),
      ecoEntries: [{ id: "1", value: "C00", exclude: true }],
    });
    expect(plan.tagRules).toContainEqual({ tag: "Eco", op: "ne", value: "C00" });
    const forbidden = new Set(["eq", "gt", "ge", "lt", "le"]);
    expect(plan.tagRules.filter((r) => r.tag === "Eco").every((r) => !forbidden.has(r.op))).toBe(true);
  });

  it("skips blank ECO entries", () => {
    const plan = compileFilters({
      ...defaultFilterDraft(),
      ecoEntries: [{ id: "1", value: "   ", exclude: false }],
    });
    expect(plan.tagRules).toEqual([]);
  });

  it("compiles move bounds only when at least one side is set", () => {
    expect(compileFilters({ ...defaultFilterDraft(), moveMin: "10" }).moveBounds).toEqual({
      min: 10,
      max: null,
    });
    expect(compileFilters({ ...defaultFilterDraft(), moveMax: "40" }).moveBounds).toEqual({
      min: null,
      max: 40,
    });
  });

  it("passes through checkmateOnly and setupPolicy unchanged", () => {
    const plan = compileFilters({ ...defaultFilterDraft(), checkmateOnly: true, setupPolicy: "setupOnly" });
    expect(plan.checkmateOnly).toBe(true);
    expect(plan.setupPolicy).toBe("setupOnly");
  });

  it("passes filter text through byte-for-byte, unescaped and uncomposed — React never touches criteria syntax", () => {
    // Task binding rule: "React must never compose criteria syntax." This
    // value is deliberately adversarial (embedded quotes, a backslash, a
    // standalone quoted word) — if this module ever started escaping,
    // quoting, or otherwise composing criteria-file text, this exact
    // byte-for-byte equality would break. Escaping is exclusively
    // `engine::criteria::escape_value`'s job (Rust), proven separately by
    // `phase5_filters_integration.rs`'s `filter_value_with_quotes_and_backslashes_*`
    // tests against the real engine.
    const adversarial = String.raw`Ci\"ty "Open" C:\Games`;
    const plan = compileFilters({ ...defaultFilterDraft(), player: adversarial });
    expect(plan.tagRules).toEqual([{ tag: "Player", op: "prefix", value: adversarial }]);
  });

  it("never emits a relational/equality operator for a non-numeric tag (general form of the Result/ECO engine gate)", () => {
    // Generalizes the ECO-specific "forbidden" check above (D-010) across
    // every non-numeric tag this module can emit. Empirically verified
    // against the real engine (Phase 5 task; see
    // `engine::criteria::tag_is_numeric`'s doc comment in Rust): `=`, `>`,
    // `>=`, `<`, `<=` silently match nothing for Player/White/Black/Result/
    // ECO. Only `Date` and the Elo-family tags are exempt (they take the
    // engine's numeric/date comparison path), and this module never emits
    // Date with anything but "ge"/"le" anyway.
    const draft = {
      ...defaultFilterDraft(),
      player: "Tal",
      white: "Fischer",
      black: "Karpov",
      resultWhiteWin: true,
      resultBlackWin: true,
      resultDraw: true,
      resultOther: true,
      decisiveOnly: true,
      ecoEntries: [
        { id: "1", value: "B10", exclude: false },
        { id: "2", value: "C00", exclude: true },
      ],
    };
    const plan = compileFilters(draft);
    const nonNumericTextTags = new Set(["Player", "White", "Black", "Result", "Eco"]);
    const forbiddenOps = new Set(["eq", "gt", "ge", "lt", "le"]);
    const offenders = plan.tagRules.filter(
      (r) => nonNumericTextTags.has(r.tag) && forbiddenOps.has(r.op),
    );
    expect(offenders).toEqual([]);
    // Sanity: the draft above genuinely produced rules for every one of
    // those tags, so the assertion above isn't vacuously true.
    const tagsSeen = new Set<string>(plan.tagRules.map((r) => r.tag));
    for (const tag of nonNumericTextTags) {
      expect(tagsSeen.has(tag)).toBe(true);
    }
  });
});

describe("validateFilterDraft", () => {
  it("has no problems for an untouched draft", () => {
    expect(validateFilterDraft(defaultFilterDraft())).toEqual({});
  });

  it("flags an Elo value outside 0..=4000 (mirrors the backend's own bound)", () => {
    const problems = validateFilterDraft({ ...defaultFilterDraft(), eloMax: "9999" });
    expect(problems.eloMax).toBeDefined();
  });

  it("flags Elo min greater than max", () => {
    const problems = validateFilterDraft({ ...defaultFilterDraft(), eloMin: "2700", eloMax: "2000" });
    expect(problems.eloMin).toBeDefined();
  });

  it("flags a move bound outside 1..=4999 (mirrors compile()'s own bound)", () => {
    const problems = validateFilterDraft({ ...defaultFilterDraft(), moveMax: "5000" });
    expect(problems.moveMax).toBeDefined();
  });

  it("flags move min greater than max", () => {
    const problems = validateFilterDraft({ ...defaultFilterDraft(), moveMin: "40", moveMax: "10" });
    expect(problems.moveMin).toBeDefined();
  });

  it("flags a non-4-digit year", () => {
    const problems = validateFilterDraft({ ...defaultFilterDraft(), dateFromYear: "abcd" });
    expect(problems.dateFromYear).toBeDefined();
  });
});

describe("isValidTagIdentifier", () => {
  it("accepts letters-then-alphanumerics (matches --detag's own constraint)", () => {
    expect(isValidTagIdentifier("WhiteElo")).toBe(true);
    expect(isValidTagIdentifier("A1")).toBe(true);
  });

  it("rejects anything starting with a digit or containing punctuation", () => {
    expect(isValidTagIdentifier("1White")).toBe(false);
    expect(isValidTagIdentifier("White-Elo")).toBe(false);
    expect(isValidTagIdentifier("")).toBe(false);
  });
});
