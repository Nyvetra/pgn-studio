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

## D-002 — TRE static-linking requirement and the pre-approved MSVC direct-compile fallback

**Status:** inferred active — no citation states a status line for D-002.
The rule is still enforced today by `scripts/build-pgn-extract.ps1`'s
direct-cl-compile-of-lib recipe (matching `engine-src/upstream.lock`'s
recorded `buildMethod`) and by `engine-src/patches/` remaining empty of
source patches.

**Reconstruction note.** This entry replaces the placeholder row previously
carried in "Cited but not recorded" (this file, formerly line ~373). It is
assembled entirely from citations — no original ledger text for D-002
survives — following the same method as D-006 above: every claim is
followed by the `file:line` that attests to it, and quotations are marked
as such, everything else is paraphrase.

### Decision

Citing files attribute two distinct things to D-002, kept separate below
because the citations themselves distinguish them:

1. **A static-linking requirement.** `engine-src/patches/tre-msvc/README.md:24-26`
   paraphrases it directly: "The approved design (D-002 in the decisions
   ledger) requires TRE statically linked into the sidecar so the shipped
   binary has no runtime DLL dependency beyond `KERNEL32.dll`." TRE v0.9.0
   is the POSIX regex library `pgn-extract` needs on Windows: the `=~`
   tag-criteria operator "is exactly the code path that differs by
   platform: on Windows it is provided by the vendored, statically-linked
   TRE v0.9.0 ... on macOS it is the system libc"
   (`fixtures/golden/regex/README.md:6-9`), matching `upstream.lock:47-49`'s
   own record of the macOS side ("No third-party regex on macOS - Apple's
   libSystem provides POSIX `<regex.h>`"). D-002 is cited as the decision
   that on Windows specifically it must be linked as a static archive into
   the sidecar executable, not consumed as a runtime DLL.

2. **A pre-approved fallback build recipe**, invoked because TRE's own
   in-repo Visual Studio project is unusable as shipped at the pinned commit
   (`d0e0c997336b3210f05b3e1daa7bb5cb9900d274`, tag `v0.9.0`) for three
   concrete reasons documented in `tre-msvc/README.md:18-32`: its `.vcxproj`
   configures only the `Win32` (x86) platform, never `x64`, while PGN Studio
   ships `x86_64-pc-windows-msvc` (line 18); it builds a `DynamicLibrary`
   with a `tre.def` export table, not a static library — directly
   conflicting with requirement 1 above; and its `<ClInclude>` list
   references `lib/regex.h`, which does not exist at the pinned commit — the
   real header lives at `local_includes/regex.h` (line 29-31). Retargeting
   the `.vcxproj` in place was rejected as "a bigger and more fragile
   deviation than simply not using it" (`tre-msvc/README.md:34-38`), so
   "Per the pre-approved fallback in the design (D-002,
   design-01-engine-build.md §6.2, §9), this build instead compiles `lib/*.c`
   **directly** with `cl.exe`" (`tre-msvc/README.md:39`).

`engine-src/upstream.lock:43` records the same attribution inside the lock
file's `regex.windows.buildMethodNote` field: "Fallback recipe
(pre-approved, D-002) is documented in
engine-src/patches/tre-msvc/README.md" (quotation), and
`upstream.lock:42` records the method itself as
`"buildMethod": "direct-cl-compile-of-lib"`.

`engine-src/patches/README.md:5` independently attributes the
zero-source-edit consequence of the fallback to D-002: "(zero pgn-extract
source edits, per D-002 in the decisions ledger)." That same file's own
"`tre-msvc/` is not a source patch" section (`patches/README.md:9`) states,
in substance though without citing D-002 by ID again, that the README
"documents a **build-recipe deviation**, not a source diff," and explains
why the recipe lives under `patches/` at all despite touching no source: it
is "exactly the kind of 'how does the shipped binary differ from clone
upstream and run its own build system' disclosure" the patches policy
exists to capture, "even though no unified diff is involved"
(`patches/README.md:16-18`, paraphrase/quotation).

The recipe the fallback authorizes (`tre-msvc/README.md:46-127`, not itself
part of D-002's text but what it approved): (1) compile every glob-discovered
`lib/*.c` file with `cl /c /std:c11 /O2 /MT /W3 /DHAVE_CONFIG_H ...`,
resolving `HAVE_CONFIG_H` to TRE's own unmodified `win32/config.h`; (2)
archive the objects into `tre.lib` with `lib.exe` (no `.def` file, since no
DLL export table is needed); (3) stage three verbatim copies of TRE's own
headers (`local_includes/regex.h`, `local_includes/tre.h`,
`win32/tre-config.h`) into a synthesized install layout; (4) compile
pgn-extract's own unmodified `*.c` files against that staged `include/`, so
its two `#include <regex.h>` sites resolve to TRE's real
`regcomp`/`regexec`/`regfree` "exactly as D-002 requires"
(`tre-msvc/README.md:122`); (5) link against `tre.lib`, with the UTF-8
manifest embedded via `link /MANIFEST:EMBED`.

### Evidence the requirement was checked, not just recited

`tre-msvc/README.md:139-150` describes a Phase 0b scratch-directory
verification, run before the recipe was written into the tree: all 13
`lib/*.c` files compiled with only benign warnings; `pgn-extract.exe` linked
with zero pgn-extract source changes, ran, and printed `pgn-extract v26-06`,
exit 0; and `dumpbin /dependents` showed `KERNEL32.dll` only — described as
"identical footprint to the earlier stub-regex probe recorded in the
decisions ledger (D-002)" (lines 142-143). This sentence is the only
surviving indication that D-002 itself records more than the fallback
recipe — an earlier "stub-regex probe" with a measured KERNEL32.dll-only
footprint. `engine-src/README.md:60-63` places a flat-schema "Phase 0"
before "Phase 0b" (which added the TRE pin), consistent with a stub-regex
probe predating TRE's involvement, but no citation states this connection
outright — it is inference, not something any file asserts. Functional
correctness was also verified, not just linking: a two-game fixture with a
`White =~ "^F.*r$"` criteria file correctly selected only the matching game
(`tre-msvc/README.md:144`).

### Consequences

- `engine-src/patches/` holds zero source-code patches for either
  pgn-extract or TRE: `"patches": []` under both `engine` and
  `regex.windows` in `upstream.lock:29,44`.
- The recipe is registered in `upstream.lock`'s `regex.windows.buildMethod`
  / `buildMethodNote` fields, and both must be updated together with any
  future recipe change, per `patches/README.md` policy point 4, quoted in
  `tre-msvc/README.md:167` as "patches must be re-verified on every upstream
  pin update" (`patches/README.md:36`).
- If a future TRE upgrade breaks the recipe, `tre-msvc/README.md:152-163`
  directs first checking whether the new commit's `win32/tre.vcxproj` has
  been fixed (x64 static lib, valid `lib/regex.h`) and switching to it if
  so; only if the direct-compile fallback is still needed should the
  `lib/*.c` file list and scratch-directory verification be re-run before
  updating `upstream.lock`'s `regex.windows.commit`.
- On macOS the same static-link requirement is not applied the same way:
  `upstream.lock:46-49` records `regex.macos` as `system-libc-regex` /
  `linkage: "libc"`, using Apple's libSystem-provided POSIX `<regex.h>`
  rather than a statically-linked TRE, matching upstream's own Unix
  Makefile. No citation states whether this macOS choice is part of D-002
  or a separate decision — see Gaps.

### Note on `design-01-engine-build.md`

Eight citing sites across 4 files (`engine-src/README.md:63`,
`tre-msvc/README.md:4,39,87,97`, `fixtures/golden/regex/README.md:50`,
`scripts/verify-engine.ps1:4,376`) point at `design-01-engine-build.md`
§2.1, §5, §6.1-§6.4, and §9 as the source of the "pre-approved" fallback and
related build details. A repository-wide search finds no file of that name
anywhere in this tree (Glob `**/design*.md` and `**/design-01-engine-build.md`,
both empty). Like this ledger before its own reconstruction, that design
document is cited extensively but was apparently never captured in the
repository — its actual wording, and therefore the procedural meaning of
"pre-approved" (who approved it, when, against what alternatives), is not
recoverable from any citation.

### Gaps

The following are NOT established by any citation found in the repo, and are
deliberately left as gaps rather than guessed at:

- No date, author, or approval process for D-002 itself. Unlike D-006
  (created/amended 2026-08-11, per this ledger's own provenance note),
  nothing in the tree dates D-002.
- The exact original wording of D-002. Every claim about "what D-002
  requires/says" here is a paraphrase or short quotation embedded in a
  citing file (tre-msvc/README.md, patches/README.md, upstream.lock), never
  the ledger's own text, which is lost.
- The content of `design-01-engine-build.md` §2.1, §5, §6.1-§6.4, and §9 —
  cited from 8 sites across 4 files as the source of the "pre-approved"
  status and much of the recipe rationale, but the file does not exist
  anywhere in the current tree.
- Whether the "stub-regex probe" mentioned in `tre-msvc/README.md:142-143`
  ("identical footprint to the earlier stub-regex probe recorded in the
  decisions ledger (D-002)") was itself part of D-002's original decision, a
  separate finding D-002 merely cites, or something else entirely. Only that
  one sentence refers to it; no other file in the repo mentions a
  "stub-regex" probe, what it built, or when it ran. The connection to
  "Phase 0" (`engine-src/README.md:60-63`) is inference from timeline
  plausibility, not something any citation states.
- Whether TRE was chosen over alternative regex libraries as part of D-002,
  or in a separate, unrecorded decision — no citation discusses alternatives
  to TRE.
- Whether the macOS choice of system libc regex (`upstream.lock:46-49`,
  `regex.macos`/`system-libc-regex`) falls under D-002's scope or is a
  separate, uncited decision. The two are recorded in the same JSON `regex`
  object but nothing states they share a decision ID.
- git history in this worktree does not help: `git log --oneline --
  engine-src/patches/tre-msvc/README.md` shows only a single squashed
  commit (`e681b5f`, "Phase 0-1: repository scaffold, verified engine
  sidecar, and safety core"), so no incremental commit history narrows down
  when D-002 was decided relative to other work.

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

## D-007 — Engine capability map: which pgn-extract flags the pinned build supports, and why

**Status:** inferred active — no citation states an explicit status line for
D-007; every citing file treats it as currently governing behavior and
nothing suggests it was ever superseded, so "active" here is an inference
from usage, not a transcribed status line.

**Reconstruction note.** Per this file's "Provenance of this file" section,
the original ledger text is lost. Everything below is assembled from
citations, exactly as D-006 was. D-007 is the single most-cited ID in the
tree (17 files grep-hit it, versus 15 for D-006 — genuinely the most-cited
ID). It is also the namespace for verification findings, consistent with
this ledger's own header definition: "specific, empirically-established
facts about the pinned engine's behaviour ... are namespaced under the
decision that commissioned them" (this file's header, which names D-007
V-1/V-2 as its own worked example). Where a citation below quotes the
original ledger's own wording (as opposed to a citing file's paraphrase of
it), that is marked explicitly.

### Decision

The engine capability map is a hand-verified record of which flags the
pinned `pgn-extract` `v26-06` Windows sidecar actually supports, established
empirically rather than trusted from its own documentation:
`docs/engine-capabilities.md` states it was kept "honest by testing the real
binary rather than trusting its `--help` text" (docs/engine-capabilities.md:7),
verified "by the coordinator running the real engine against purpose-built
fixtures" and citing this ledger's D-007/D-013 alongside the specific test
files that prove each claim (docs/engine-capabilities.md:8-10). The same
rejection of `--help`-based inference is stated twice in source, nearly
verbatim: "Help-text parsing is explicitly *not* the contract"
(src-tauri/src/engine/capability.rs:6, echoed at src-tauri/src/domain/capability.rs:36-37)
— "every boolean below is a hand-verified fact from DECISIONS-LEDGER.md
D-007 and design-02 §1.3's source-cited flag table, not something inferred
from `--help` output at runtime" (src-tauri/src/engine/capability.rs:8-9).

The map is split across the codebase's usual domain/engine boundary:

- `domain::EngineCapabilities` (src-tauri/src/domain/capability.rs:41-80) is
  the version-agnostic *shape*, kept independent of any one pinned build so
  the command compiler doesn't hard-code assumptions about a specific
  engine version.
- `engine::capability::pinned_v26_06()` (src-tauri/src/engine/capability.rs:114-149)
  is the concrete, tested static map for the currently pinned build. Nine
  booleans are `true` (duplicate_detection, duplicate_audit_file,
  external_duplicate_table, check_file, eco_classification, fen_patterns,
  textual_variations, fix_result_tags, reject_bad_results —
  src-tauri/src/engine/capability.rs:117-125); `separate_broken_output` is
  `false` (line 126); `supported_output_formats` is `[San]` only (line 127,
  attributed to design-02's own **D-13** — a two-digit, design-02-series ID,
  not this ledger's three-digit D-013, per the ledger header's own warning
  about the two series colliding); and `unicode_paths` is set to `false`
  (line 147).

**Correction to how the static map's `unicode_paths: false` must be read.**
It is not a placeholder awaiting a probe that has never been implemented.
The static map's own comment is explicit that `false` is a *conservative
default for this one field specifically because a static, build-time map
cannot itself observe a runtime fact*: "Conservatively `false`. Design-02
Decision D-3 makes this ONE field genuinely runtime-derived, not static ...
Phase 1a implements no process spawning at all, so no such probe has been
run **by this code**" (`src-tauri/src/engine/capability.rs:128-134`,
quotation, emphasis original). The probe itself exists and runs on every
launch: `probe_unicode_paths`/`probe_unicode_paths_in`
(`src-tauri/src/engine/sidecar.rs:264-293`) spawn the real,
checksum-verified sidecar against a freshly generated Bengali-named
directory and file and report the capability `true` only if the sidecar
exits 0 and produces non-empty output. `startup_check` wires this into
every launch — `capabilities.unicode_paths = probe_unicode_paths(&engine).await;`
(`sidecar.rs:312`) — and a dedicated test requires the probe to pass against
the real sidecar (`sidecar.rs:418-428`). So the accurate framing is: the
static map conservatively *records* `false` because it has no way to know
better; the per-launch runtime probe *overrides* that with the real,
empirically observed result before the capability ever reaches a consumer.
See D-009 below, which this same probe is also cited under.

Every `false`/gated value is a **hard, structural gate**, not a soft
default: "`false` on any field is a hard capability gate: the compiler
returns `CompileError::UnsupportedOption` rather than dropping,
downgrading, or approximating the corresponding request"
(src-tauri/src/domain/capability.rs:34-37). `command_compiler.rs`'s own
module doc states the same discipline from the consuming side: "Every rule
enforced here is cited against `DECISIONS-LEDGER.md` D-007 (empirically
verified against the real engine binary) or design-02's source-cited flag
table (§1.3) — this module intentionally does not 'improve on' or
second-guess those findings" (src-tauri/src/engine/command_compiler.rs:7-11).

### Findings under this decision: V-1 through V-5

**V-1 — `-d`/`-D` are mutually exclusive.** Passing both flags is an
immediate exit-code-1 failure in either order, never a silent "last one
wins" (docs/engine-capabilities.md:168-170). The two single-flag variants
(`-d` alone vs. `-D` alone) are **asserted in the domain doc comment** to
produce byte-identical main-output content — the only difference being
whether an audit file exists (src-tauri/src/domain/operations.rs:59-64).
The cited test (src-tauri/tests/duplicate_integration.rs:256-296) verifies
matching *retention* — equal game counts (2) and the same named game
present/absent — not a literal byte-for-byte comparison of the two output
files; that stronger byte-identity claim is the doc comment's own assertion,
not something this particular test independently re-proves.

**V-2 — duplicate retention semantics, and the "metrics trap."** `-d` alone
diverts every *later*-encountered copy of a duplicate group to an audit
file, keeping only the first-encountered copy in the main output — it never
merely "additionally writes" an audit file next to an unfiltered main
output (src-tauri/src/domain/operations.rs:59-64). Reproduced end to end: a
3-game fixture (two duplicate copies + one unique game) yields 2 games in
the main output and 1 in the audit file
(src-tauri/tests/duplicate_integration.rs:158-231). The "metrics trap": the
engine's own final-summary line ("N games matched out of M") counts
diverted duplicates as matched, so a job's `input_games` metric (derived
from that summary line) legitimately disagrees with `output_games` (from
actually counting the published file) — asserted directly:
`input_games == Some(3)`, `output_games == Some(2)` for the same run
(src-tauri/tests/duplicate_integration.rs:233-246).

