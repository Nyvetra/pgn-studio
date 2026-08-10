# fixtures/golden/

Inputs for the "golden command" and "golden output" tests described in
architecture.md §20.2 and §20.4.

## What exists now

- `merge-source-a.pgn` (2 games) and `merge-source-b.pgn` (2 games) - a
  matched pair for a "merge two files" golden test (architecture.md §20.2's
  first example). They deliberately share one byte-identical game
  (`Almeida, Ana` vs `Berg, Bjorn`, Round 1) so a merge-with-deduplication
  golden test has a real cross-file duplicate to exercise, alongside two
  files' worth of otherwise-unique games.
- `regex/` (Phase 0b) - **the first fixtures in this directory with real,
  generated `*-expected.pgn` oracle output**, produced by actually running
  the checksum-verified pinned `pgn-extract` sidecar
  (`scripts/build-pgn-extract.ps1`), per the policy below. They prove the
  platform regex engine behind `=~` (TRE on Windows) is correctly wired
  in, which upstream's own `test/` suite never exercises at all. See
  `regex/README.md`.

## What is deliberately NOT here yet

Expected/**golden output** files (the exact PGN the pinned `pgn-extract`
binary produces for a given command, byte for byte) are **not** included.
Producing them requires actually running the pinned sidecar
(architecture.md §10, Phase 1 "Engine adapter proof"), which does not exist
in this repository yet - Phase 0 only pins the source revision
(`engine-src/upstream.lock`). Hand-writing a "golden" output would mean
guessing pgn-extract's exact normalization/formatting behavior (move
spacing, line wrapping, tag ordering) instead of recording engine truth,
which directly contradicts architecture.md §4.3 ("Engine truth, UI
clarity" - the adapter must remain faithful to actual `pgn-extract`
behavior) and §20.2's own instruction to test real argument arrays and
outputs, not assumed ones.

**When Phase 1 lands:** run the pinned binary against these inputs with the
documented command for each supported workflow, capture its actual stdout
artifacts as `*.expected.pgn` files next to the inputs, and add the
argument-vector assertions described in architecture.md §20.2. Do not
hand-author `*.expected.pgn` content at that point either - generate it
from a real, verified-checksum run of the pinned engine, then treat it as
frozen until the pin changes.
