<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Decisions ledger

Durable record of the project-level decisions that source comments, docs,
and `engine-src/upstream.lock` cite by ID (`D-###`) rather than restate.
A decision belongs here when it constrains work across more than one file
and would otherwise have to be re-derived — or, worse, silently
re-litigated — every time someone touches an affected area.

`V-#` IDs appearing alongside these are *verification* findings: specific,
empirically-established facts about the pinned engine's behaviour (e.g.
flag-order hazards) established by running it, not decided. They are
namespaced under the decision that commissioned them and are cited that
way — `src-tauri/src/domain/operations.rs:56` writes "D-007 V-1/V-2".

> **Two different `D-` series exist, and they collide.** IDs written with
> three digits (`D-006`) are *this ledger's*. IDs written with one or two
> (`D-1`, `D-6`, `D-13`, `D-17`, `D-20`) belong to **design-02** and are
> always cited with that prefix — `design-02 §4.3/D-17`. They are
> unrelated: design-02's `D-6` is not this file's `D-006`. When citing,
> keep the three-digit form and the `design-02 §…/` prefix respectively,
> or the two become impossible to tell apart.

## The rule this ledger's header is cited for

Several modules cite "DECISIONS-LEDGER.md header" for a single standing
rule, referred to in-tree as the **never-invent-never-placeholder rule**:

> Where the source material available for a task does not establish a
> value, name, or behaviour, leave the gap explicit and documented.
> Never invent a plausible value, never fill a gap with a placeholder,
> and never widen a claim beyond what a citation actually supports.

It is load-bearing, not aspirational. Working examples in the tree:

- `src-tauri/src/domain/filters.rs:72` — the `TagName` enum stops at the
  tags a source citation covers; inventing the remaining ~31 names to
  round out the set was rejected.
- `src-tauri/src/domain/operations.rs:140` — output notations whose exact
  `-W<fmt>` spelling was not in the available citations are deliberately
  not enumerated.
- `src-tauri/src/engine/sidecar.rs:185` — the self-test embeds one real
  reviewed fixture rather than a second invented game.
- `src-tauri/src/engine/capability.rs:19` — pin values are embedded via
  `include_str!` from build output rather than hand-copied, so they
  cannot drift.
- `engine-src/upstream.lock` — the `x86_64-apple-darwin` entry records no
  artifact hash, because none was captured from the run that would have
  supplied it.

## Provenance of this file

**This file was created on 2026-08-11, after the decisions it records.**
`DECISIONS-LEDGER.md` was cited from at least 20 files across the repo —
Rust source, docs, fixtures, workflows, and `upstream.lock` — but had
never actually been created. Every citation pointed at a file that did
not exist.

D-006 below is therefore a **reconstruction**, assembled from those
citations rather than transcribed from an original. Each claim it makes
is followed by the `file:line` that attests to it, so the reconstruction
can be audited against the tree instead of trusted. Where a citation
quotes the original ledger directly, that is marked as a quotation;
everything else is a paraphrase of what the citing code asserts D-006
says.

Consistent with the rule above, the other cited IDs are **not**
reconstructed here — see "Cited but not recorded" at the end. Their
original text is not recoverable from the citations alone, and inventing
entries to fill out the numbering would be exactly the failure this
ledger's header rule exists to prevent.

---

## D-006 — No Mac and no signing credentials; how unverifiable platform work is handled

**Status:** active, amended twice on 2026-08-11 (see below). The two
physical constraints and the handling rule are unchanged; what has changed
is how much of the macOS gap CI has since closed. Read both amendments
before citing this entry — the first is now partly historical.

### Decision

Two physical constraints bound this project, and are not defects to be
fixed casually:

1. **No Mac is available to the development environment.**
2. **No code-signing or notarization credentials are available** —
   neither an Apple Developer ID Application certificate nor a Windows
   Authenticode certificate.