**V-3 — `--maxmoves` must be emitted before `--minmoves`.** Per
docs/engine-capabilities.md:182-190, the engine "stores move bounds
ply-encoded but compares them against the raw incoming move count during
validation," which silently drops the upper bound when the "obvious"
min-then-max order is used and `max < 2*min - 1` — a 30-move game can pass
a filter that looks like a 10–15 move bound, with no error printed and exit
0. PGN Studio's compiler always emits `--maxmoves` first
(src-tauri/src/domain/filters.rs:167-171), with a regression test that
deliberately picks bounds (30/40, or 10/15 in the end-to-end variant)
*inside* the empirically verified trigger zone
(src-tauri/src/engine/golden_tests.rs:326-378) rather than an arbitrary
min/max pair that would give false confidence. Reproduced directly against
the real engine, bypassing the compiler, at
src-tauri/tests/phase5_filters_integration.rs:915-978, and pinned as a
fixture (`move-bounds.pgn`: three games of exactly 3/15/30 moves) at
fixtures/README.md:86.

**V-4 — the ECO flag must use the attached form `-e<path>`, never the
separated `-e <path>` two-token form.** `docs/engine-capabilities.md:172-173`
states the separated form's *most common* failure mode is not silent at
all: it "usually fails loudly (`Unable to open the ECO file eco.pgn.`,
empty output, exit 1)." `fixtures/golden/regex/README.md:82-84` documents
the rarer, more dangerous manifestation: the separated form "fails
*silently* with exit 0 and zero games extracted for that specific flag." A
still more dangerous silent-wrong-classification variant is also
documented — if a file literally named `eco.pgn` is independently reachable
via the engine's own fallback search (`$ECO_FILE` or CWD), "a
separated-form invocation could succeed while silently classifying against
the *wrong* ECO data" (docs/engine-capabilities.md:171-181, quoted). Which
of these three outcomes (loud exit 1, silent exit 0, or silent
misclassification) occurs is environment-dependent, per the same passage.
PGN Studio always uses the attached form for every value-taking flag and
strips `ECO_FILE` from the engine's environment (same section;
src-tauri/tests/eco_supplement_integration.rs:134-135; regression guard at
src-tauri/tests/phase4_integration.rs:693-733).
*Reconstruction note on exact original wording:* `phase4_integration.rs`'s
own header comment says its **fresh, independent** Phase-4 reproduction of
this hazard found a *different* manifestation (exit 1, empty output, no
ambient `eco.pgn`) than what it calls "the ledger's own historical repro
('EXIT CODE 0')" (src-tauri/tests/phase4_integration.rs:44-59) — this is the
closest thing to a direct quotation of D-007 V-4's original text found
anywhere in the tree, and it is only preserved second-hand, inside another
file's paraphrase. That same comment states the reproduction "differs in
its precise exit code" but that "the ledger's core conclusion – the
attached form is mandatory – is independently reconfirmed, not disputed."
Separately, this ledger's own "cited but not recorded" table describes V-4
only generically, as a "Regex/criteria-file verification finding" — looser
than, though not contradicted by, the specific ECO-flag hazard every other
citation site describes.

**V-5 — no separate broken-games output file is possible in one pass.**
There is exactly one broken-games-related flag, `--keepbroken`, with
exactly two states: without it, a structurally broken game (e.g. missing
its result marker) is silently dropped from the output entirely; with it,
that game lands in the *same main* output alongside good games — never a
third, separate file (docs/engine-capabilities.md:71-86 table;
src-tauri/src/domain/operations.rs:103-111; src-tauri/src/domain/capability.rs:63-68;
src-tauri/src/engine/capability.rs:102-109; end-to-end tests at
src-tauri/tests/phase4_integration.rs:1130-1213). This is also why the
broken-games *count* cannot be derived from the engine's own accounting: a
verified case had a 3-game file, with the last game missing its result
marker, in which the engine reported "2 games matched out of 2" even though
a third game existed and was silently dropped
(docs/engine-capabilities.md:88-99, quoted). PGN Studio's `broken_games`
metric is therefore unconditionally `None` — never computed, estimated, or
defaulted to zero (docs/engine-capabilities.md:101-104;
src-tauri/tests/phase4_integration.rs:1163-1166). This constraint is carried
through to the UI/product layer by name: ModeAndValidationSection.tsx:8-14
and the app's built-in presets (src/state/presets.ts:11-17, enforced by
src/state/presets.test.ts:19-25) both cite "D-007 V-5" to justify why the
broken-games UI offers only Discard / Keep-in-Main-Output, never a
"separate file" option.

**V-6 is deliberately not included above.** A companion finding, "PowerShell
mangles attached-form flags built by string interpolation," is reconstructed
separately below. No citation ties it to D-007 specifically — both of its
citation sites (`scripts/lib/engine-common.ps1:55`: "decisions ledger V-6";
`fixtures/golden/regex/README.md:77`: "decisions ledger finding V-6") name
only "the decisions ledger," never a `D-###` parent, unlike V-1 through V-5
which are all explicitly tagged "D-007 V-#" somewhere in the tree. Folding
V-6 under D-007 here would be exactly the invented-connection error this
ledger's header rule forbids.

### Consequences

- `EngineCapabilities`/`pinned_v26_06()` gate every corresponding UI option
  as a hard yes/no: "if a capability is `false`, the corresponding UI option
  is disabled with an explanation rather than silently dropped or
  approximated" (docs/engine-capabilities.md:37-39).
- The `BrokenOutput` domain enum has no "separate file" variant at all —
  the impossible option is made structurally unrepresentable rather than
  accepted and silently downgraded (src-tauri/src/domain/operations.rs:106-111).
- `docs/engine-capabilities.md` is treated as the source of truth over
  earlier design documents wherever they disagree: "Where this document and
  the architecture design docs disagree, this document — grounded in the
  running binary — is the one to trust" (docs/engine-capabilities.md:13-14).
- The same citations propagate from the Rust domain layer through the
  generated IPC types (src/ipc/generated-types.ts:59-65, 173-179, 237-243,
  587-593) to React UI copy (ModeAndValidationSection.tsx) and to the
  built-in preset library and its tests, so "D-007 V-5" in particular
  constrains four independent layers of the codebase, not just the engine
  boundary.
- Purpose-built fixtures exist specifically to keep each hazard
  reproducible: `move-bounds.pgn` for V-3 (fixtures/README.md:86),
  `duplicates/order-a.pgn`+`order-b.pgn` for V-1/V-2
  (src-tauri/tests/duplicate_integration.rs:161-163), and
  `malformed/illegal-move.pgn` for V-5
  (src-tauri/tests/phase4_integration.rs:1135-1138).

### What these citations do *not* establish

