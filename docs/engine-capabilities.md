# Engine capabilities

PGN Studio bundles one specific, pinned build of `pgn-extract` as a sidecar
process and never modifies it (see `THIRD_PARTY_NOTICES.md` and
`engine-src/upstream.lock` for exact provenance). This document is a
human-readable record of what that pinned build can actually do, kept
honest by testing the real binary rather than trusting its `--help` text.
Every claim below was independently verified by the coordinator running the
real engine against purpose-built fixtures (see
`DECISIONS-LEDGER.md` D-007/D-013 and `src-tauri/tests/phase4_integration.rs`,
`phase5_filters_integration.rs`, `duplicate_integration.rs` for the tests
that prove each one), and several corrected an earlier design document's
assumptions. Where this document and the architecture design docs disagree,
this document — grounded in the running binary — is the one to trust.

## Identity

| Field | Value |
|---|---|
| Version string | `v26-06` (`argsfile.c:CURRENT_VERSION`) |
| Upstream | <https://github.com/kentdjb/pgn-extract> |
| Pinned commit | `e69e863b70f2fb8ed7916752db95e1c771daf4f0` |
| License | GPL-3.0-or-later |
| Windows regex | statically linked TRE v0.9.0 (BSD-2-Clause), MSVC-built |
| macOS regex | system libc `<regex.h>` (no TRE) |

PGN Studio verifies the bundled sidecar's identity twice before ever
running it on your files: once when the release is packaged (checksum
recorded in `checksums.json`), and once at every app startup (SHA-256 of
the installed binary compared against the pinned value, followed by a
`--version` probe). If either check fails, PGN Studio refuses to run any
job rather than run an unverified binary — see `errors::engine_tampered`/
`errors::engine_missing` and `commands::engine::get_engine_info`.

## Capability summary for the pinned build

Every one of these is a hard yes/no gate in PGN Studio's command compiler —
if a capability is `false`, the corresponding UI option is disabled with an
explanation rather than silently dropped or approximated:

| Capability | Supported |
|---|---|
| Duplicate detection (`-D`/`-d`) | Yes |
| Duplicate audit file (`-d`) | Yes |
| External (disk-backed) duplicate table (`-Z`) | Yes |
| Check-file / "new games against master" (`-c`) | Yes |
| ECO classification (`-e`) | Yes |
| FEN pattern filters | Yes |
| Textual opening-line filters (`-v`) | Yes |
| `--fixresulttags` | Yes |
| `--nobadresults` | Yes |
| Separate broken-games output file | **No, structurally impossible** (see below) |
| Output notation | SAN only |
| Unicode (non-ASCII) file paths | Yes, verified via the embedded UTF-8 manifest |

## The verified surprises

These five are not obvious from the engine's own documentation and each one
either changed an earlier design assumption or fixed a real bug during
development. If you take away nothing else from this document, take away
these.

### 1. Duplicate identity is move-based — header differences do not matter

Two games with a byte-identical sequence of moves are duplicates of each
other, even if every header tag (player names, event, site, date, round)
differs completely. Conversely, identical headers do not make two games
duplicates if even one move differs. Full detail, including which copy is
kept and why, is in `duplicate-semantics.md`.

### 2. There is no separate file for broken games

You might expect a "games with errors" output alongside the unique-games
and duplicates-audit files. It does not exist, and cannot, with this engine
build. There is exactly one broken-games-related flag, `--keepbroken`, and
it has exactly two states:

| | Without `--keepbroken` | With `--keepbroken` |
|---|---|---|
| A game missing its result marker (or otherwise structurally broken) | Silently dropped from the output entirely | Included in the **main** output, alongside good games |

There is no third option that routes broken games to their own file in one
pass. PGN Studio's `BrokenOutput` setting therefore only offers two values —
Discard or Keep in Main Output — and the UI states plainly: "Games with
errors are reported in the log; they are not extracted to a separate file."
Any wording that implies otherwise is a bug.

### 3. The broken-games count cannot be measured, and is never shown as zero