*(`docs/acceptance-criteria.md` — "the two physical constraints repeated
throughout this project".)*

The decision was not merely to note these, but to fix **how work that
cannot be verified because of them is written and reported**:

- Platform code that cannot be exercised here is still written, in good
  faith, against documented behaviour and real source — not stubbed out,
  and not guessed at. `src-tauri/src/filesystem/platform/mod.rs:12-18`
  writes the Unix branch directly against `libc` crate source read for
  the task, with exact signatures "confirmed against the vendored `libc`
  source, not guessed".
- It is then **reported honestly as unverified**. `platform/mod.rs:16`
  quotes this ledger directly: D-006 sets the precedent that such work
  *"must be reported honestly as unverified, never as passing."*
- Where verification is impossible and a guess would be risky, the
  honest resolution is to leave the behaviour **unimplemented and
  documented** rather than approximated.
  `src-tauri/src/filesystem/identity.rs:71-79` leaves macOS NFC
  normalization undone for exactly this reason: "guessing at ICU-grade
  NFC normalization without any way to check it against a real
  HFS+/APFS volume was judged riskier than clearly leaving it
  unimplemented."
- Skips are **documented and non-silent**.
  `src-tauri/src/filesystem/folder_scan.rs:545-553` cites D-006 as "this
  project's established pattern" for emitting a visible skip message
  rather than quietly passing a test that did not run.

### Consequences

- Only `x86_64-pc-windows-msvc` is built and verified as a shipping
  target (`src-tauri/src/engine/sidecar.rs:73`).
  `src-tauri/src/engine/capability.rs:23-27` embeds the Windows
  `build-info-*.json` unconditionally for the same reason, with the
  `cfg!`-based multi-target selection left as an explicit future change.
- macOS CI legs exist as real, executable job definitions but are marked
  `unverified: true` / `continue-on-error` and named accordingly, in all
  three workflows.
- Signing and notarization steps are real but inert, gated on repository
  secrets that do not exist. See `docs/release-process.md`.

### Amendment, 2026-08-11 — the macOS engine has now been verified on real hardware

Superseded: the blanket claim that the macOS toolchain "has never been
run." It has. GitHub Actions `macos-14` and `macos-15-intel`, workflow
run [31497585623](https://github.com/Nyvetra/pgn-studio/actions/runs/31497585623)
(commit `a02aeb1`).

**Established.** `scripts/build-pgn-extract.sh` works on both
architectures: Apple clang 15.0.0 compiles and links the pinned commit,
the `--version` smoke check passes, and the sidecar installs
(`aarch64-apple-darwin`: 214680 bytes, sha256 `83c4eae6...`).
`scripts/verify-engine.ps1` then passes Layer 0 (pin provenance), Layer 1
(identity) and **Layer 2 at 76/76** — pgn-extract's entire own upstream
regression suite — against that binary.

That first run also surfaced two real defects, which is precisely what
D-006's "treat the first real run as verification, not a formality"
instruction was for: the script was committed mode `100644`, so CI could
not execute it at all, and its `--version` capture read stdout only while
pgn-extract writes `--version` to stderr. Both fixed.

**Not established at that point (all but one has since changed — see the
second amendment below):**

- **Layer 3 reported 0/6 on macOS — a fixture artifact, not an engine
  defect.** Layer 3 compares output byte-for-byte against
  `fixtures/golden/regex/*-expected.pgn`, which are stored CRLF and
  pinned that way by `.gitattributes` (`fixtures/** -text`), while the
  macOS engine emits LF. Stripping `\r` from each committed golden
  reproduces the macOS run's *actual* SHA-256 exactly, in all six cases —
  so macOS output is content-identical to Windows for literal, anchors,
  bracket, star, backreference, and the `grammar.c` odds call site.
  Apple's libc regex matched TRE on every supplemental case. Making this
  pass needs a newline-normalizing comparison, not an engine change.
- **macOS reproducibility is unmeasured.** The
  `-D__DATE__`/`-D__TIME__`/`-Wno-builtin-macro-redefined` and
  `-Wl,-no_uuid` flags are applied and did not break the build, but no
  two-build comparison has been run there. Do not extend the Windows
  `/Brepro` evidence to macOS.
- **The Rust crate does not compile on macOS at all**, so **no macOS
  application bundle has ever been produced.** A working sidecar is not a
  shippable macOS release.
- **Both original constraints stand unchanged:** still no Mac available
  to the development environment, and still no signing or notarization
  credentials. CI runners closed part of the verification gap; they did
  not close the credential gap at all.

### Amendment 2, 2026-08-11 — a macOS application bundle now exists

Three of the four bullets above are superseded. Established by workflow run
[31533789885](https://github.com/Nyvetra/pgn-studio/actions/runs/31533789885)
(commit `c43133d`), with the Rust half established by run
[31527446775](https://github.com/Nyvetra/pgn-studio/actions/runs/31527446775).

- **Layer 3 now passes on macOS.** `verify-engine.ps1` compares
  byte-exact first and, only on failure, retries with CRLF/LF normalized —
  reported as `[PASS~]`, counted separately in the summary, and recorded
  as `passedAfterNewlineNormalization` in the JSON report. Byte-exactness
  is unweakened where it already held: the Windows run still reports all
  six as byte-exact and the fallback never fires there.
- **The Rust crate compiles and its full test suite passes on macOS**,
  on both `aarch64` and `x86_64` — 234/234 unit tests plus every
  integration binary, with `clippy --all-targets -D warnings` clean.
  `engine::capability` selects `build-info-<triple>.json` by `cfg`, and
  the golden/compiler fixtures no longer assume Windows-shaped paths.
- **macOS application bundles are now produced on both architectures.**
  `Engine and Bundle / macos-14` ran green end to end for the first time
  and uploaded `pgn-studio-macos-14-unsigned` (9,531,587 bytes);
  `macos-15-intel` has since done the same, uploading
  `pgn-studio-macos-15-intel-unsigned` (9,716,792 bytes) in run
  [31535290496](https://github.com/Nyvetra/pgn-studio/actions/runs/31535290496).

**Still open, and the reason this is not "macOS is done":**

- **Nobody has ever launched that bundle.** It was produced by CI and
  uploaded as an artifact. "Builds and packages" is not "runs correctly",
  and no Mac is available here to check. Treat the first real launch as
  verification, exactly as D-006 said to treat the first real build.
- **DMG packaging on `macos-15-intel` is intermittent — demonstrated, not
  inferred.** In run **31533789885 attempt 1** the release binary
  compiled and `PGN Studio.app` bundled, and the run then failed in
  `bundle_dmg.sh` producing `PGN Studio_0.1.0_x64.dmg` — a *packaging*
  failure, not a build failure. That same job was then re-run with **no
  change of any kind**, and **attempt 2 of the same run, on the same
  commit `c43133d`, succeeded** and produced the Intel bundle. Identical
  inputs, opposite outcomes: that is what rules out a code defect here,
  and it is stronger evidence than the two same-step-different-commit
  observations that follow it (runs 31535290496 and 31536632395, both
  green). `macos-14` packaged fine throughout.

  **Cite the attempt, not just the run.** A GitHub re-run *replaces* the
  failed attempt, so run 31533789885 today reports `completed/success`
  with all three jobs green at attempt 2. Following that run ID expecting
  to find the failure will show a fully green run and make this entry look
  wrong. The failure exists only in attempt 1's logs.

  The behaviour matches a documented GitHub Actions runner-image problem
  rather than anything in this project: `hdiutil` intermittently fails
  DMG creation with "Resource busy"
  ([actions/runner-images#7522](https://github.com/actions/runner-images/issues/7522),
  and repeatedly against Tauri, e.g.
  [tauri-action#801](https://github.com/tauri-apps/tauri-action/issues/801)).
  Attempt 1's cleanup reported an orphaned `diskimages-helper`, which is
  `hdiutil` still holding the image. Expect this leg to fail occasionally;
  a red DMG step is not by itself evidence of a defect here — re-run it
  before investigating.

  Since `534dd6f` the macOS bundle step retries up to three times and
  passes `--verbose`, so `bundle_dmg.sh`'s own output now reaches the log.
  Before that Tauri swallowed it entirely and a failure logged no cause at
  all, which is why attempt 1's cause had to be inferred from a process
  name in the runner's cleanup output. Three consecutive failures are
  therefore no longer explainable as this flake, and should be read as a
  real regression.
- **macOS reproducibility is still unmeasured.** Unchanged from the first
  amendment: no two-build comparison has been run there.
- **Both original constraints stand entirely unchanged.** Still no Mac
  available to this development environment, and still no signing or
  notarization credentials. The bundle above is genuine and unsigned.
  CI runners have now closed most of the *verification* gap; they have
  closed none of the *credential* gap.

`engine-src/upstream.lock` records this as `"verified": "builds"` for both
darwin toolchains — deliberately the string `"builds"`, not `true`.

---

## D-014 — Repository reorganization: doc relocation, `.github/` policy files, toolchain pinning

**Status:** active.

> **On this ID.** Numbered D-014 — above every ID known to be in use — and
> not D-011, deliberately. Citations in the tree reach D-013, so the
> original ledger almost certainly also used D-011 and D-012; they are
> simply not cited by any surviving file. Reusing a number that a lost
> entry may already hold would silently create two different D-011s, which
> is precisely the citation drift the provenance note above exists to
> prevent. When the earlier entries are reconstructed, expect gaps at
> D-001, D-003–D-005, D-011 and D-012 rather than assuming they were free.

### Decision

An approved repository reorganization moved five root-level files with
history preserved (`git mv`), added two new pinning files, and closed a
gitignore gap that only worked on one machine:

- **The root-level architecture document (formerly named with a
  redundant `PGN-Studio-` prefix) moved to `docs/architecture.md`,
  renamed to match.** This file's own §8 (this section) prescribes the
  filename `architecture.md`, and the ~424 existing short-form citations
  across the tree already read "architecture.md §N" with no path, so this
  move made the great majority of citations correct rather than requiring
  them to be edited. Only the handful of citations that spelled out the
  old, prefixed filename verbatim, or linked to it by relative path,
  needed a change — see Evidence.
- **`DECISIONS-LEDGER.md` → `docs/DECISIONS-LEDGER.md`.** This file's own
  ~42 citations across the tree are almost entirely by name
  ("DECISIONS-LEDGER.md D-007"), not by path, so the move did not require
  editing them either. The filename itself did **not** change, unlike
  `architecture.md` above.
- **`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md` → `.github/`.**
  GitHub recognizes all three identically whether they sit at the repo
  root or under `.github/`; consolidating them there declutters the root
  without losing any platform behavior (auto-linking from the Issues/PRs
  UI, the community-health-files tab, etc.).
- **New `rust-toolchain.toml` (repo root).** Pins the exact Rust `stable`
  channel this project was built and verified against, plus the
  `rustfmt`/`clippy` components. Without a pin, two development machines
  (or a machine and CI) can silently disagree on which lints `cargo
  clippy -- -D warnings` enforces, since clippy's lint set changes across
  stable releases — see the file's own header comment.
- **New `package.json` `engines.node` field (`>=24`).** Documents, in the
  one place a Node tool would actually check it, that CI pins Node 24; no
  `.nvmrc` was added since `engines` already states the constraint.
- **`.gitignore` gains `.claude/worktrees/`.** This directory holds full,
  separate checkouts of this repository (each with its own `.git`,
  `node_modules`, and built sidecar). It was previously excluded only via
  the primary clone's local `.git/info/exclude`, which is machine-local
  and does not travel with the repository — so on a second machine,
  `git add -A` would attempt to commit an entire embedded checkout.
  `.claude/` itself is deliberately **not** ignored wholesale:
  `.claude/skills/` and `.claude/launch.json` remain tracked.
- **This section (§8) was amended in the same change** that performed the
  moves above, so the repository-layout tree it shows is never out of
  date with respect to its own reorganization — see Evidence.

### Evidence

- `docs/architecture.md` §8 (this section) — the amended tree now shows
  `docs/architecture.md`, `docs/DECISIONS-LEDGER.md`, `.github/CONTRIBUTING.md`,
  `.github/CODE_OF_CONDUCT.md`, `.github/SECURITY.md`, `rust-toolchain.toml`,
  and `engine-src/eco-json/`, and no longer shows any of those files at
  the repository root nor a root `data/` directory (moved to
  `engine-src/eco-json/` in an earlier, separate change).
- `rust-toolchain.toml:1-11` — the pin and its rationale comment.
- `package.json` — `engines.node` field.
- `.gitignore` — the `.claude/worktrees/` block and its comment explaining
  the local-exclude gap.
- `.github/CONTRIBUTING.md` — the new "macOS" development subsection
  (alongside the existing "Windows" one), covering the Xcode Command Line
  Tools prerequisite, the PowerShell 7 requirement for
  `scripts/verify-engine.ps1`, the sidecar-before-Rust build ordering
  shared by both platforms, the `ENGINE_TAMPERED` rebuild-ordering hazard,
  cross-machine sidecar hash differences being expected (cross-referencing
  `engine-src/README.md`'s "What `/Brepro` does not fix"), `.gitattributes`
  already forcing LF so `core.autocrlf` needs no per-machine fix, and
  `git update-index --chmod=+x` for `.sh` files authored from Windows.
- `README.md`, `.github/CONTRIBUTING.md`, `docs/user-guide.md`,
  `docs/README.md`, `docs/acceptance-criteria.md`,
  `.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/feature_request.md`,
  `.github/SECURITY.md` — the nine sites across eight files that spelled
  out the old, `PGN-Studio-`-prefixed filename verbatim and were updated
  to `architecture.md` (or a corrected relative link) as part of this
  move.

### Consequences

- Root markdown is now exactly `README.md` and `THIRD_PARTY_NOTICES.md`;
  every other project markdown file lives under `docs/` or `.github/`.
- Any future citation of `architecture.md` or `DECISIONS-LEDGER.md` by
  name (the repository-wide convention) continues to resolve without a
  path, since neither citation style encodes a directory. A citation that
  needs an actual link must point into `docs/`.
- A future `.github/CONTRIBUTING.md` edit that discusses either OS-specific
  setup path should keep the "Windows" / "macOS" subsections symmetric
  rather than letting one drift out of date while the other is updated.
- `git status --short` for this change reports the five moved files as
  renames (`R`), not delete+add, because every move used `git mv`.

---

## Cited but not recorded

These IDs are cited in the tree and have no entry here. They are listed
so the gap is visible and so each can be reconstructed from its citation
sites by someone with the original context — not invented.

| ID | Subject, as far as the citations reveal | Cited from |
| --- | --- | --- |
| D-002 | TRE build-recipe deviation (pre-approved fallback to direct `cl.exe` compilation of `lib/*.c`) | `engine-src/patches/README.md`, `engine-src/patches/tre-msvc/README.md`, `engine-src/upstream.lock` |
| D-007 | Engine capability map — which pgn-extract flags are supported, source-cited | `docs/engine-capabilities.md`, `fixtures/README.md`, `src-tauri/src/domain/{capability,filters,operations}.rs`, `src-tauri/src/engine/{capability,command_compiler,golden_tests}.rs`, 3 integration test files |
| D-008 | `eco.pgn`/`COPYING` true-upstream-LF-bytes hazard (`core.autocrlf` silently changes hash/size) | `engine-src/upstream.lock` |
| D-009 | Windows UTF-8 manifest / non-ASCII path support. Carries a verification note quoted as "Bengali filenames AND Bengali directory names work", and records that `--help`'s banner contains a build-date placeholder, so `--help` is never parsed for engine identity | `src-tauri/src/engine/sidecar.rs:21,265,398` |
| D-010 | Tag-filter `<>` (not-equal) semantics and the ECO methodology used to establish them | `fixtures/README.md`, `src-tauri/src/engine/criteria.rs`, `src-tauri/tests/phase5_filters_integration.rs` |
| D-013 | Cited jointly with D-007 as the record of capability claims verified by running the real engine against purpose-built fixtures — several of which corrected an earlier design document | `docs/engine-capabilities.md:10` |
| V-1, V-2 | Findings under D-007: how games whose move sequence/hash repeats an earlier game in input order are treated | `src-tauri/src/domain/operations.rs:56`, `src-tauri/tests/duplicate_integration.rs` |
| V-3 | `--maxmoves`/`--minmoves` argument-order hazard (reversed order silently admits out-of-range games) | `fixtures/README.md`, `src-tauri/src/domain/filters.rs`, `src-tauri/src/engine/{command_compiler,golden_tests}.rs`, `phase5_filters_integration.rs` |
| V-4 | Regex/criteria-file verification finding | `fixtures/golden/regex/README.md`, `src-tauri/src/engine/command_compiler.rs`, `src-tauri/tests/phase4_integration.rs` |
| V-5 | Capability-map verification finding | `src-tauri/src/domain/{capability,operations}.rs`, `src-tauri/src/engine/capability.rs`, `phase4_integration.rs` |
| V-6 | PowerShell mangles attached-form flags (e.g. `-oPATH`) built by string interpolation; pass argument arrays, never shell strings | `fixtures/golden/regex/README.md`, `scripts/lib/engine-common.ps1` |