Two other "verified surprises" documented in the same section of
`docs/engine-capabilities.md` — that relational/equality operators (`=`,
`<`, `<=`, `>`, `>=`) silently match zero games on non-numeric tags, and
that `Date` bounds need a full date rather than a bare year — sit under
that document's blanket "D-007/D-013" citation (docs/engine-capabilities.md:10)
but are **not** individually tagged with a D-007 V-# anywhere else in the
tree. Where the actual enforcing code cites its source, it attributes the
non-numeric-operator rule to **D-010** specifically
(src-tauri/src/engine/criteria.rs:206-211: "empirically verified against the
real engine binary; DECISIONS-LEDGER.md D-010 recorded the same fact for
ECO specifically ... but it is a general property of every non-numeric
tag") and the `Date`-bounds rule to **design-02**, not this ledger
(src-tauri/src/engine/criteria.rs:114-122). This entry does not fold those
two findings into V-1–V-5, since no citation ties them to a D-007
verification-finding number.

### Gaps

- The original ledger's exact prose for D-007 and for each V-# under it is
  lost. Everything above is a paraphrase of what citing files assert
  D-007/V-1..V-5 say, not a transcription, except the one near-verbatim
  fragment `phase4_integration.rs:44-59` attributes to the ledger's
  "historical repro" of V-4 ("EXIT CODE 0") — and even that is itself a
  paraphrase inside another file's comment, not a direct quotation of the
  ledger file.
- No date of the original decision, no author/reviewer, and no record of
  alternatives considered are recoverable from any citation.
- This ledger's own "cited but not recorded" table labelled V-4 only
  generically as a "Regex/criteria-file verification finding," looser than
  — though not contradictory to — the specific ECO-attached-flag hazard
  every other citation site consistently describes. Whether that table's
  author simply categorized V-4 by which file it happens to be cited from,
  or whether V-4 covers additional regex/criteria-file findings no other
  file happens to cite, cannot be resolved from what survives.
- design-02's own D-1, D-3, D-6, and D-13 IDs are cited alongside D-007 in
  several files (e.g. src-tauri/src/domain/operations.rs:56-57,104;
  src-tauri/src/domain/capability.rs:65,70) as apparently-related but
  textually separate findings in design-02's own numbering series. Per this
  ledger's header warning about the two D- series colliding, this entry
  does not attempt to reconstruct or characterize design-02's content —
  only notes where a citation sits next to a design-02 ID so the boundary
  between the two series stays visible.
- Which decision, if any, commissioned V-6 is not established here — see
  the V-6 entry below for what its own citations do and do not support.

---

## D-008 — `eco.pgn`/`COPYING` must be true-upstream LF bytes; the `core.autocrlf` checksum-drift hazard

**Status:** inferred active — no citation states a status line for D-008.
The rule is still enforced today by `.gitattributes`' `-text` exceptions for
the three affected files and by `scripts/build-pgn-extract.ps1` writing
checksums from `upstream.lock`'s pre-recorded values rather than re-hashing
the working tree.

**Reconstruction note (read before citing).** This entry is thinner than
D-006 and D-014 above. Exactly **one** file in the tree cites the ID
`D-008` — `engine-src/upstream.lock:21`, plus its unlabelled twin note at
`:26` — and neither one *quotes* original ledger prose; the citation is a
pointer ("see decisions ledger D-008"), not a transcription. So unlike
D-006, nothing below is marked "quotation" — it is all paraphrase, built
by tracing the hazard the citation names into the un-tagged corroborating
context that independently describes the same mechanism:
`engine-src/README.md`, `.gitattributes`, `src-tauri/resources/pgn-extract/SOURCE.json`,
and `scripts/build-pgn-extract.ps1`. No date, author, or rationale beyond
the hazard itself and its fix is established by any citation found.

### Decision

Paraphrased from `engine-src/upstream.lock:20-21,25-26`: the per-file
checksums (and sizes) recorded for the two upstream files bundled
verbatim from pgn-extract — `eco.pgn` and `COPYING` — must be **true
upstream LF bytes**, and must never be recomputed from a copy that passed
through an ordinary `git clone`/checkout working tree, because the local
machine's `core.autocrlf` setting can silently rewrite LF to CRLF on
checkout, changing the file's hash and size without changing its content.
`upstream.lock:21` gives the recipe recorded for `eco.pgn` — reading the
git blob directly via `git cat-file -p <rev>:eco.pgn`, which bypasses a
working-tree checkout entirely — and states the warning as an imperative:
"Do NOT recompute from a git working tree." `upstream.lock:26` records
that `COPYING` carries "the same CRLF caveat."

`engine-src/README.md:117-127` documents what reads as the same rule,
independently, as part of "How each field was produced" for this pin, and
is the only place in the tree that narrates *why* the rule exists rather
than just stating it: the per-file checksums for `COPYING`/`eco.pgn` in
`SOURCE.json` "must be computed from files extracted directly from this
verified tarball (`tar xzf ...`), **not** from a local `git clone`"
(`README.md:118-120`), because, in the words of the "pitfall encountered
while producing this pin, worth recording" — "this happened during Phase
0 development: a Windows checkout with `autocrlf=true` converted
upstream's native LF line endings to CRLF, which changed the files'
bytes/hashes without changing their meaning" (`README.md:122-125`).
`src-tauri/resources/pgn-extract/SOURCE.json:23` restates the same rule,
in substance, as the machine-readable `bundledFilesChecksumMethod` field:
hashes were taken by extracting the verified source tarball, "not copied
from a local `git clone` working directory," for the identical reason —
"a local checkout's line endings can be silently rewritten by the cloning
machine's git `core.autocrlf` setting, which would change the file's
bytes/hash without changing its meaning."

One nuance the citations leave open rather than resolve: `upstream.lock`
describes the safe method as `git cat-file -p` against the pinned commit,
while `README.md`/`SOURCE.json` describe it as extraction from the
checksum-verified source tarball. Both avoid a working-tree checkout —
the one place `core.autocrlf` acts — but no citation states outright
that the two are meant as the same procedure, alternatives, or reflect
two different points in the pin's history. This reconstruction reports
both as documented rather than collapsing them into one claim.

### Evidence of the underlying incident

- `engine-src/README.md:122-124` is the only concrete account in the tree
  of the hazard actually firing: a Windows checkout with
  `autocrlf=true`, during what the same file calls "Phase 0 development,"
  silently converted `eco.pgn`/`COPYING`'s upstream LF line endings to
  CRLF, changing their bytes and hashes without changing their meaning.
- `.gitattributes:1-6` points at this same incident rather than repeating
  it — "see the exceptions below and `engine-src/README.md` point 4 for a
  real instance of this going wrong during Phase 0 development" —
  confirming that `.gitattributes`' own `-text` exceptions (see
  Consequences) are the tree's structural response to it, not an
  unrelated hardening measure.

### Consequences

- `.gitattributes:13-15` disables Git's line-ending normalization
  entirely (`-text`) for `LICENSE`, `src-tauri/resources/pgn-extract/COPYING`,
  and `src-tauri/resources/pgn-extract/eco.pgn` — layered on top of the
  repo-wide `* text=auto eol=lf` at line 7 — specifically, per the comment
  at `.gitattributes:9-12`, so "a checkout can never drift from the
  recorded checksum."
- `scripts/build-pgn-extract.ps1:467-470` writes the `eco.pgn` entry of
  the shipped `checksums.json` from
  `$lock.engine.resources.'eco.pgn'.sha256`/`sizeBytes` — i.e. from
  `upstream.lock`'s pre-recorded values — rather than by re-hashing
  whatever copy of `eco.pgn` happens to sit in the working tree at build
  time.
- `src-tauri/resources/pgn-extract/SOURCE.json:28,34` records the same
  two sha256 values (`8ceb4b9e...` for `COPYING`, `058ad9ff...` for
  `eco.pgn`) as `upstream.lock:20,25`, so the two records agree with each
  other rather than being independently derived and potentially drifting.

### Gaps

- No date, author, or explicit alternatives-considered for the decision
  itself. `README.md` dates the *pin-production* narrative it appears in
  to 2026-08-07 and places the *triggering incident* in "Phase 0," but no
  citation dates D-008 as a decision, and no citation quotes original
  ledger prose the way `platform/mod.rs:16` does for D-006.
- Whether anything re-verifies, on an ongoing basis, that the committed
  `src-tauri/resources/pgn-extract/{COPYING,eco.pgn}` bytes still match
  the sha256/sizeBytes recorded in `upstream.lock`.
  `scripts/verify-engine.ps1`'s Layer 0 (`verify-engine.ps1:128-148`)
  only re-downloads and re-checks `sourceArchiveSha256` — the whole
  pgn-extract source tarball — against a fresh copy; no citation found
  ties Layer 0, or any other verification layer, to re-hashing these two
  specific bundled resource files against `upstream.lock`'s `resources`
  entries. (Independently re-checked here: `eco.pgn` appears elsewhere in
  the verify script only as Layer 2's `ECO_FILE` make variable, not as a
  hash target.)

---

## D-009 — Windows UTF-8 process manifest for non-ASCII paths; `--help` excluded from engine-identity checks

**Status:** presumed active — the manifest is still embedded by the current
build script and `engine::sidecar` still implements the probe and the
`--version`-only identity rule the citations describe. No citation marks
D-009 amended or superseded (contrast D-006), and no citation states an
original decision date or author (contrast D-006/D-014's explicit
provenance notes).

> **Citation drift, noted honestly.** This ledger's own "Cited but not
> recorded" table cites `src-tauri/src/engine/sidecar.rs:21,265,398`. A
> repo-wide, case-insensitive grep for D-009 today finds it at lines **21,
> 275, and 425** instead — the file has evidently moved since that table
> row was written. All line numbers below are current, re-verified
> positions, not the stale ones.

### Decision

`sidecar.rs` is the only file in the tree that cites D-009 by ID (three sites). It attributes two distinct rules to D-009:

1. **Non-ASCII (e.g. Bengali) filesystem paths must work on Windows, achieved via a PGN-Studio-authored Win32 application manifest, not an engine source change.** `sidecar.rs:425` ties this to D-009 directly — a test asserts *"the real sidecar embeds a UTF-8 activeCodePage manifest (D-009) and must pass"* the Bengali round trip. The manifest (`engine-src/manifest/pgn-extract.manifest:5`) states it is *"PGN Studio-authored Win32 application manifest for the pgn-extract"* engine and (`:8`) is *"embedded into the built binary by scripts/build-pgn-extract.ps1 via"* `link.exe /MANIFEST:EMBED /MANIFESTINPUT:<file>`. Mechanism, paraphrased from the manifest's own comment (`engine-src/manifest/pgn-extract.manifest:11-19`): pgn-extract opens files with ANSI CRT calls (`fopen()`/`access()`), which on Windows use the process's active code page (CP_ACP); a legacy CP_ACP cannot represent non-Latin scripts, so such paths are mangled before `CreateFileW` and the open fails. Windows 10 1903+ lets a process opt in to UTF-8 as CP_ACP via the manifest setting the file actually contains, `<ws2:activeCodePage>UTF-8</ws2:activeCodePage>` (`engine-src/manifest/pgn-extract.manifest:41`). The build script guards this as a hard requirement — it throws *"UTF-8 manifest not found at $ManifestFile"* if the file is missing (`scripts/build-pgn-extract.ps1:297`) — and passes it to the linker via `"/MANIFEST:EMBED"` / `"/MANIFESTINPUT:$ManifestFile"` (`scripts/build-pgn-extract.ps1:317-318`).

2. **The capability is verified by an actual runtime probe every launch, not hardcoded from the build-time check.** `probe_unicode_paths`/`probe_unicode_paths_in` (`sidecar.rs:264-293`) generate a fresh Bengali-named temp directory and a Bengali-named file inside it, write embedded fixture content into it, run the real sidecar, and report the capability true only if the sidecar exits 0 and the output is non-empty (`sidecar.rs:292`: `let produced_nonempty_output = output_path.metadata().map(|m| m.len() > 0).unwrap_or(false);`). The comment where the probe path is built **quotes D-009 directly**: *"Bengali directory name AND file name (D-009's verification note:"* / *"Bengali filenames AND Bengali directory names work")."* (`sidecar.rs:275-276`) — this is a quotation of the original ledger's verification note, not a paraphrase. The fixture content is `fixtures/unicode-paths/দাবা-খেলা.pgn`, pulled in at compile time via `include_str!` (`sidecar.rs:206`). `startup_check` wires the real probe result into the capabilities struct every launch (`sidecar.rs:312`: `capabilities.unicode_paths = probe_unicode_paths(&engine).await;`), and a dedicated test requires it to actually pass against the real sidecar (`sidecar.rs:419-425`, `probe_unicode_paths_is_true_for_the_real_utf8_manifest_sidecar`).

