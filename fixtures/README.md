# fixtures/

Small, synthetic PGN test fixtures for engine-integration and adapter
testing (architecture.md §20.3). Every game in every file here was authored
for this project - none of it comes from a real game database, per
architecture.md §17.3 ("Test fixtures must be created by contributors,
clearly public domain/CC0, or sufficiently minimal synthetic fixtures").
Player names are fictional/placeholder (some are deliberately
opening-theorist-adjacent puns, e.g. "Steinitz-alike, Silas") and any
resemblance of a game score to a real historical game is coincidental
opening theory, not a copied game record - opening moves are, in any case,
not copyrightable.

None of this is wired into an automated test suite yet - Phase 0 only
authors the fixtures. Phase 1+ integration tests should read from this
directory rather than embedding PGN text in test source.

## valid/

Well-formed PGN, each file exercising a specific feature:

| File | Exercises |
|---|---|
| `single-game.pgn` | One ordinary, fully-formed game. |
| `multi-game-results.pgn` | Three games in one file covering all four `Result` values: `1-0`, `0-1`, `1/2-1/2`, `*`. |
| `setup-fen.pgn` | Two games using `[SetUp "1"]` + `[FEN "..."]` to start from a non-initial position (one a normal opening continuation, one a bare king-and-pawn endgame study). |
| `variations-and-nags.pgn` | Nested-depth-1 RAV variations `(...)` and Numeric Annotation Glyphs (`$1`, `$5`, `$6`, `$10`). |
| `unicode-comments.pgn` | Comments containing literal `[square brackets]`, literal `(parentheses)`, and non-Latin Unicode text (Russian, Chinese, Hindi, Bengali, Arabic, chess-piece emoji). Comments intentionally do **not** contain literal `{`/`}`, since PGN comments cannot escape or nest their own delimiter. |
| `long-comment.pgn` | One deliberately long (~1500 character) comment, to test streaming/buffering rather than truncation. |
| `lf-line-endings.pgn` | Baseline file using bare `\n` line endings only (verified: 9/9 line endings are bare LF, 0 CRLF). |
| `crlf-line-endings.pgn` | Same game as the LF file, re-encoded with `\r\n` line endings (verified: 9/9 line endings are CRLF pairs, 0 bare LF). |
| `utf8-bom.pgn` | Same game again, saved with a leading UTF-8 byte-order mark (`EF BB BF`) before the first `[` byte (verified via hex dump). |

## duplicates/

Each file is a matched set designed for duplicate-detection testing
(architecture.md §10.7 - detection is based on the move sequence, not
headers, comments, or variations):

| File | Exercises |
|---|---|
| `identical-pair.pgn` | Two games with byte-identical headers **and** moves - the simplest possible duplicate. |
| `same-moves-different-headers.pgn` | Identical move sequence, but different `Event`/`Site`/`Date`/`Round` - proves duplicate identity must not depend on headers. |
| `annotated-vs-plain.pgn` | Identical moves and headers, but only the second copy has a comment and a NAG - proves annotations are not part of duplicate identity, and that the annotated copy's extra information would be lost if the plain copy were kept without an audit trail. |
| `same-players-different-games.pgn` | Same two players across two rounds of a fictional match, but genuinely different openings/moves/results - a **negative** case: must *not* be flagged as a duplicate despite matching headers. |
| `truncated-vs-complete.pgn` | A complete game and a second game sharing its exact opening prefix but ending early with `*` - a **negative** case: a truncated score must not be treated as identical to a complete one that happens to share an opening. |
| `order-a.pgn`, `order-b.pgn` | A matched pair, one game each, identical moves but maximally different headers (`Event`/`Site`/`Date`/`Round`/`White`/`Black`) - designed to be fed as two separate *input files* with swappable priority, so a test can prove input order (not content) decides which copy survives (architecture.md §10.7's "input order is a retention priority"). |
| `annotated-first-then-plain.pgn` | Identical moves, first copy carries a comment and a NAG, second (later) copy is plain - the mirror image of `annotated-vs-plain.pgn`. Proves the Phase 3 annotated-duplicate warning is scoped to what was actually *discarded*: since the plain copy is the one diverted to the audit file here, no warning should fire, even though the collection as a whole does contain an annotation. |

## malformed/

Deliberately broken PGN, one specific defect per file:

| File | Defect |
|---|---|
| `malformed-quotes.pgn` | The `Event` tag's value is missing its closing double-quote. |
| `illegal-move.pgn` | `3. Nc4` is not reachable by either White knight from the preceding position (not a legal knight move from f3 or b1) - an impossible move, not just a bad one. |
| `inconsistent-result.pgn` | The `[Result "1-0"]` tag disagrees with the movetext's own termination marker (`1/2-1/2`). |
| `missing-result-marker.pgn` | The movetext has real moves but no trailing `1-0`/`0-1`/`1/2-1/2`/`*` token at all. |

## unicode-paths/

Non-Latin file and folder names (Bengali), verified byte-correct on disk via
`fs.readdirSync`:

- `দাবা-খেলা.pgn` ("chess game") - a Bengali-named file directly in this
  directory, whose one comment is also written in Bengali.
- `বাংলা-উদাহরণ/` ("Bengali example") - a Bengali-named **folder**,
  containing `দ্বিতীয়-নমুনা.pgn` ("second sample") - a Bengali-named file
  inside a Bengali-named folder, to test nested Unicode path handling.

## filters/

Fixtures for Phase 5 (architecture.md §13.4, §24 Phase 5) filter/criteria-file
integration testing, each purpose-built so a filter's positive, negative, and
combination behavior can be asserted on an *exact* game count and be
independently re-verified by running the real engine directly (every count
in every filter integration test was produced this way, not hand-guessed):

| File | Exercises |
|---|---|
| `players-results-dates.pgn` | Six games for player (either/White/Black), result, decisive-only, and date-range filters. Deliberately includes `Talbot, Toby` alongside `Tal, Mikhail` (the no-op-is-prefix-not-equality trap: a "Tal" filter also matches "Talbot"), and one game with Cyrillic player names (`Чайковский, Пётр` / `Толстой, Лев`) for Unicode round-trip-through-the-criteria-file testing. |
| `date-edge-1999.pgn` | Three games dated `1999.01.01`, `1999.12.31`, and `2000.01.01` — pins the "`Date` ranges must render full dates" hazard (design-02 §1.5.1): a naive `Date <= "1999"` would wrongly exclude the `1999.12.31` game, since the engine defaults a missing month/day to `01/01`. |
| `elo-tags.pgn` | Four games: both players rated, both mid-rated, **no Elo tags at all** (proves a game missing the filtered tag does not match), and **White-only** rated (proves the `Elo` pseudo-tag's "White first, then Black" semantics — a filter can be satisfied by one side alone even when the other side's tag is entirely absent). |
| `eco-codes.pgn` | Five games with pre-set `ECO` tags (`B10`, `B90`, `A00`, `C50`, `B12`) spanning a "family" prefix collision (`B10`/`B12` both match prefix `"B1"`) — exercises prefix, `<>` (not-equal — DECISIONS-LEDGER.md D-010), and `=~` (regex) matching. ECO tags are set directly in the fixture rather than produced by `-e` classification, matching the same methodology D-010 itself used. |
| `move-bounds.pgn` | Three games of exactly 3, 15, and 30 full moves (a deterministic, trivially-legal knight-shuffle move sequence — `Nf3`/`Ng1`/`Nf6`/`Ng8` repeated — chosen so the exact move count is correct by construction rather than by counting a "real" game's moves by hand). Reproduces DECISIONS-LEDGER.md D-007 V-3's `--maxmoves`/`--minmoves` order hazard: with a 10–15 filter, correct order keeps only the 15-move game, reversed order silently admits the 30-move game too. |
| `checkmates.pgn` | Fool's mate (`Qh4#`, Black wins by checkmate) and Scholar's mate (`Qxf7#`, White wins by checkmate) alongside a decisive-but-not-checkmate game (a normal opening sequence with a `Result` tag but no `#` anywhere) — proves `--checkmate` is a positional check, not a `Result`-tag check. |
| `escaping.pgn` | One game whose `Event` tag value contains a literal embedded double-quote and backslash (`Round "Robin" Stage C:\Games`, itself PGN-escaped in the file as `Round \"Robin\" Stage C:\\Games` per the PGN tag-value grammar), plus one plain game — proves a filter value containing `"`/`\` survives Rust's criteria-file escaping and round-trips into a correct match, and (combined with a second, unrelated criterion on `players-results-dates.pgn` in the integration test) that an adversarial value cannot corrupt/truncate a *later* line in the same criteria file (`taglines.c`'s silent-parse-termination hazard, design-02 §1.5). |
| `tag-missing-entirely.pgn` | One game with an `ECO` tag, one with no `ECO` tag at all — pins a **correction to design-02 §1.5.1** (see the task report): design-02 claims a game missing a criteria tag matches "unless every criterion on that tag is `<>`"; fresh empirical testing disproves the exception — `ECO <> "B10"` matches the game whose real ECO (`C50`) differs, but does **not** match the game with no ECO tag at all. A missing tag never matches, with no `<>` carve-out. |

Filters that need no dedicated fixture reuse existing ones: standard-start-vs-SetUp/FEN reuses `valid/setup-fen.pgn` (2 non-standard-start games) against `valid/single-game.pgn`/`valid/multi-game-results.pgn` (standard-start); "filters combined with cleanup and dedup in one job" reuses `duplicates/order-a.pgn`/`order-b.pgn` (a real duplicate pair) and `valid/long-comment.pgn` (a comment to strip) alongside `players-results-dates.pgn`.

## golden/

Inputs reserved for architecture.md §20.2/§20.4's golden command/output
tests. See `golden/README.md` for why expected *output* files are not
included yet (they must come from actually running the pinned engine,
which is Phase 1+ scope, not from hand-authored guesses).

- `merge-source-a.pgn`, `merge-source-b.pgn` - a matched pair for a
  "merge two files" golden test, sharing one identical game across both
  files on purpose so a merge+dedupe golden test has a real cross-file
  duplicate to exercise.
