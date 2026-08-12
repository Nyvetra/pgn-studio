// SPDX-License-Identifier: GPL-3.0-or-later
//
// Regenerates `src-tauri/resources/eco-supplement/eco-supplement.pgn` from
// the vendored MIT-licensed `engine-src/eco-json/` dataset.
//
// WHAT THIS DOES NOT DO: it never reads, writes, or modifies
// `src-tauri/resources/pgn-extract/eco.pgn` beyond opening it read-only to
// find out which lines are ALREADY classified. That file is redistributed
// byte-for-byte unmodified and is checksummed in
// `resources/pgn-extract/SOURCE.json`; this script would be wrong if it
// ever touched it.
//
// The supplement contains only opening lines the bundled eco.pgn does not
// contain at all. Combined with `filesystem::eco_merge`'s bundled-content-
// first ordering (pgn-extract resolves a duplicated line to its FIRST
// occurrence - verified empirically, see that module's doc comment), this
// gives two independent guarantees that no existing classification can be
// overridden: the duplicate is not in the supplement in the first place,
// and even if it were, the bundled copy would win.
//
// Output is deterministic: entries are sorted by (eco, ply-count, move key)
// so regenerating from unchanged inputs produces a byte-identical file.
//
// Usage:  node scripts/build-eco-supplement.mjs [--check]
//   --check  verify the committed supplement is up to date; exit 1 if not
//            (used by CI so the generated file can never silently drift
//            from its inputs)

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const JSON_DIR = path.join(REPO_ROOT, "engine-src", "eco-json");
const ECO_PGN = path.join(REPO_ROOT, "src-tauri", "resources", "pgn-extract", "eco.pgn");
const OUT_FILE = path.join(
  REPO_ROOT,
  "src-tauri",
  "resources",
  "eco-supplement",
  "eco-supplement.pgn",
);

const checkOnly = process.argv.includes("--check");

/** Normalizes a SAN move sequence into a stable comparison key: drops move
 * numbers, comments, NAGs, annotation glyphs, and the result token, and
 * collapses whitespace. Case is preserved - SAN is case-significant (`b4`
 * the pawn move vs `B4` which is not a move at all). */
function moveKey(text) {
  return text
    .replace(/\{[^}]*\}/g, " ")
    .replace(/\([^)]*\)/g, " ")
    .replace(/\$\d+/g, " ")
    .replace(/\b\d+\s*\.(\.\.)?/g, " ")
    .replace(/[?!]+/g, "")
    .replace(/\*|1-0|0-1|1\/2-1\/2/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
}

/** Reads the bundled eco.pgn (READ-ONLY) and returns the set of move keys
 * it already classifies. */
function bundledMoveKeys(file) {
  const body = fs.readFileSync(file, "utf8").replace(/^\s*\{[\s\S]*?\}/, "");
  const keys = new Set();
  const tagRe = /\[(\w+)\s+"((?:[^"\\]|\\.)*)"\]/g;
  for (const chunk of body.split(/(?=\[ECO\s)/g)) {
    if (!chunk.trim()) continue;
    tagRe.lastIndex = 0;
    let match;
    let lastEnd = 0;
    let sawEco = false;
    while ((match = tagRe.exec(chunk)) !== null) {
      if (match[1] === "ECO") sawEco = true;
      lastEnd = tagRe.lastIndex;
    }
    if (!sawEco) continue;
    const key = moveKey(chunk.slice(lastEnd));
    if (key) keys.add(key);
  }
  return keys;
}

/** eco.pgn splits an opening name across [Opening] and [Variation]; the
 * json dataset uses a single "Opening: Variation" string. */
function splitName(name) {
  const i = name.indexOf(":");
  if (i === -1) return { opening: name.trim(), variation: "" };
  return { opening: name.slice(0, i).trim(), variation: name.slice(i + 1).trim() };
}

const escapeTag = (s) => s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');