3. **`--help` is never parsed to establish engine identity, because its banner carries a build-date placeholder rather than a stable value; identity comes from `--version` only.** `sidecar.rs:20-21` states this and cites D-009 for it: *"parsed for identity (its banner contains a build-date placeholder,"* / *"DECISIONS-LEDGER.md D-009)."* The two-gate identity check instead spawns `--version` as an argument array and requires exit 0 with stderr trimmed to exactly `pgn-extract v26-06` (`sidecar.rs:9,11`: *"version. Gate 2 spawns `--version` as an argument array (never a shell)"* … *"exactly `pgn-extract v26-06`."*). The placeholder mechanism itself is corroborated — though not cited to D-009 by ID — in `scripts/build-pgn-extract.ps1:275,277`: the `/Brepro` compile flag *"neutralizes"* … *"argsfile.c's `__DATE__` use in the --help banner to a fixed"* placeholder, *"confirmed by"* … *"`pgn-extract --help` printing "(1)" where the date would"* otherwise appear, and confirmed **not** to affect `--version` (a separate, date-free `CURRENT_VERSION` site in the same source file, `:279`).

### Evidence

- `src-tauri/src/engine/sidecar.rs:9-11, 19-21` — module doc: two-gate identity check is `--version`-stderr-only; `--help` excluded, citing D-009.
- `src-tauri/src/engine/sidecar.rs:264-293` — `probe_unicode_paths`/`probe_unicode_paths_in`, the runtime probe.
- `src-tauri/src/engine/sidecar.rs:275-276` — direct quotation of D-009's verification note.
- `src-tauri/src/engine/sidecar.rs:206` — probe fixture `include_str!`'d from `fixtures/unicode-paths/দাবা-খেলা.pgn`.
- `src-tauri/src/engine/sidecar.rs:308-317` — `startup_check` sets `EngineCapabilities.unicode_paths` from the real probe every launch.
- `src-tauri/src/engine/sidecar.rs:418-428` — test naming D-009, requiring the probe to pass against the real sidecar.
- `engine-src/manifest/pgn-extract.manifest` (whole file, esp. lines 5-9, 11-25, 27-31, 38-44) — authorship, CP_ACP mechanism, an informal "Phase 0b" verification claim, and the literal `activeCodePage` XML setting. This file itself never cites "D-009"; the link to D-009 is established only via `sidecar.rs:425`.
- `scripts/build-pgn-extract.ps1:17, 244-246, 296-298, 316-319` — manifest is a mandatory, `throw`-guarded build input, embedded via `link.exe /MANIFEST:EMBED`.
- `scripts/build-pgn-extract.ps1:272-279` — corroborates the `--help` build-date-placeholder mechanism via `/Brepro`, without itself citing D-009.
- `src-tauri/src/engine/capability.rs:128-147` — Phase 1a's static capability map deliberately defaults `unicode_paths: false` because no probe had been wired up yet by that code specifically; explicitly calls the manifest's own Phase 0b comment "encouraging but ... a development-time, human-run check on one build, not the per-launch automated probe" (`:140,144`) — so it does not license hardcoding `true`. Attributes the runtime-probe *architecture* to **design-02 Decision D-3** (`:128`), not to this ledger's D-009 — see Gaps.
- `src-tauri/src/domain/capability.rs:73-79` — the `unicode_paths` field doc comment, likewise attributed to design-02 Decision D-3, not D-009.
- `fixtures/README.md:61-70` — documents the `unicode-paths/` fixture directory's two files ("verified byte-correct on disk via `fs.readdirSync`"); does not cite D-009 by ID.
- `src-tauri/tests/job_orchestration_integration.rs:407-420` (`unicode_bengali_input_and_destination_round_trip`) — a second, independent exercise of the same fixture through a full job, copying `unicode-paths/দাবা-খেলা.pgn` from disk; does not cite D-009 by ID.
- `docs/acceptance-criteria.md:165-176` — records this capability as "[~] Partially verified: Windows verified, macOS not achievable here," and explicitly declines to call macOS verified merely because it is "architecturally lower-risk"; does not cite D-009 by ID.
- This ledger's own prior "Cited but not recorded" row for D-009 was itself already a citation-based summary rather than a transcription, and the source of the stale line numbers noted above.

### Consequences

