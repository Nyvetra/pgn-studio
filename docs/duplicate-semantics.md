# Duplicate semantics

This document explains exactly what PGN Studio means by "duplicate," how it
decides which copy to keep, and what the duplicate-annotation warning does
and does not tell you. Every claim below was verified by running the real,
pinned `pgn-extract` engine against purpose-built fixtures — see
`fixtures/duplicates/` and `src-tauri/tests/duplicate_integration.rs` for the
tests that prove it. Where this document and casual intuition might
disagree, the intuition is usually the thing that's wrong.

## What counts as a duplicate

Two games are duplicates when they have the **same sequence of moves** —
nothing else.

Header tags (`Event`, `Site`, `White`, `Black`, `Round`, `Date`, ...),
comments, NAGs, and variations play **no part** in duplicate identity. Two
games with byte-identical move scores but completely different players,
events, and dates are still flagged as duplicates of each other. Conversely,
two games with identical headers but even one different move are not
duplicates.

This surprises people who expect "duplicate" to mean "the same game
recorded twice" in the everyday sense (same players, same event). It
specifically does not mean that here — it means the same sequence of chess
moves, full stop. If you need to distinguish "the same players played this
line twice" from "someone re-entered this exact game under a different
event name," you'll need to inspect the audit file yourself (see below);
PGN Studio does not make that distinction for you.

A game that is a **strict prefix** of another (e.g. one score stops at move
20, the other continues to checkmate) is *not* treated as a duplicate of
the longer game. Only an exact, complete match of the move sequence counts.

## Keep-first, and why input order matters

When you enable duplicate handling, PGN Studio always keeps the **first**
copy of a duplicate and diverts every later copy to a separate audit file.
"First" means first in the order your input files are listed on the Files
screen — not file modification time, not alphabetical order, not which file
happens to have "better" annotations.

This is why the Files screen lets you reorder your sources with Move
Up/Move Down controls, and why it shows an explanation of this rule
whenever duplicate handling is turned on: **the order you put your files in
directly decides which copy of every duplicate survives.** If file A is
listed before file B, and both contain a copy of the same game, the copy
from A is kept and the copy from B is diverted — even if B's copy has more
comments, a better ECO tag, or was added more recently.

There is no "keep the more complete copy" or "keep the more annotated
copy" mode. PGN Studio does not attempt to judge which copy is better, and
nothing in the UI should ever claim that it does — if you see wording that
suggests otherwise, that is a bug, not an intended behavior. (This is also
why the annotated-duplicate warning below is worded so carefully: it is
designed specifically not to imply a quality judgment it cannot make.)

### The two duplicate-handling modes

| Mode | What happens to duplicates | Audit file |
|---|---|---|
| Keep first, save an audit file | Diverted to `<base>.duplicates.pgn` | Yes |
| Keep first, discard the rest | Discarded entirely, not written anywhere | No |

These two modes are mutually exclusive at the engine level — the underlying
engine flags they compile to (`-d` and `-D`) cannot be combined, and PGN
Studio's compiler never attempts to. You get one or the other, never both,
and never neither once duplicate handling is turned on at all.

### The audit file is not "broken games" or "everything extra"

`<base>.duplicates.pgn` contains **only** the diverted later copies of
duplicate games — nothing else. It is a distinct artifact from the log file
and from any validation/error reporting. See `engine-capabilities.md` for
why there is no separate file for games with structural problems (a
different limitation, often confused with this one).

By default, an empty audit file (no duplicates were actually found) is not
written at all — you won't find a zero-byte `<base>.duplicates.pgn`
cluttering your output folder from a job that had nothing to divert.
Turning on "Always create audit artifacts" changes this: the file is
written even when empty, which is useful if you have automation downstream
that expects the file to always exist.

## The metrics trap: "games matched" is not "games in the output"

The engine's own summary line reports how many games it *matched and
processed*, which includes every diverted duplicate — a duplicate is a
"match," it's just routed to a different file. PGN Studio never derives the
`output_games` metric from that summary line for this reason: doing so
would overstate how many games actually landed in your main output. Instead,
`output_games` (when available at all) is computed by counting games in the
actual published main-output file. If you ever see the log mention a larger
"games matched" number than the metrics panel's "Output games" count, this
is why — it is not a bug, and the gap is exactly the number of diverted
duplicates (plus, in default mode, any broken games — see
`engine-capabilities.md`).

## The annotated-duplicate warning, and its limits

After a job that produces a non-empty duplicates-audit file, PGN Studio
scans that file for comments, NAGs (numeric annotation glyphs), and
variations. If it finds any, you get a warning along these lines:

> N suppressed duplicate games in the duplicates audit file contain a
> comment, NAG, or variation that the kept copy may not have. PGN Studio
> always keeps the first copy in your input order and does not judge which
> copy is "better" — open the audit file to review what was set aside.

Read this warning precisely, because it is deliberately narrow:

- **It does not mean the kept copy is missing anything in particular** —
  only that *some* diverted copy somewhere in the audit file has *some*
  annotation. The kept copy might already have equivalent or better
  annotations of its own; this warning has no way to know either way, and
  does not claim to.
- **It never re-ranks or re-selects which copy is kept.** Seeing this
  warning does not change the outcome of the job in any way. Input order is
  still what decided retention, before the warning was ever computed. The
  warning is purely informational, so you know to go look if you care.
- **The example list is capped at 5 games**, even if far more have
  annotations. The warning's count (`N`) is always the true total, but the
  named examples ("Examples: ...") stop at 5 and say "and N more" beyond
  that.
- **The scan itself is capped at 200,000 games** in the audit file. If your
  duplicates file is larger than that, the warning is appended with "(Only
  part of the audit file was checked for annotations.)" — the count and
  examples you see cover only the scanned portion, not necessarily the
  whole file.
- **If the scan itself fails** (an I/O error reading the audit file back),
  you get no warning at all, silently — the job's real outcome does not
  depend on this advisory check succeeding, and there is no honest "unknown"
  state to show for "we couldn't check." This is a deliberate, narrow
  exception to PGN Studio's usual "never show zero/nothing for something
  unmeasured" rule: the annotation check is explicitly advisory, not a
  metric, so its absence is not itself misleading the way a fabricated
  metric would be.

If this warning appears and you want to know exactly which annotations were
set aside, open `<base>.duplicates.pgn` directly — it's a normal PGN file.

## What this document does not cover

- How to configure duplicate handling from the UI — see `user-guide.md`.
- What "broken" games are and why there's no separate file for them (a
  related but distinct limitation) — see `engine-capabilities.md`.
- Text-tag filter operators and why `=` silently fails on most tags — see
  `engine-capabilities.md`.