function build() {
  const skipped = { noMoves: 0, badEco: 0, alreadyClassified: 0, duplicateLine: 0 };
  const pgnKeys = bundledMoveKeys(ECO_PGN);
  const rows = new Map(); // move key -> entry (first occurrence wins)

  const files = fs
    .readdirSync(JSON_DIR)
    .filter((f) => f.endsWith(".json"))
    .sort();
  if (files.length === 0) {
    throw new Error(`no .json files found in ${JSON_DIR}`);
  }

  for (const file of files) {
    const obj = JSON.parse(fs.readFileSync(path.join(JSON_DIR, file), "utf8"));
    for (const entry of Object.values(obj)) {
      if (!entry.moves || !entry.moves.trim()) {
        skipped.noMoves++;
        continue;
      }
      // Only real ECO codes: the dataset carries a handful of malformed
      // ones, and an invalid code is worse than no classification.
      if (!entry.eco || !/^[A-E]\d{2}$/.test(entry.eco)) {
        skipped.badEco++;
        continue;
      }
      const key = moveKey(entry.moves);
      if (!key) {
        skipped.noMoves++;
        continue;
      }
      if (pgnKeys.has(key)) {
        skipped.alreadyClassified++;
        continue;
      }
      if (rows.has(key)) {
        skipped.duplicateLine++;
        continue;
      }
      rows.set(key, {
        eco: entry.eco,
        name: entry.name ?? "",
        // The source data has irregular internal spacing ("1. a4  a5");
        // collapse it so the emitted file is canonical PGN.
        moves: entry.moves.trim().replace(/\s+/g, " "),
        plies: key.split(" ").length,
        key,
      });
    }
  }

  const entries = [...rows.values()].sort(
    (a, b) => a.eco.localeCompare(b.eco) || a.plies - b.plies || a.key.localeCompare(b.key),
  );

  // NOTE: this template is compared byte-for-byte against the committed
  // eco-supplement.pgn by --check, so editing anything below REQUIRES
  // regenerating the file in the same change - otherwise --check
  // immediately reports the committed output as stale. That is a reason to
  // regenerate, never a reason to leave a wrong path here: this header is
  // the shipped file's own provenance record, and pointing it at a
  // directory that no longer exists is the exact drift --check exists to
  // prevent.
  const header = `{
PGN Studio ECO supplement - GENERATED FILE, DO NOT EDIT BY HAND.
Regenerate with: node scripts/build-eco-supplement.mjs

This file is NOT part of the pgn-extract distribution and is NOT a modified
copy of pgn-extract's own eco.pgn. That file is redistributed byte-for-byte
unmodified; see THIRD_PARTY_NOTICES.md and
resources/pgn-extract/SOURCE.json.

Derived from the MIT-licensed eco.json dataset
(https://github.com/hayatbiralem/eco.json), vendored at engine-src/eco-json/
and recorded in resources/eco-supplement/SOURCE.json.

This supplement contains ONLY opening lines the bundled eco.pgn does not
classify at all - every line already present there was excluded when this
file was generated. The engine is given the two files concatenated with the
bundled content FIRST, and pgn-extract resolves a duplicated line to its
first occurrence, so the bundled file's classifications always win.

Entries: ${entries.length}
}

`;

  const body = entries
    .map((e) => {
      const { opening, variation } = splitName(e.name);
      const tags = [`[ECO "${escapeTag(e.eco)}"]`];
      if (opening) tags.push(`[Opening "${escapeTag(opening)}"]`);
      if (variation) tags.push(`[Variation "${escapeTag(variation)}"]`);
      return `${tags.join("\n")}\n\n${e.moves} *\n`;
    })
    .join("\n");

  return { text: header + body, entries, skipped, bundledCount: pgnKeys.size };
}

const { text, entries, skipped, bundledCount } = build();

if (checkOnly) {
  const existing = fs.existsSync(OUT_FILE) ? fs.readFileSync(OUT_FILE, "utf8") : null;
  if (existing === text) {
    console.log(`eco-supplement.pgn is up to date (${entries.length} entries).`);
    process.exit(0);
  }
  console.error(
    "eco-supplement.pgn is STALE or missing - it does not match what\n" +
      "engine-src/eco-json/ + eco.pgn currently produce.\n" +
      "Run: node scripts/build-eco-supplement.mjs",
  );
  process.exit(1);
}

fs.mkdirSync(path.dirname(OUT_FILE), { recursive: true });
fs.writeFileSync(OUT_FILE, text, "utf8");

console.log(`bundled eco.pgn lines (read-only):  ${bundledCount}`);
console.log(`skipped - already classified:       ${skipped.alreadyClassified}`);
console.log(`skipped - duplicate line in source: ${skipped.duplicateLine}`);
console.log(`skipped - malformed ECO code:       ${skipped.badEco}`);
console.log(`skipped - no moves:                 ${skipped.noMoves}`);
console.log(`supplement entries written:         ${entries.length}`);
console.log(`wrote ${OUT_FILE} (${fs.statSync(OUT_FILE).size} bytes)`);
