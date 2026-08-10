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

## golden/

Inputs reserved for architecture.md §20.2/§20.4's golden command/output
tests. See `golden/README.md` for why expected *output* files are not
included yet (they must come from actually running the pinned engine,
which is Phase 1+ scope, not from hand-authored guesses).

- `merge-source-a.pgn`, `merge-source-b.pgn` - a matched pair for a
  "merge two files" golden test, sharing one identical game across both
  files on purpose so a merge+dedupe golden test has a real cross-file
  duplicate to exercise.