It's tempting to compute "how many games were broken" as (games in the
input) minus (games matched, from the engine's summary line). This was
tried and disproven: a broken game can be **invisible to the engine's own
accounting**, not merely excluded from it. In one verified case, a 3-game
file with the last game missing its result marker made the engine report
"2 games matched out of 2" — a clean-looking ratio that hides the fact a
third game existed and was silently dropped. Subtracting would have
reported **zero** broken games when one was actually dropped, which is
worse than not reporting a number at all: it would confidently assert a
wrong answer instead of admitting an unknown one.

For this reason, PGN Studio's `broken_games` metric is unconditionally
`None`/"Not available" — never computed, never estimated, never defaulted
to zero. This is a deliberate application of the project's broader rule
that an unmeasured value is shown as unknown, never as a placeholder zero.

### 4. Text tags support prefix and not-equal only — never `=` or a relational operator

This is the most consequential surprise: it caused a real, ship-blocking
bug (every "Result" filter checkbox compiled to `Result = "1-0"`-style
criteria and silently matched **zero games** on every job that used it,
while reporting success). The fix is now enforced structurally, not just by
convention, but the underlying engine behavior is worth understanding:

| Criterion on a non-numeric tag (Result, Site, White, Black, Event, ECO, ...) | Behavior |
|---|---|
| No operator (prefix / "starts with") | Works correctly |
| `<>` (not-equal) | Works correctly |
| `=` (equality) | **Silently matches nothing** |
| `<`, `<=`, `>`, `>=` | **Silently matches nothing** |
| `=~` (regular expression) | Works correctly (POSIX BRE, TRE on Windows) |

None of these failure modes produce an error — the job succeeds, reports a
game count, and simply contains none of the games you asked for. This is
exactly the kind of silent-weakening failure the project treats as
unacceptable. `criteria.rs`'s `ensure_relational_op_safe_for_text_tag`
rejects `=`/`<`/`<=`/`>`/`>=` on every non-numeric tag before the job can
even start, with an error naming the field.

Relational and equality operators work normally on genuinely numeric tags
(Elo, Round, and `Date`, which has its own encoding — see below).

**What PGN Studio's own Filters screen actually exposes:** the built-in
filter UI never lets you choose an operator directly — it always compiles
name/result filters to the safe prefix form, and the ECO "Exclude" checkbox
is the only UI path to `<>`. Regular-expression matching (`=~`), while
supported by the engine and by the underlying data model, is not exposed as
a filter-builder option in this version; there is no advanced free-text
criteria editor in V1.

### 5. `Date` bounds need a full date, not just a year

`Date` criteria are encoded internally as `YYYY*10000 + MM*100 + DD`, with
a missing month/day defaulting to `01`/`01`. That default makes a bare year
dangerously ambiguous for anything except a clearly-directional bound:

- `Date >= "1999"` unambiguously means "on or after January 1st, 1999" — a
  bare year is safe here, and PGN Studio expands it to `1999.01.01`
  automatically.
- `Date <= "1999"` is the trap: naively left as `"1999"`, the engine's
  defaulting rule reads it as `<= 1999.01.01`, which **excludes every game
  from February through December of 1999** — the opposite of what "up to
  and including 1999" means to a person. PGN Studio expands this case to
  `1999.12.31` instead, so the upper bound actually means what it looks
  like it means.
- For any other operator, a bare year is rejected outright with an
  explanatory error rather than guessed at — there is no single correct
  expansion for, say, an exact-match or not-equal comparison against "the
  year 1999" alone.

PGN Studio's Filters screen only ever collects a year for its date-range
control and expands it to a full date using exactly this rule before it
ever reaches the engine, so you will not encounter the raw error in normal
use — this section documents the underlying constraint for anyone
inspecting the generated criteria file or extending the filter builder.

## Other verified engine behavior worth knowing

- **`-d` and `-D` are mutually exclusive.** Passing both is an immediate,
  loud failure (exit code 1) in either order — never a silent
  "last one wins." PGN Studio's compiler only ever emits one or the other.
- **The ECO file flag must use the attached form** (`-e<path>`, not
  `-e <path>` as two tokens). The separated form usually fails loudly
  (`Unable to open the ECO file eco.pgn.`, empty output, exit 1) — but a
  genuinely silent failure mode exists too: if a file literally named
  `eco.pgn` happens to be reachable through the engine's own fallback
  search (an `$ECO_FILE` environment variable, or the current working
  directory), a separated-form invocation could succeed while silently
  classifying against the *wrong* ECO data. PGN Studio always uses the
  attached form for every output/input flag it emits (`-o`, `-d`, `-t`,
  `-e`, `-c`) and strips `ECO_FILE` from the engine's environment, closing
  both failure modes.
- **`--maxmoves` must be emitted before `--minmoves`.** The engine stores
  move bounds ply-encoded but compares them against the raw incoming move
  count during validation, which silently drops the upper bound when
  `max < 2*min - 1` and the flags are emitted in the "obvious" min-then-max
  order — a 30-move game can pass what looks like a 10–15 move filter, with
  no error printed. PGN Studio's compiler always emits `--maxmoves` first,
  and a regression test specifically uses fixture values inside the
  trigger zone (not just any min/max pair) so a future accidental reordering
  would be caught rather than passing by coincidence.
- **Unicode file paths work**, but only because the bundled sidecar embeds
  a manifest fragment declaring UTF-8 as its active code page. Without it,
  the engine cannot open non-ASCII paths at all. This is a build-time
  property of the sidecar, not something any job configuration affects.
- **A strict prefix of a game's move sequence is never treated as a
  duplicate** of the complete game, even though both "start the same way."
  Only a complete, exact match of the full move sequence counts.

## What is genuinely not supported

- A separate output file for broken/error games (see above — structural,
  not a missing feature).
- Any output notation other than SAN (the engine's own default; V1 never
  requests a different one).
- Equality or relational filtering on any non-numeric tag (see above).
- An advanced/free-text criteria editor exposing the engine's full filter
  grammar (including `=~`) directly — V1's Filters screen only compiles a
  fixed, curated set of controls to safe criteria forms.