- `EngineCapabilities.unicode_paths` is a runtime-derived field, never a static one: `engine::capability::pinned_v26_06()` sets it to a conservative `false` (`capability.rs:147`), and only `startup_check` (`sidecar.rs:308-317`) can set it `true`, and only after the real probe succeeds against the real, checksum-verified sidecar.
- Engine identity verification (`resolve_and_verify`'s gate 2) is contractually `--version`-only; reading identity from `--help` output would reintroduce the exact hazard (a build-date placeholder, not a stable identity string) D-009 is cited to rule out.
- The `unicode-paths/` fixture directory and its two Bengali-named files/folder exist to back both the startup probe (via `include_str!`) and a separate full-job integration test (via a disk copy).

### Note on reconstruction

Only `sidecar.rs:275-276` (`"Bengali filenames AND Bengali directory names work"`) and `sidecar.rs:21` (`"DECISIONS-LEDGER.md D-009"`) are direct quotations of the original ledger, both marked as such above. Everything else in the Decision section is paraphrase of what the citing code (`sidecar.rs`, the manifest file, the build script) asserts D-009 requires or established — not a transcription of D-009's original text, which is not recoverable from these citations.

### Gaps

The citations establish, with reasonable confidence, only three things: (1) a Windows UTF-8 process manifest exists specifically so pgn-extract can open non-Latin (demonstrated: Bengali) paths, and it is verified by a real per-launch probe rather than hardcoded; (2) that verification note is directly quoted once, verbatim, as "Bengali filenames AND Bengali directory names work" (sidecar.rs:276); and (3) `--help` is deliberately excluded from engine-identity parsing because its banner carries a build-date placeholder, with `--version` used instead. Beyond that:

- No citation gives D-009's original decision date, author, or "Status" (active/amended/superseded). The "presumed active" status above is inferred only from current code still implementing the rule, not from any citation stating status.
- No citation records what alternatives were considered and rejected before choosing an embedded Win32 manifest (e.g. wide-character APIs, a wrapper process, SetConsoleOutputCP) — nothing should be invented here.
- The exact original wording of D-009 beyond the two quoted fragments is not recoverable. The rest of the Decision section is paraphrase of citing code, clearly marked as such.
- `src-tauri/src/domain/capability.rs:73-79` and `src-tauri/src/engine/capability.rs:128-147` describe the same runtime-probe architecture (unicode_paths set from a live probe, not hardcoded) but attribute it explicitly to **design-02 Decision D-3**, a different, numerically-unrelated ID series per this ledger's own header warning. This entry does not merge D-3 into D-009 even though both concern the identical probe in the identical code — that would be exactly the citation-drift the ledger's header warns against.
- Nothing in the D-009 citation trail makes any claim about macOS Unicode-path handling. `docs/acceptance-criteria.md:165-176` discusses it but never cites D-009, and explicitly refuses to call it verified.
- The line numbers this reconstruction started from (sidecar.rs:21, ~265, ~398 — per this ledger's own existing "Cited but not recorded" table) do not match a fresh grep today (21, 275, 425). The file has evidently changed since that table row was written; this entry cites the current, re-verified positions and flags the drift rather than silently using stale ones.

---

## D-010 — Tag-filter relational/equality operators silently fail against ECO; only prefix, `<>`, and `=~` work

**Status:** cannot be determined from citations (no file states whether
D-010 is active, amended, or superseded as a ledger entry in its own
right — see Gaps).

### Decision (reconstructed — paraphrase unless marked as a quotation)

D-010 empirically established, by running the real pinned `pgn-extract` engine directly ("the raw engine", as distinguished by `phase5_filters_integration.rs:709` from a later end-to-end job-pipeline reconfirmation), that against the `ECO` tag specifically, five of the engine's six relational/equality criteria-file operators silently match **zero games** — not an error, just a criteria file that compiles and runs but matches nothing:

- `=`, `<`, `<=`, `>`, `>=` — silently match nothing against `ECO`
  (`src-tauri/src/engine/criteria.rs:157-159,539-540`; `src/types/workflow.ts:74-76`;
  `src/features/filters/EloAndEcoFilters.tsx:5-7`).
- `<>` (not-equal) — empirically verified to work correctly against `ECO`
  (e.g. `ECO <> "B10"` matched the B90/A00 games in D-010's own fixture)
  (`src-tauri/src/engine/criteria.rs:576-578`).
- Prefix (no operator) and `=~` (regex) are not called out by these citations as
  part of D-010's own tested set, but this ledger's own "Cited but not recorded"
  summary line named D-010's subject as "Tag-filter `<>` (not-equal) semantics
  and the ECO methodology used to establish them", and `fixtures/README.md:85`
  groups prefix/`<>`/`=~` together as the three operators the (later, Phase 5)
  `eco-codes.pgn` fixture exercises "matching the same methodology D-010
  itself used."

D-010 recorded this finding as **ECO-specific** — `criteria.rs:159` says D-010
"recorded this as an ECO-specific fact", and `criteria.rs:701` cites "D-010's
own framing" for the earlier, narrower, ECO-only scope, in contrast to the
later Phase 5 generalization (see Consequences). The phrase `"ECO operator
support"` appears in quotation marks at `criteria.rs:159` but no citation
explicitly frames it as a verbatim quotation from the lost ledger text, so it
is reported here only as wording that appears associated with D-010, not
asserted as an exact quote.

**Methodology.** D-010's finding was established empirically against the real
engine binary, using a fixture with `ECO` tags **set directly** in the PGN
data rather than produced by the engine's own `-e` classification flag
(`fixtures/README.md:85`: "ECO tags are set directly in the fixture rather
than produced by `-e` classification, matching the same methodology D-010
itself used") — isolating the question "does this filter operator work" from
the separate question "does the engine classify openings correctly."

**Correction to an earlier phase.** `criteria.rs:579` states D-010's finding
that `<>` works "is a correction to Phase 1a, which rejected all six
operators" (i.e. an earlier project phase had been more conservative than the
engine's actual behavior required, disallowing `<>` on `ECO` along with the
other five).

### Consequences (as implemented / cited)

- `src-tauri/src/engine/criteria.rs`'s `ensure_relational_op_safe_for_text_tag`
  rejects `=`, `<`, `<=`, `>`, `>=` for `ECO` (and, per a later Phase 5
  generalization described below, for every non-numeric tag) at compile time,
  with an explanatory `CompileError`, and allows only Prefix, `Ne` (`<>`), and
  Regex (`criteria.rs:198-217`, tests at `criteria.rs:538-587`).
- The Filters screen never exposes an ECO operator picker at all — only a
  free-text value and an "Exclude" checkbox (which compiles to `<>`); the five
  broken operators are never offered as UI options
  (`src/features/filters/FiltersScreen.test.tsx:109`,
  `src/features/filters/EloAndEcoFilters.tsx:5-11`,
  `src/types/workflow.ts:74-82`).
- `src/state/filterMapping.test.ts:112-120` pins that excluded ECO entries
  compile to `op: "ne"` and never to `"eq"`/`"gt"`/`"ge"`/`"lt"`/`"le"`.

### Later generalization beyond D-010 (Phase 5) — not part of D-010 itself, but built directly on it

A later task ("this phase" / Phase 5, per `phase5_filters_integration.rs:28-41`
and `criteria.rs:156-184`) re-tested D-010's finding against other non-numeric
tags (`White`, `Site`, `Result`) and found the "gate" is **not ECO-specific**
but a general property of every non-numeric tag on this engine. This
generalization is explicitly framed in the citing code as building on D-010,
not as amending it — no citation states D-010's own entry was edited, dated,
or given an amendment/status marker for this. Concretely: `src/state/filterMapping.ts`
was compiling every Result-filter checkbox (White wins / Black wins / Draw /
Other / Decisive-only) to `Result = "<value>"`, which — hitting the same gate
— silently matched zero games on every job that used a Result filter; this
is described as "a live, shipped, ship-blocking bug, not a hypothetical one"
(`phase5_filters_integration.rs:33-41`) and was fixed alongside a generalized
compiler-level guard (`criteria.rs`'s `tag_is_numeric`/
`ensure_relational_op_safe_for_text_tag`) and a matching frontend fix
(`src/state/filterMapping.ts:56-66`, `filterMapping.test.ts:46-62,161-169`).

### Gaps

- **Status/date/author are not established.** No citation gives D-010 a
  status line (active/superseded/amended), a creation or decision date, or an
  author — unlike D-006 and D-014, no file quotes or paraphrases such
  metadata for D-010.
- **The lost ledger's exact wording is not recoverable.** Only fragments
  survive via paraphrase in citing code/docs; no citation frames any of its
  text as a direct quotation of the original entry (contrast D-006, where
  `platform/mod.rs:16` explicitly quotes the ledger). The phrase "ECO
  operator support" at `criteria.rs:159` may or may not be a verbatim
  fragment — this cannot be confirmed from the citation alone.
- **The underlying engine mechanism ("why" `<>` works but the other five
  don't) is not explained by any citation.** All citations describe the
  observed behavior (verified counts against a fixture), not a `pgn-extract`
  source-level cause. No file under `engine-src/` cites D-010 at all.
- **D-010's own original fixture/test is not identifiable in the current
  tree.** The `eco-codes.pgn` fixture and its counts documented in
  `fixtures/README.md` and exercised by `phase5_filters_integration.rs` are
  explicitly Phase 5 artifacts that reuse D-010's *methodology*, not
  necessarily D-010's original fixture or exact verified counts.
- **Which phase established D-010 itself is not stated.** Citations place it
  strictly after "Phase 1a" (which it corrects) and strictly before the later
  Phase 5 generalization, but no citation names the phase in which D-010 was
  itself produced.
- **Whether prefix and `=~` were part of D-010's own tested claim, or are
  only associated with it via this ledger's later "cited but not recorded"
  summary line and the Phase 5 fixture description, is not fully resolved** —
  the direct D-010 citations in `criteria.rs` and `phase5_filters_integration.rs`
  focus on the six relational/equality operators and `<>`; prefix/`=~` appear
  grouped with D-010 only in `fixtures/README.md:85` and this ledger's own
  prior summary row, which was itself a citation reference rather than the
  original entry.

---

## D-013 — cited only jointly with D-007 as authority for `docs/engine-capabilities.md`; independent content not recoverable

**Status:** cannot be determined from the tree. No file treats a
D-013-specific rule as binding, so whether it is active, superseded, or
purely historical is not established by any citation.

> **Reconstruction note.** This entry is assembled from a single citation
> site. It is deliberately thin because the evidence is thin — see "What
> cannot be recovered" below before relying on it for anything beyond the
> bare facts it states.

### What the citations establish

**D-013 is a real ID used by the original, lost ledger — not a numbering gap invented for this reconstruction.** This ledger's own D-014 entry states it directly: "Citations in the tree reach D-013, so the original ledger almost certainly also used D-011 and D-012" (D-014's "On this ID" note, above). That note exists precisely because D-013 is the highest three-digit ID actually cited anywhere in the repository, which is why D-014's author chose 014 rather than reusing 011.

**The one and only site in the tree that cites D-013 is `docs/engine-capabilities.md:10`, and it never appears alone — always bundled with D-007:**

> "Every claim below was independently verified by the coordinator running the real engine against purpose-built fixtures (see `DECISIONS-LEDGER.md` D-007/D-013 and `src-tauri/tests/phase4_integration.rs`, `phase5_filters_integration.rs`, `duplicate_integration.rs` for the tests that prove each one), and several corrected an earlier design document's assumptions." (`docs/engine-capabilities.md:8-13`, quoted)

Paraphrasing what this citation asserts D-007/D-013 jointly stand for: the entire body of capability and behavior claims that make up `docs/engine-capabilities.md` — the supported/unsupported flag table, the five "verified surprises," and the "other verified engine behavior" list — was established empirically, by running the actual pinned engine binary against fixtures built for that purpose, rather than by trusting the engine's `--help` text or an earlier design document. The same sentence states several of those verified claims corrected assumptions in "an earlier design document" (`docs/engine-capabilities.md:12`); elsewhere in the tree, corrections of this kind are consistently attributed to **design-02** (e.g. `fixtures/README.md:89`: "pins a correction to design-02 §1.5.1"; `src-tauri/tests/phase5_filters_integration.rs:49`: "a correction to the design document's own claim", following a passage naming design-02 at line 43). `docs/engine-capabilities.md:14` then states the resulting precedence rule: engine-capabilities.md, being grounded in the running binary, is the one to trust when it and the architecture design docs disagree.

**A separate, unrelated `D-13` (two-digit) exists in design-02's own numbering** and must not be conflated with this three-digit `D-013` — this ledger's header warns of exactly this collision generally, listing `D-13` among the design-02 IDs that collide in short form with this ledger's IDs. Several repo files cite design-02's `D-13` (e.g. `src-tauri/src/domain/operations.rs:83-85,128`, about the `--notags`/output-notation scope dropped from V1); none of those are evidence about this ledger's D-013.

### What cannot be recovered

Every other place in the tree that cites the same body of engine-verification evidence — and there are many — cites **D-007 alone**, never D-013: `src-tauri/src/engine/capability.rs:8` ("DECISIONS-LEDGER.md D-007 and design-02 §1.3's source-cited flag table"), `src-tauri/src/domain/operations.rs:56` ("D-007 V-1/V-2"), and likewise `fixtures/README.md:86`, `src-tauri/tests/phase5_filters_integration.rs:915`, `src-tauri/tests/phase4_integration.rs` (multiple sites), `src-tauri/tests/duplicate_integration.rs` (multiple sites), `src-tauri/tests/eco_supplement_integration.rs:134`, `src-tauri/src/engine/golden_tests.rs:348,376`, `src-tauri/src/engine/command_compiler.rs:8,232,688`, `src-tauri/src/domain/filters.rs:169`, `src-tauri/src/domain/capability.rs:65`, `src/state/presets.ts:14`, `src/features/operations/ModeAndValidationSection.tsx:11`, and `src/ipc/generated-types.ts:62,176,240,590`. None of these attributes any individual flag, capability boolean, or "verified surprise" specifically to D-013 as distinct from D-007.

There is therefore no basis in the tree for saying **what D-013 records that D-007 does not**. Plausible-sounding guesses — that D-013 was a companion decision about verification *methodology* (e.g. "trust the running binary over the design doc") while D-007 recorded the resulting *capability data*; that D-013 covered a narrower sub-scope of the same effort; or that it was simply a duplicate citation slot — are exactly that: guesses, not citations, and per this ledger's own never-invent rule they are not recorded here.

This ledger's own prior "Cited but not recorded" table row summarizing D-013 was itself part of an earlier reconstruction pass over this same file, not independent evidence — it restated only what `docs/engine-capabilities.md:10` already says.

No date, author, alternatives-considered, or explicit rationale for D-013 is established by any citation; none is guessed at here.

### Consequences

None can be stated from the evidence. Unlike D-007 — whose capability booleans and flag-ordering rules are directly enforced in code (`capability.rs`, `command_compiler.rs`) and covered by dedicated regression tests — no file in the tree implements, tests, or otherwise depends on a rule attributed to D-013 specifically. Whatever consequences the original D-013 decision had are lost along with its text.

### Gaps

The citations establish only three things about D-013: (1) it is a genuine ID from the original lost ledger, evidenced indirectly via D-014's remark that citations in the tree reach as high as D-013; (2) its sole direct citation is docs/engine-capabilities.md:10, where it is bundled with D-007 as joint authority for the entire engine-capabilities.md document, verified by running the real engine against purpose-built fixtures, with several claims correcting an earlier design document (elsewhere identified as design-02); (3) every other citation of this same evidentiary record in the repo names D-007 alone, never D-013. What is NOT established and is not guessed at here: what distinguishes D-013's own content or scope from D-007's; whether D-013 is still active or was superseded/folded into D-007; any date, author, or explicit rationale for D-013; and which specific claim(s) among engine-capabilities.md's many verified findings, if any, belong to D-013 rather than D-007. A separate two-digit `D-13` belonging to design-02's own numbering exists in the tree and is confirmed unrelated (per this ledger's own header warning about the two colliding D- series) but is worth flagging so a future reader does not accidentally merge the two.

---

## V-1/V-2 — Duplicate-detection verification findings under D-007: `-d`/`-D` mutual exclusivity, keep-first retention, and the "metrics trap"

**Status:** inferred active — no citation states a status for V-1/V-2; the
rule is still enforced today by `DuplicatePolicy`'s closed three-variant
enum and by `command_compiler.rs`'s structural exclusivity of `-d`/`-D`
(cited below).

> **On these IDs.** Per this ledger's header, `V-#` IDs are *verification
> findings* — "specific, empirically-established facts about the pinned
> engine's behaviour ... established by running it, not decided"
> (this file's header, which names D-007 V-1/V-2 as its own worked example) —
> namespaced under the decision that commissioned them and cited jointly as
> they are written in the tree: `src-tauri/src/domain/operations.rs:56`
> writes `"D-007 V-1/V-2"`. **D-007 itself (the parent capability-map
> decision) is reconstructed above.** This entry documents V-1 and V-2 as
> free-standing empirical findings about games "whose move sequence/hash
> repeats an earlier game in input order," not as an excerpt of D-007's own
> lost text.

### Finding

Both findings concern how the pinned `pgn-extract` build's `-d`/`-D` duplicate
flags behave, established by running the real engine through
`src-tauri/tests/duplicate_integration.rs`, which states its own method:
"real fixture PGNs, the real orchestrator, the real checksum-verified bundled
sidecar - no mocks" (`src-tauri/tests/duplicate_integration.rs:6-8`).

**V-1 — `-d` and `-D` are mutually exclusive, and produce byte-identical main
outputs.**

- The `DuplicatePolicy` doc comment, cited jointly to V-1/V-2, states: "`-d`
  and `-D` are mutually exclusive at the engine level (exit 1 if both are
  given) ... The two variants produce byte-identical main outputs; the only
  difference is whether an audit file exists at all."
  (`src-tauri/src/domain/operations.rs:59-64`; the identical sentence is
  mirrored in the generated TypeScript type at
  `src/ipc/generated-types.ts:179-184`). This byte-identity claim is the doc
  comment's own assertion; the cited tests verify matching retention
  (game counts and named-game presence/absence), not literal byte
  comparison of the two output files — see below.
- The integration test explicitly tagged to V-1 alone,
  `suppress_keep_first_matches_report_mode_main_output_with_no_audit_file`,
  is doc-commented "`-D` alone: same main-output retention as `-d`, but no
  audit file at all (DECISIONS-LEDGER.md D-007 V-1: `-d`/`-D` are mutually
  exclusive; the two variants produce byte-identical main outputs)."
  (`src-tauri/tests/duplicate_integration.rs:256-258`). Its body asserts the
  main output keeps exactly 2 games under `SuppressKeepFirst` (same count as
  the `ReportAndKeepFirst` test) and that no `.duplicates.pgn` file is ever
  produced (`src-tauri/tests/duplicate_integration.rs:287-296`).
- The `DuplicatePolicy` enum is modeled as a closed, three-variant enum
  (`None` / `ReportAndKeepFirst` / `SuppressKeepFirst`,
  `src-tauri/src/domain/operations.rs:67-76`), so there is no representable
  state that requests both flags at once — mutual exclusivity is enforced by
  the type, not by a runtime check.
- The command compiler's `match` over this enum marks the rule "hard": "O-4
  (hard: never both -d and -D; ValidateOnly forces DuplicatePolicy::None via
  validate_structural, so this arm naturally emits nothing under -r)"
  (`src-tauri/src/engine/command_compiler.rs:207-208`), emitting `-D` alone
  for `SuppressKeepFirst` (`src-tauri/src/engine/command_compiler.rs:212-214`)
  or `-d<path>` alone for `ReportAndKeepFirst`
  (`src-tauri/src/engine/command_compiler.rs:215-221`) — never both from one
  compiled command. This code does not itself carry a "V-1" tag; it is
  offered as the implementing mechanism for the doc-commented finding, not as
  an independent citation of it.

**V-2 — keep-first retention order, the exact verified 3-game shape, and the
"metrics trap."**

- The test tagged to V-2 alone,
  `report_and_keep_first_diverts_duplicates_and_retains_first_copy`, is
  doc-commented: "`-d` alone (DECISIONS-LEDGER.md D-007 V-2): main output
  holds only first copies, the audit file holds later copies. Reproduces the
  exact verified shape (3 games: two duplicates + one unique -> main gets 2,
  audit gets 1) using `order-a.pgn`/`order-b.pgn` ... plus a genuinely unique
  third game." (`src-tauri/tests/duplicate_integration.rs:158-163`). Its body
  proves the shape: the main output holds exactly 2 games ("2 duplicates + 1
  unique -> main output must hold exactly 2 games (first copy + unique)"),
  the audit file holds exactly 1
  (`src-tauri/tests/duplicate_integration.rs:203-212`), and the retained copy
  is the one from whichever fixture was listed first in input order
  (`src-tauri/tests/duplicate_integration.rs:214-231`).
- The same test's final assertions are explicitly labeled: "The 'metrics
  trap' (DECISIONS-LEDGER.md D-007 V-2): the engine's own final-summary line
  reports diverted duplicates as matched, so input_games (from that summary)
  and output_games (from actually counting the published main output) must
  legitimately differ here - this asserts the codebase never derives
  output_games from the summary line's matched count."
  (`src-tauri/tests/duplicate_integration.rs:233-238`), backed by
  `input_games: Some(3)`, `output_games: Some(2)`,
  `duplicate_games: Some(1)` (`src-tauri/tests/duplicate_integration.rs:239-241`).
- `docs/duplicate-semantics.md` restates the same finding in user-facing
  language, without citing "V-2" by number: "The engine's own summary line
  reports how many games it matched and processed, which includes every
  diverted duplicate ... PGN Studio never derives the `output_games` metric
  from that summary line for this reason ... `output_games` ... is computed
  by counting games in the actual published main-output file."
  (`docs/duplicate-semantics.md:87-92`).
- The matching implementation: `compute_final_metrics` derives `input_games`
  from the engine's own summary total
  (`src-tauri/src/jobs/run.rs:664`), but derives `output_games` by locating
  the published `UniqueGames` artifact and counting games in that file on
  disk, not from the summary's matched count
  (`src-tauri/src/jobs/run.rs:670-677`). This function carries no "V-2"
  citation at all — the link to V-2 is inferred from matching behavior
  described elsewhere, not from an in-file citation, and is flagged here as
  such.
- Retention-order causality (which duplicate copy survives is decided by
  input order, not content) is proven in both directions by
  `input_order_forward_keeps_alpha_and_diverts_bravo` and
  `input_order_reversed_keeps_bravo_and_diverts_alpha`
  (`src-tauri/tests/duplicate_integration.rs:304-341`,
  `:347-385`), using the `order-a.pgn`/`order-b.pgn` fixture pair described in
  `fixtures/README.md:47` as "designed to be fed as two separate *input
  files* with swappable priority, so a test can prove input order (not
  content) decides which copy survives." The design requirement this
  verifies is independently stated (without a V-# citation) in
  `docs/architecture.md:673-689` §10.7: "input order is a retention
  priority" and "Version 1 must not label this as 'Keep best copy.' It is
  'Keep first copy.'"

### Evidence

- `src-tauri/src/domain/operations.rs:55-76` — the `DuplicatePolicy` doc
  comment and enum definition, cited jointly "D-007 V-1/V-2".
- `src/ipc/generated-types.ts:174-198` — the generated TypeScript mirror of
  the same doc comment and enum.
- `src-tauri/src/engine/command_compiler.rs:207-221` — the compiler arm that
  makes the two flags structurally exclusive.
- `src-tauri/tests/duplicate_integration.rs:1-17,49-56,158-254,256-299,
  304-341,347-385` — the integration tests carrying the explicit V-1 and V-2
  tags, run against the real, checksum-verified sidecar
  (`src-tauri/tests/duplicate_integration.rs:49-56`), plus the retention-order
  tests they sit alongside.
- `docs/duplicate-semantics.md:1-9,35-56,58-68,85-97` — the user-facing
  document that restates keep-first retention, the two-mode table, and the
  metrics trap, stating up front that "every claim below was verified by
  running the real, pinned `pgn-extract` engine against purpose-built
  fixtures" (lines 5-6), citing `duplicate_integration.rs` as the proof.
- `docs/architecture.md:673-689` §10.7 — the design requirement ("input
  order is a retention priority") that V-1/V-2 verify empirically.
- `fixtures/README.md:34-48` — the `duplicates/` fixture set, including
  `order-a.pgn`/`order-b.pgn`'s purpose-built role in proving order-not-content
  retention.
- `src-tauri/src/jobs/run.rs:655-701` — `compute_final_metrics`, the
  implementation matching the "metrics trap" description (not itself
  V-2-tagged; offered as supporting, not citing, evidence).

### Consequences

- The `DuplicatePolicy` DTO has exactly three variants and cannot represent
  "both `-d` and `-D`"; illegal states are unrepresentable rather than
  guarded at runtime (`src-tauri/src/domain/operations.rs:67-76`;
  `src-tauri/src/engine/command_compiler.rs:207-221`).
- Choosing between the audit-file mode (`ReportAndKeepFirst`) and the
  silent-discard mode (`SuppressKeepFirst`) never changes which copy of a
  duplicate survives in the main output — only whether an audit file exists
  at all (`src-tauri/src/domain/operations.rs:63-64`;
  `docs/duplicate-semantics.md:58-68`).
- `output_games` is computed by counting the actual published main-output
  file rather than trusting the engine's "games matched" summary line, and
  this gap between the two numbers is documented to users as expected, not a
  bug (`src-tauri/src/jobs/run.rs:664,670-677`;
  `docs/duplicate-semantics.md:85-97`).
- Input order — not filename, modification time, or which copy has "better"
  annotations — is what the Files-screen reordering UI and its explanatory
  copy exist to control, per `docs/duplicate-semantics.md:37-56`.

### Gaps

- No date, author, commit hash, or CI run ID is cited anywhere for when/how V-1 and V-2 were established — unlike D-006's amendments, which cite specific GitHub Actions run numbers and commits. The only traceable fact is that the checked-in integration tests require the real, checksum-verified sidecar to pass its own startup self-test before running (src-tauri/tests/duplicate_integration.rs:49-56).
- The doc comment's claim that giving both -d and -D "exit[s] 1" (src-tauri/src/domain/operations.rs:59) is not traced to any pgn-extract source line the way some other findings in this codebase are (e.g. hashing.c is named directly for the virtual.tmp cleanup behavior in duplicate_integration.rs:741). Whether this was read from source or observed empirically is not stated by any citation found.
- No file explicitly defines "V-1 means X, V-2 means Y" as a rule. The split used above (V-1 = mutual exclusivity/byte-identical outputs; V-2 = keep-first 3-game shape + metrics trap) is inferred from which single-number tag each individual sentence in duplicate_integration.rs carries (D-007 V-1 vs D-007 V-2, as distinct from the joint D-007 V-1/V-2 tag used elsewhere). This is a faithful reading, not a transcription of an explicit definition.
- "design-02 §0 finding 1," cited alongside D-007 V-1/V-2 in operations.rs:56 and generated-types.ts:176, cannot be reconstructed: no design-02 document exists anywhere in this repository (searched), consistent with the ledger header's note that design-02 is a separate, lost series.
- The annotated-duplicate warning (docs/duplicate-semantics.md's "annotated-duplicate warning" section, and duplicate_integration.rs section B) is a closely related feature verified in the same test file, but no citation ties it to "V-1" or "V-2" specifically, so it was deliberately left out of this entry rather than folded in by assumption.

---

## V-3 — `--maxmoves`/`--minmoves` argument-order hazard (finding under D-007)

**Status:** verification finding, namespaced under D-007 per this ledger's own convention (this file's header: "`V-#` IDs... are namespaced under the decision that commissioned them"). D-007 itself (the engine capability map) is reconstructed above; this entry documents only what V-3's own citations establish.

### Finding

When a job specifies both a minimum and maximum move-count filter, the compiled engine invocation must emit `--maxmoves` before `--minmoves`. Emitting them in the reverse ("obvious", min-then-max) order does not error — the engine still exits 0 — but silently drops the upper bound whenever `max < 2*min - 1`, letting games longer than the intended maximum through (docs/engine-capabilities.md:182-187; src-tauri/src/engine/command_compiler.rs:231-233; src-tauri/src/engine/golden_tests.rs:348-352,375-376).

docs/engine-capabilities.md:182-184 additionally gives a mechanism: "The engine stores move bounds ply-encoded but compares them against the raw incoming move count during validation." No citation found points this explanation at pgn-extract's own source code, so it is treated here as the project's own derived explanation of observed behavior, not a confirmed fact about the engine's internals.

### Evidence

- **Compiler enforcement.** `src-tauri/src/engine/command_compiler.rs:229-241` (`// O-6: filter flags`) always pushes `--maxmoves` (with `bounds.max`) before `--minmoves` (with `bounds.min`), with an inline comment: "Hard: --maxmoves MUST precede --minmoves (DECISIONS-LEDGER.md D-007 V-3) — reversed, the engine silently drops the upper bound whenever max < 2*min - 1 and exits 0 anyway."
- **Type-level pointer.** `src-tauri/src/domain/filters.rs:164-171` documents the `MoveBounds` type as carrying the bounds only, stating the order rule "lives in `engine::command_compiler` (design-02 §0 finding 3, D-007 V-3, canonical order rule O-6b)" — the citing code attributes this hazard jointly to an external design document ("design-02 §0 finding 3") and to this ledger's D-007 V-3. `design-02` is not a file present in this repository (confirmed by a repo-wide filename search); per this ledger's own header, `design-02` is a separate, colliding, external ID series, so its "finding 3" content is not recoverable from this repo's citations — only the fact that filters.rs cites it jointly with V-3.
- **Golden regression test** (`g8_move_bounds_30_40_order_regression`, src-tauri/src/engine/golden_tests.rs:326-378). Asserts the compiled argv for `min=30, max=40` places `--maxmoves` before `--minmoves`, and additionally asserts `max < 2*min - 1` (i.e. `40 < 59`) computed from the fixture's own values. The comment explains this pair was deliberately chosen inside the "empirically verified trigger zone," since a pair like `min=10/max=20` would not reproduce the bug and "would give false confidence."
- **Fixture.** `fixtures/README.md:86` describes `move-bounds.pgn` (three games of exactly 3, 15, and 30 full moves) as reproducing this V-3 hazard: "with a 10–15 filter, correct order keeps only the 15-move game, reversed order silently admits the 30-move game too."
- **End-to-end empirical proof against the real engine binary**, not just the compiler (`src-tauri/tests/phase5_filters_integration.rs:915-993`, test `move_bounds_reversed_argument_order_silently_admits_a_too_long_game_empirical_proof`). The test first runs the correct compiled path (min=10, max=15) and confirms only the 15-move game survives, then bypasses the compiler entirely and invokes the pinned sidecar directly with `--minmoves 10 --maxmoves 15` (reversed order), asserting three things about that real process: (1) `run.status.code()` is `Some(0)` — "the engine still exits 0 — silent, not loud" (line 980); (2) stderr contains the engine's own text "Upper bound of ply limit is smaller than the lower bound" (line 984); (3) the output file contains 2 games, not 1 — "reversed order silently drops the upper bound: the 30-move game wrongly survives a [10, 15] filter alongside the 15-move game" (lines 990-991).
- **Documented in the human-readable capability record.** docs/engine-capabilities.md:182-190, under "Other verified engine behavior worth knowing" — a document whose header (lines 8-10) states every claim in it "was independently verified by the coordinator running the real engine against purpose-built fixtures" and cites `DECISIONS-LEDGER.md D-007/D-013` — restates the hazard and adds that PGN Studio's compiler "always emits `--maxmoves` first, and a regression test specifically uses fixture values inside the trigger zone... so a future accidental reordering would be caught rather than passing by coincidence."
- The generated TypeScript type file (`src/ipc/generated-types.ts:584-593`) carries the identical doc comment as `filters.rs`'s `MoveBounds`, evidently ts-rs-generated from it — not independent evidence, the same source restated.

### Consequences

- `command_compiler.rs`'s `compile` function structurally cannot emit the reversed order — it hard-codes `--maxmoves` before `--minmoves` whenever `move_bounds` is set — so the hazard cannot recur through this code path short of an edit to that block.
- Both a unit-level golden test (g8) and an end-to-end integration test against the real pinned sidecar exist specifically to catch a regression of the emission order, and both deliberately use fixture values inside the "trigger zone" (`max < 2*min - 1`) rather than an arbitrary min/max pair, so a regression can't pass by coincidence with an untriggering pair.

### Note on reconstruction

This entry is assembled entirely from citations, consistent with this ledger's provenance note: every claim above is followed by the file:line that attests to it, and nothing here is transcribed from an original V-3 entry — none survives. No citation gives a date, an author, or the identity of whoever ran the empirical proof beyond "the coordinator" (docs/engine-capabilities.md:8); those are left unstated per this ledger's never-invent-never-placeholder rule rather than guessed at.

### Gaps

The citations establish the technical hazard, its trigger condition, the compiler's structural fix, and an empirical proof run against the real pinned engine binary — but they do NOT establish: (1) who discovered/verified this finding, when, or how; no date or author name appears in any V-3 citation (docs/engine-capabilities.md:8 says only "the coordinator"); (2) the full content of "design-02 §0 finding 3", which filters.rs:169 and generated-types.ts:590 cite jointly with D-007 V-3 — `design-02` is confirmed absent as a file from this repository (repo-wide search found none), so only the fact of the joint citation is recoverable, not design-02's own text or rationale; (3) V-3's place among D-007's other verification findings — operations.rs:56 shows D-007 also has V-1 and V-2 (duplicate-game handling), and command_compiler.rs:688/capability.rs:103 (plus roughly nine more sites) show D-007 also has V-4 and V-5 (the ECO attached-flag hazard and the no-separate-broken-output finding), but no citation states how many V-# findings D-007 has in total or in what order they were established relative to V-3; (4) the exact pgn-extract source location (file/function) responsible for the ply-encoding/raw-comparison behavior paraphrased at docs/engine-capabilities.md:182-184 — no citation points this explanation at pgn-extract's own source, so it is presented here as the project's own derived account of observed behavior, not a sourced fact about the engine's internals; (5) whether this finding was verified only against the pinned Windows sidecar or also cross-checked on the macOS build — D-006's amendments document separate macOS verification work in detail but never mention move-bounds or V-3, and no V-3 citation mentions macOS, so this is simply unaddressed by the evidence rather than confirmed either way; (6) any revision history for V-3 itself — unlike D-006/D-014, none of the citations describe this finding as amended, corrected, or superseded.

---

## V-4/V-5 — Findings under D-007 (reconstructed above): the separated `-e <path>` silent-failure hazard, and no engine flag can route broken games to a file of their own

**Status:** inferred active — cited from 11 files (excluding this ledger's
own citation) across Rust, TypeScript, tests, and fixture docs, with nothing
suggesting either finding was ever superseded; both remain enforced today by
`attached_flag` (V-4) and the two-variant `BrokenOutput` enum (V-5), cited
below. Both are `V-#` verification findings — established by running
the pinned engine, not decided (this ledger's own header) — and several
citing files write them jointly as **"D-007 V-4"** /
**"D-007 V-5"** (`src-tauri/src/engine/command_compiler.rs:688`,
`src-tauri/src/engine/capability.rs:103`, `src-tauri/src/domain/operations.rs:104`,
`src-tauri/src/domain/capability.rs:65`), consistent with the header's rule
that `V-#` IDs are namespaced under the decision that commissioned them. D-007
itself ("Engine capability map…") is reconstructed above; this entry reports
what citations to V-4 and V-5 individually assert. This ledger previously
carried one-line summaries of both ("Regex/criteria-file verification
finding" / "Capability-map verification finding"); what follows expands each
from its actual citation sites.

### V-4 — the separated `-e <path>` form is a silent data-loss hazard

**Finding, as citations describe it.** Passing the ECO-classification flag
and its file path as two separate argv elements (`-e`, then the path,
space-separated) rather than one attached token (`-e<path>`) does not
surface as a single, predictable error. `docs/engine-capabilities.md:172-173`
states the separated form's *most common* failure mode is loud, not silent:
"usually fails loudly (`Unable to open the ECO file eco.pgn.`, empty output,
exit 1)." `fixtures/golden/regex/README.md:82-83` documents the rarer,
more dangerous manifestation: "V-4 separately proved that `-e <path>`
(space-separated) fails *silently* with exit 0 and zero games extracted"
for that flag. The original ledger's own historical reproduction is
referenced by `src-tauri/tests/phase4_integration.rs:44-48` as recording an
exit code the file quotes as **"EXIT CODE 0"** (`phase4_integration.rs:48-49`,
a direct quotation of the original ledger text). That same comment
paraphrases V-4 as further noting that `-e`'s fallback chain — when the
flag's argument is lost to this bug — "tries `$ECO_FILE` then a literal
`eco.pgn` in the CWD" (`phase4_integration.rs:50-51`, paraphrase), and that
which of those, if either, is reachable in a given working directory is what
determines whether the failure surfaces as a hard exit or as a
silently-contaminated "success" (`phase4_integration.rs:51-53`, paraphrase).
Which of the three outcomes (loud exit 1, silent exit 0, or silent
wrong-data classification) occurs is environment-dependent, per
`docs/engine-capabilities.md:171-181`, which separately documents the third,
most dangerous variant: if a file literally named `eco.pgn` is independently
reachable via the engine's own fallback search (`$ECO_FILE` or CWD), "a
separated-form invocation could succeed while silently classifying against
the *wrong* ECO data" (quoted).

**Scope, as the citing code is careful to state it.** `fixtures/golden/regex/README.md:84-86`
treats V-4 as proven specifically for `-e`, and frames extending the same
avoidance to every other value-taking short flag (`-o`, `-t`, `-d`, `-c`,
`-v`) as a separate, broader policy choice, adopted "even where it has not
been individually proven to fail the same way" for those other flags. V-4
itself should not be read as having tested flags other than `-e`.

**Consequence in the compiler.** `src-tauri/src/engine/command_compiler.rs:687-696`'s
`attached_flag` helper is the single code path that builds every one of the
compiler's value-bearing short flags (`-d`, `-c`, `-t`, `-v`, `-e`, `-o` —
call sites at `command_compiler.rs:219,226,253,265,309,323`), and its doc
comment cites "DECISIONS-LEDGER.md D-007 V-4" directly for why: "the
separated form `-e <path>` is a catastrophic silent-data-loss hazard for at
least `-e`, so every value-bearing short flag uniformly uses the attached
form" (`command_compiler.rs:688-691`), also citing "design-02 Decision D-2"
(a design-02 ID, a different, unrelated numbering series per this ledger's
own header — not reconstructed here). `src-tauri/tests/eco_supplement_integration.rs:134-135`
independently states the same constraint when building its own `-e` argument:
"The attached `-e<path>` form is mandatory (DECISIONS-LEDGER.md D-007 V-4:
the separated `-e <path>` form silently fails)."

**Independent Phase-4 reconfirmation (not part of V-4 itself).**
`phase4_integration.rs:701-733` (`eco_attached_form_is_the_only_form_the_compiler_ever_emits`)
asserts the compiler's `attached_flag` helper never emits a bare `-e` token.
`phase4_integration.rs:743-809` (`eco_separated_form_is_catastrophic_empirical_proof`)
independently re-spawns the real pinned sidecar with the separated form and
asserts the result does not silently reproduce the correct, attached-form
output. That fresh run, done "for this task," produced exit 1 with empty
output in a clean per-job workspace with no ambient `eco.pgn` — a different
exact manifestation than the ledger's recorded "EXIT CODE 0" — which the test
file attributes to "a different working-directory fallback outcome," not to
any dispute of V-4's conclusion: "The ledger's core conclusion - the attached
form is mandatory - is independently reconfirmed, not disputed."
(`phase4_integration.rs:44-59`, paraphrase with the one quoted fragment noted
above.)

### V-5 — no single engine invocation can route broken games to a file of their own

**Finding, as citations describe it.** `src-tauri/src/engine/capability.rs:102-106`'s
doc comment for the `separate_broken_output` capability field states it
"must stay `false`. Empirically verified impossible in one pass
(DECISIONS-LEDGER.md D-007 V-5): without `--keepbroken` broken games are
dropped everywhere; with it they land in the *main* output. There is no flag
that routes them to their own file." The domain-level `BrokenOutput` enum
doc comment (`src-tauri/src/domain/operations.rs:103-111`) states the same
conclusion: "There is deliberately no 'separate file' variant: empirically, a
single `pgn-extract` invocation cannot route broken games to their own
output — without `--keepbroken` they are dropped everywhere (including the
non-matching file); with it they land in the *main* output." That same
comment also cites this alongside a design-02 ID, "D-007 V-5, D-6"
(`domain/operations.rs:104`) — `D-6` belongs to the separate design-02
numbering series per this ledger's own header and is not reconstructed here.

**Enforced as unrepresentable, not merely documented.** `domain/operations.rs:110-111`
states the mechanism directly: "Making the impossible option unrepresentable
(rather than accepting it and silently downgrading it) is the enforcement
mechanism for architecture.md §29 here." Concretely, `BrokenOutput`
(`domain/operations.rs:112-119`) has exactly two variants — `Discard`
(default) and `KeepInMainOutput` — and no third, "separate file" variant
exists to select. `domain/capability.rs:63-68` mirrors the same field and
rationale at the domain-type level ("design-02 D-007 V-5"), and
`src/ipc/generated-types.ts:60-66,236-244` mirrors both the field and its
doc comment into the generated TypeScript IPC types.

**Consequence in product copy and presets.** `src/features/operations/ModeAndValidationSection.tsx:9-13`
states this as a binding wording constraint: "there is no separate
broken-games file — the engine cannot produce one in a single pass (D-007
V-5). Only 'discard' and 'keep in main output' are offered, and both say
plainly that games with errors are reported in the log, never a file of
their own." `src/state/presets.ts:14-16` states "No preset may claim a
separate broken-games file exists (D-007 V-5)" and that every built-in
preset uses `broken: "discard"` and never mentions a broken-games artifact
in its description — enforced by a dedicated test,
`src/state/presets.test.ts:22-26` ("never claims a separate broken-games
file (D-007 V-5)"), which asserts every preset's `operations.broken` is
`"discard"` and that no preset description contains the word "broken".

**Independent Phase-4 reconfirmation.** `phase4_integration.rs:1130-1167`
(`default_drops_broken_game_from_output_entirely`) runs the real engine on a
mix of one valid and one illegal-move game and asserts the broken game is
dropped from the output entirely under the default policy. `phase4_integration.rs:1169-1214`
(`keepbroken_lands_broken_game_in_the_main_output`) runs the same input with
`--keepbroken` and asserts the broken game lands in the *same* main output
file, with an explicit assertion that no `broken-keep.broken.pgn` file is
ever produced: "no separate broken-games file is ever produced (D-007 V-5)"
(`phase4_integration.rs:1210-1213`).

### Consequences

- Every value-bearing short flag the compiler emits (`-d`, `-c`, `-t`, `-v`,
  `-e`, `-o`) goes through one helper, `attached_flag`, that always produces
  a single attached token — never a separated flag/value pair —
  specifically because of V-4 (`command_compiler.rs:687-696,219,226,253,265,309,323`).
- Two regression tests guard the ECO flag specifically against the V-4
  regression: one asserts the compiler never emits a bare `-e` token
  (`phase4_integration.rs:701-733`), the other re-proves empirically that the
  separated form does not silently work (`phase4_integration.rs:743-809`).
- `EngineCapabilities.separate_broken_output` is a permanent `false` in the
  pinned capability map, kept as an explicit field "so a future capability
  consumer never assumes otherwise" rather than removed
  (`engine/capability.rs:102-109`, `domain/capability.rs:63-68`).
- `BrokenOutput` has no "separate file" variant by construction
  (`domain/operations.rs:112-119`), and the UI, product copy, and every
  built-in preset are constrained to match — no wording anywhere in the
  product may claim a separate broken-games file exists
  (`ModeAndValidationSection.tsx:9-13`, `presets.ts:14-16`,
  `presets.test.ts:22-26`).
- Both findings were independently re-run against the real pinned sidecar
  during the Phase 4 task rather than only trusted from the historical
  record, and both re-runs reconfirmed rather than disputed the original
  conclusions (`phase4_integration.rs:44-59,701-809,1130-1214`).

### Gaps

The original ledger's actual prose for V-4 and V-5 is lost; almost everything above is paraphrase of what citing code asserts the findings said, not transcription. Only two short fragments are direct quotations of the original ledger text, both attested by phase4_integration.rs's comment quoting them: the exit-code result "EXIT CODE 0" (phase4_integration.rs:48-49) and the description of V-4's fallback chain "tries `$ECO_FILE` then a literal `eco.pgn` in the CWD" (phase4_integration.rs:50-51, itself phrased there as paraphrase, not marked as a direct quote, so treat even this cautiously).

Not established by any citation found:
- Who performed the original V-4/V-5 verification, on what date, or by what exact method/commands (contrast with D-006's amendments, which cite specific run IDs and commits — no such provenance survives for V-4/V-5).
- Whether V-4 and V-5 were formally, explicitly filed "under" D-007 in the original ledger's own structure, versus simply being cited alongside D-007 by several (not all) citing files as a convention. The fixtures/golden/regex/README.md citation of V-4, for instance, does not mention D-007 at all.
- Any alternatives considered, or why the fallback chain for `-e` behaves as described (that mechanism is stated as something "the ledger's V-4 itself notes," not independently verified by this reconstruction against pgn-extract's source).
- Whether V-4's "silent failure" hazard was ever tested for engine flags other than `-e` — the citing README explicitly frames this as unproven and the broader attached-form policy as a separate, precautionary choice, not part of V-4's own tested scope.
- Any exact byte counts, hashes, or engine invocation transcripts from the original V-4/V-5 verification runs (unlike, e.g., D-006's amendments, no raw run output for V-4/V-5 survives in any file found).

---

## V-6 — PowerShell mangles attached-form flags built by string interpolation; pass argument arrays, never shell strings

**Status:** reconstructed here for the first time. Until now this ID appeared
only as a row in this ledger's own "Cited but not recorded" table; no entry
existed. Per the header, `V-#` IDs are "namespaced under the decision that
commissioned them" — but neither citation site for V-6 states which `D-###`
that is. See Gaps.

### Finding

PowerShell silently mangles an *attached-form* flag — a short option with
its value glued directly onto it, e.g. `-oPATH` or `-tPATH` — when that
flag+value is assembled by string interpolation or concatenation into a
shell command line, rather than passed as a single, literal element of a
PowerShell argument array. Two independent files state this rule and both
attribute it to this ledger, without restating what the mangled result
actually looks like:

- `scripts/lib/engine-common.ps1:54-58` (paraphrase, as attributed by the
  comment): "PowerShell mangles attached-form flags like `-oPATH` when
  they're built via string interpolation/concatenation instead of passed
  as a literal array element." This comment cites it as **"decisions ledger
  V-6"** — no "finding" in its wording.
- `fixtures/golden/regex/README.md:76-79` (paraphrase, independent
  restatement, adds "silently"): "decisions ledger finding V-6 documented
  PowerShell silently mangling attached-form flags (e.g. `-o<path>`) when
  built via string interpolation instead of passed as a literal array
  element." Only this second site's wording includes "finding."

### Evidence of the finding being acted on

- `scripts/lib/engine-common.ps1:59-77` — the `Invoke-Native` helper
  requires `[Parameter(Mandatory)][string[]]$Arguments`
  (`scripts/lib/engine-common.ps1:62`) and spawns the process with
  PowerShell's splat operator, `& $Exe @Arguments`
  (`scripts/lib/engine-common.ps1:69`) — never a single interpolated
  command string. **Its scope is narrower than a first read of its comment
  suggests.** Despite its comment citing V-6, `Invoke-Native`'s only actual
  call sites in the tree are `git` operations: `git apply`
  (`scripts/build-pgn-extract.ps1:157`), and `git fetch`/`git checkout`
  (`scripts/lib/engine-common.ps1:101-102,143`). It is never used to spawn
  `pgn-extract` — it is a shared native-process-invocation helper that
  embodies the V-6 rule generally, not the sidecar's spawn path.
- `scripts/verify-engine.ps1` invokes the sidecar directly — not through
  `Invoke-Native`, which it never calls at all — in at least three places,
  and each independently follows the same array-element/attached-form
  pattern: Layer 1's identity check (`$versionOutput = & $binaryPath
  "--version" 2>&1`, `verify-engine.ps1:199`), Layer 2's `make`-driven
  upstream suite (`& make -k -C $testDir all "PGN_EXTRACT=$binForMake" ...`,
  `verify-engine.ps1:300`), and Layer 3's regex-fixture runner, which builds
  its argument list as a `[System.Collections.Generic.List[string]]` with
  each attached-form flag as its own `.Add(...)` element (e.g.
  `$cliArgs.Add("-o$tempOut")`, `verify-engine.ps1:428`).
- `fixtures/golden/regex/README.md:68-74` — the documented
  golden-fixture-regeneration command shows each token (`-t<path>`,
  `-o<path>`, `--quiet`, the input path) as its own separate string passed
  to `& $exe`, continued across lines with backtick line-continuation
  rather than joined into one string, e.g. line 70:
  `& $exe "-t$pwd/fixtures/golden/regex/<name>-criteria.txt" `.
- `fixtures/golden/regex/README.md:81-86` explicitly distinguishes V-6 from
  the separate finding V-4: V-6 is about how PowerShell handles a
  flag+value string before the process is even spawned, while V-4
  (tied explicitly to "D-007" in `src-tauri/src/engine/command_compiler.rs:688`)
  is about `pgn-extract` itself silently mis-handling the *separated* form
  (`-e <path>`) regardless of shell. The README uses V-6 as reason to use
  the attached form "as a matter of policy" for every value-taking short
  option, "even where it has not been individually proven to fail the same
  way" as V-4's flag (`fixtures/golden/regex/README.md:85`).
- `docs/architecture.md:1186-1191` ("16.2 Controls") states the general
  rules "Never invoke a shell" (line 1188) and "Pass arguments as OS-native
  tokens" (line 1189) for the Rust/Tauri side. The `engine-common.ps1`
  comment cites this section jointly with V-6
  (`scripts/lib/engine-common.ps1:55`) as parallel authority for the same
  practice in the PowerShell tooling, though §16.2 itself does not mention
  PowerShell, string interpolation, or V-6 by name — it is a general
  policy statement, not independent corroboration of the specific
  empirical finding.

### Consequences

- `Invoke-Native` (`scripts/lib/engine-common.ps1:59-77`) is a shared
  native-process-invocation helper — used by both the build and verify
  scripts for their `git` calls — whose mandatory `[string[]]$Arguments`
  parameter and splat-based spawn forecloses passing a pre-built command
  string through it. It is one place the V-6 rule is embodied, not the sole
  or primary one: it is never used to invoke `pgn-extract`.
- The golden-fixture regeneration instructions in
  `fixtures/golden/regex/README.md:66-86` codify array-element,
  attached-form invocation as the required method for anyone adding or
  regenerating a `fixtures/golden/regex/*` case by hand.
- `scripts/verify-engine.ps1`'s three direct sidecar/tool invocations
  (Layer 1's `--version` check, Layer 2's `make` invocation, and Layer 3's
  regex-fixture runner) each independently follow the same
  array-element/attached-form pattern, without going through `Invoke-Native`
  at all.

### Gaps

- **Parent decision not established.** This ledger's header says `V-#` IDs
  are namespaced under the `D-###` that commissioned them, and V-4 is
  explicitly tied to "D-007" at its own citation site
  (`src-tauri/src/engine/command_compiler.rs:688`). Neither V-6 citation
  site does this — both say only "decisions ledger V-6" /
  "decisions ledger finding V-6" with no `D-###` prefix. Which decision
  commissioned V-6 is not established by these citations and is not
  guessed at here.
- **The exact mangled result is not stated.** Both citations assert
  PowerShell mangles the flag when built via interpolation and prescribe
  the fix (pass as an array element), but neither describes what the
  mangled output actually was (a dropped value, a split token, an altered
  flag, etc.), nor the exact PowerShell mechanism responsible. "Silently"
  appears only in the `fixtures/golden/regex/README.md` citation, not in
  the `engine-common.ps1` comment.
- **No date, author, or original wording.** Per this ledger's own
  provenance note, the original ledger is lost. Nothing in either V-6
  citation site supplies when this was found, by whom, or the ledger's
  original phrasing — both citing files paraphrase/attribute rather than
  quote the original verbatim.

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

D-002, D-007, D-008, D-009, D-010, D-013, V-1, V-2, V-3, V-4, V-5, and V-6
were previously listed here as citation-only gaps. All twelve have since
been reconstructed above from their citation sites and are no longer listed
in this section.

What remains below are IDs with **zero citations anywhere in the tree** —
not thin evidence, not a single pointer, nothing. A repository-wide,
case-insensitive grep for each of `D-001`, `D-003`, `D-004`, `D-005`,
`D-011`, and `D-012` finds no match in any file except this ledger's own
D-014 entry, which names the gap directly in its "On this ID" note. These
six IDs cannot be reconstructed by any method available to this project:
reconstruction here works only by tracing a decision's effects forward
through the files that cite it, and a decision with no surviving citation
left no traceable effects to trace. They remain open gaps, not invented
entries — inventing plausible content for them would be exactly the
failure this ledger's header rule exists to prevent.

| ID | Status |
| --- | --- |
| D-001 | Zero citations anywhere in the tree. Unrecoverable. |
| D-003 | Zero citations anywhere in the tree. Unrecoverable. |
| D-004 | Zero citations anywhere in the tree. Unrecoverable. |
| D-005 | Zero citations anywhere in the tree. Unrecoverable. |
| D-011 | Zero citations anywhere in the tree. Unrecoverable. |
| D-012 | Zero citations anywhere in the tree. Unrecoverable. |

**One deliberate near-miss, worth recording so it is never mistaken for a
citation:** `src-tauri/src/engine/command_compiler.rs:224` carries the
comment `// O-5 (preset only: "New Games Against Master", Decision D-11).`
That `D-11` is a two-digit ID from **design-02's own, separate numbering
series** — the same series this ledger's header warns collides with
three-digit IDs in short form (`D-11` vs. `D-011`) — and is unrelated to
this ledger's `D-011`. It must not be counted as evidence for, or a
citation of, this ledger's D-011 entry.
