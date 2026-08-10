## Summary

<!-- What does this change do, and why? -->

## Which phase/section of PGN-Studio-architecture.md does this relate to?

<!-- e.g. "Phase 1: Engine adapter proof" or "§11.4 Atomic output publication" -->

## Checklist

- [ ] I read the relevant section(s) of `PGN-Studio-architecture.md` before
      making this change.
- [ ] `npm run build`, `npm test`, and `npm run lint` pass locally.
- [ ] `cargo test`, `cargo clippy --all-targets -- -D warnings`, and
      `cargo fmt --check` pass locally (if Rust code changed).
- [ ] New/changed command-generation logic has a golden argument-vector
      test (architecture.md §20.2), not just a display-string test.
- [ ] No source PGN file can be opened with write access, and no
      transformation can overwrite a source file (architecture.md §4.1,
      §11.1), if this touches filesystem or engine-invocation code.
- [ ] No shell is invoked to run the `pgn-extract` sidecar or any other
      process (architecture.md §10.3, §16.2), if this touches process
      invocation.
- [ ] New source files have an SPDX header
      (`// SPDX-License-Identifier: GPL-3.0-or-later` or the language's
      comment equivalent).
- [ ] Any new third-party dependency's license is compatible with
      GPL-3.0-or-later.

## Testing performed

<!-- What did you actually run, and what was the result? -->
