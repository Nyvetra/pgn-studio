# engine-src/

This directory pins the exact upstream `pgn-extract` revision (and, for
Windows, the TRE regex library it is statically linked against) that PGN
Studio builds its sidecar from (architecture.md §10.1) and records how
each pin was produced and can be independently re-verified.

PGN Studio does **not** vendor the `pgn-extract` or TRE C sources into
this repository. `engine-src/upstream.lock` is the pin (schema version 2,
below); `patches/` holds any local patches on top of it - currently none
for either project's *source* (see `patches/tre-msvc/README.md` for a
build-recipe deviation that changes zero source bytes); `manifest/`
holds PGN Studio's own UTF-8 code-page manifest embedded into the Windows
binary. The actual build is `scripts/build-pgn-extract.ps1` (Windows,
implemented and verified) / `scripts/build-pgn-extract.sh` (macOS,
written but unverified - no Mac available); see `scripts/README.md`.

## What `upstream.lock` records (schema version 2)

```json
{
  "schemaVersion": 2,
  "engine": {
    "name": "pgn-extract",
    "repository": "https://github.com/kentdjb/pgn-extract",
    "mirror": "https://github.com/Nyvetra/pgn-extract-mirror",
    "commit": "e69e863b70f2fb8ed7916752db95e1c771daf4f0",
    "gitTree": "f1fbf5ba594d5f5609f746749ea29d46769f55a6",
    "version": "v26-06",
    "license": "GPL-3.0-or-later",
    "sourceArchiveSha256": "8acaa7167ea3f7dc9e87210368f51efacf85f7ad8f8a2c94719a44490999c1cb",
    "resources": { "eco.pgn": { "...": "sha256 + size" }, "COPYING": { "...": "sha256 + size" } },
    "patches": []
  },
  "regex": {
    "windows": {
      "name": "tre", "repository": "https://github.com/laurikari/tre",
      "mirror": "https://github.com/Nyvetra/tre-mirror",
      "tag": "v0.9.0", "commit": "d0e0c997336b3210f05b3e1daa7bb5cb9900d274",
      "license": "BSD-2-Clause", "linkage": "static"
    },
    "macos": { "name": "system-libc-regex", "linkage": "libc" }
  },
  "toolchains": { "...": "per-target compiler flags, recorded for §10.1 'compiler and target information'" },
  "lastVerified": "2026-08-07"
}
```

The pin's integrity anchor is the **git commit + tree hash** for both
projects, verified by `scripts/build-pgn-extract.ps1` after every
clone/checkout (`git rev-parse HEAD` and `HEAD^{tree}` must equal the
lock; hard failure on mismatch, no silent fallback to an unpinned
checkout). `sourceArchiveSha256` (pgn-extract only - TRE has no
equivalent field, since its integrity is via git commit+tree alone) is a
secondary, independent check performed by `scripts/verify-engine.ps1`'s
Layer 0.

### Why the schema changed from version 1 (flat) to version 2 (nested)

Phase 0 recorded a flat set of top-level fields
(`name`/`repository`/`commit`/...). Phase 0b added the TRE pin, upstream
mirrors, and per-toolchain build flags, which the flat shape could not
represent cleanly - see design-01-engine-build.md §6.1 for the approved
richer schema. `scripts/verify-engine.ps1` and
`scripts/build-pgn-extract.ps1` both read the nested shape
(`$lock.engine.*`, `$lock.regex.windows.*`, `$lock.toolchains.*`); there
is no other consumer of this file to migrate.

## How each field was produced (2026-08-07)

1. **`commit`** - cloned `https://github.com/kentdjb/pgn-extract.git`
   (default branch `main`) and read `git rev-parse HEAD`:
   `e69e863b70f2fb8ed7916752db95e1c771daf4f0`.

2. **`version`** - the pinned commit's `argsfile.c` defines
   `#define CURRENT_VERSION "v26-06"` (also printed by the program itself
   via `pgn-extract --version`-style output). This is the authoritative
   engine version string, not a git tag (upstream does not consistently tag
   releases).

3. **`license`** - inspected source file headers (e.g. `main.c`) and the
   repository's `copyright` file at the pinned commit. Both state:

   > "either version 3 of the License, or (at your option) any later
   > version"

   which is the standard FSF language for **GPL-3.0-or-later**, not
   `GPL-3.0-only`. This satisfies architecture.md §17.1's requirement to
   inspect upstream headers before choosing between the two before picking
   a project license.

4. **`sourceArchiveSha256`** - downloaded GitHub's generated commit archive
   twice, independently, and confirmed both downloads were byte-identical:

   ```sh
   curl -sL -o a.tar.gz \
     https://github.com/kentdjb/pgn-extract/archive/e69e863b70f2fb8ed7916752db95e1c771daf4f0.tar.gz
   curl -sL -o b.tar.gz \
     https://github.com/kentdjb/pgn-extract/archive/e69e863b70f2fb8ed7916752db95e1c771daf4f0.tar.gz
   sha256sum a.tar.gz b.tar.gz
   # both: 8acaa7167ea3f7dc9e87210368f51efacf85f7ad8f8a2c94719a44490999c1cb
   ```

   `scripts/verify-engine.ps1` automates this check (download + hash
   comparison against `sourceArchiveSha256` in this file) - run it whenever
   `upstream.lock` changes, and again immediately before cutting a release.

   Note on reproducibility: this checksum is of *GitHub's generated
   tarball* for the commit (`codeload.github.com`), not of a signed
   upstream release artifact - `pgn-extract` does not publish signed
   release archives. GitHub's codeload archives have historically been
   stable/reproducible for a given commit, but if a future re-verification
   ever produces a different hash for the same commit SHA, treat that as a
   signal to investigate (possible GitHub archive-format change) rather
   than silently re-pinning.

   **Pitfall encountered while producing this pin, worth recording:** the
   individual per-file checksums recorded for `COPYING` and `eco.pgn` in
   `src-tauri/resources/pgn-extract/SOURCE.json` must be computed from
   files extracted directly from this verified tarball (`tar xzf ... `),
   **not** from a local `git clone` of the upstream repository. A local
   clone's line endings can be silently rewritten by the cloning machine's
   own `core.autocrlf` setting (this happened during Phase 0 development:
   a Windows checkout with `autocrlf=true` converted upstream's native LF
   line endings to CRLF, which changed the files' bytes/hashes without
   changing their meaning). Always extract from the checksum-verified
   archive when computing a file-level hash that will be published, so it
   stays tied to the same verified artifact as `sourceArchiveSha256`.

5. **`eco.pgn` provenance** - `eco.pgn` at the pinned commit carries its own
   in-file header:

   > "I believe that the original file from which I generated this was put
   > together by Ewart Shaw, Franz Hemmer and others, to whom appropriate
   > thanks and acknowledgement is due. Permission has been granted for its
   > inclusion with the pgn-extract program [...] David J. Barnes"

   This is a permission-to-redistribute-with-pgn-extract statement, **not**
   a separately declared open-source license for `eco.pgn` itself. Neither
   the file nor the upstream repository states an independent license for
   it. `THIRD_PARTY_NOTICES.md` and
   `src-tauri/resources/pgn-extract/SOURCE.json` report exactly that -
   permission for inclusion, uncertain/unstated standalone license -
   rather than asserting a license (e.g. CC0/MIT) that was never declared.

## Re-verifying this pin yourself

```sh
git clone https://github.com/kentdjb/pgn-extract.git
cd pgn-extract
git rev-parse HEAD              # compare against engine.commit above
git rev-parse HEAD^{tree}       # compare against engine.gitTree above
grep -n "CURRENT_VERSION" argsfile.c
grep -n "any later version" main.c copyright

git clone https://github.com/laurikari/tre.git
cd tre
git checkout v0.9.0
git rev-parse HEAD              # compare against regex.windows.commit above
git rev-parse HEAD^{tree}       # compare against regex.windows.gitTree above
```

Then run `pwsh ./scripts/verify-engine.ps1` (Layer 0) to re-download the
pinned pgn-extract commit's archive and confirm its SHA-256 still matches
`engine.sourceArchiveSha256`; `pwsh ./scripts/build-pgn-extract.ps1`
itself re-verifies both commits' HEAD *and* tree hashes on every run
(hard failure on mismatch - see that script's `Get-PinnedCheckout`
helper in `scripts/lib/engine-common.ps1`).

## Reproducibility

Re-verifying the *source* pin (above) only proves you have the right
inputs. PGN Studio's integrity design (architecture.md §16.2/§20.2-§20.4)
goes one step further and depends on the *build* being reproducible too:
a two-gate SHA-256 chain (CI packages the binary and records its hash;
the Rust host verifies that hash at startup before ever invoking the
sidecar - see `scripts/verify-engine.ps1` Layer 1's ENGINE_TAMPERED
check) is only as trustworthy as the assumption that the recorded hash
*is* what the pinned sources produce. Because this project is
GPL-3.0-or-later, that assumption also underwrites the corresponding-source
obligation: a user should be able to clone this repo at the commit that
shipped a given release, run `scripts/build-pgn-extract.ps1`, and get a
byte-identical `pgn-extract-x86_64-pc-windows-msvc.exe` back - not merely
"a working one."

That last sentence carries one load-bearing qualifier that the rest of
this section spells out: **on the same MSVC toolset**. `/Brepro` removes
*time* as a build input, not *toolchain version* - and `upstream.lock`
pins the engine sources, not the compiler that consumes them. See "What
`/Brepro` does not fix" below for the measured evidence, and note that
nothing in the integrity design depends on cross-toolset equality:
`checksums.json` is written by the same build that produces the binary
it describes, so Layer 1 compares a binary against its own build's hash,
never against a hash pinned somewhere else.

### Why it wasn't reproducible before, and what fixed it

MSVC's `link.exe` stamps the real wall-clock build time into the PE
header's `TimeDateStamp` field by default, so two builds from
byte-identical sources produced different SHA-256 hashes every time.
Confirmed on this machine before the fix (same pinned commit, same
flags, back-to-back builds):

```
0d551ceaa3b76fac8c97d668fb7f1283b42bf99c0d49802bf19e314b89c7119f
dffeff71d6bd7f427f057413f47185ab07fcf12ee01d244f258a9d0633307ddc
```

The fix is MSVC's `/Brepro` switch, applied uniformly across every
compile, archive, and link step:

- **`engineFlags`/`treFlags`** (`upstream.lock`, consumed by `cl.exe` when
  compiling pgn-extract's and TRE's `.c` files respectively) now include
  `/Brepro`.
- **`linkFlags`** (new field, `upstream.lock`) is `["/Brepro"]`, applied
  to **both** `lib.exe` (archiving TRE's objects into `tre.lib`) and
  `link.exe` (linking `pgn-extract.exe`) - Microsoft's own guidance is
  that `/Brepro` must be passed at every stage that stamps a timestamp,
  not just the final link.
- Source files handed to `cl.exe`/`lib.exe`/`link.exe` are now sorted in
  a fixed ordinal order (`Get-OrdinalSorted`, `scripts/lib/engine-common.ps1`)
  instead of relying on `Get-ChildItem`'s filesystem-enumeration order,
  which is an OS/filesystem implementation detail, not a documented
  contract - `/Brepro` makes a *given* input order byte-reproducible, it
  doesn't fix the order itself.

With `/Brepro` on cl.exe, `dumpbin /headers` no longer shows a plausible
calendar date in the PE header's time-date-stamp field - it shows a
content-hash-derived value instead (e.g. `E382E31A`, which is not a
valid-looking Unix timestamp), consistent with Microsoft's documented
behavior of substituting a hash of the binary for the real time.

### The `__DATE__`/`__TIME__` finding

pgn-extract's `argsfile.c` calls `fprintf(..., CURRENT_VERSION, __DATE__)`
in its `--help` banner (not in `--version`, which prints only
`CURRENT_VERSION` and is unaffected). `__DATE__` expands to the
compiler's real compile-time date, embedded as a string literal - a
second, independent source of non-reproducibility beyond the PE
timestamp, and one that could not be fixed by patching (zero source
edits is a hard project invariant, and pgn-extract/TRE source is not
vendored into this repo).

It turns out `/Brepro` fixes this too, as an intentional part of its
documented behavior: it also neutralizes `__DATE__`, `__TIME__`, and
`__TIMESTAMP__` to a fixed placeholder rather than the real compile
date/time. Verified directly against this toolchain (MSVC 19.44, VS2022
Build Tools) via a preprocessor dump:

```
without /Brepro: printf("hello %s %s\n", "Aug  7 2026" , "18:46:10" );
with    /Brepro: printf("hello %s %s\n", "1" , "1" );
```

...and confirmed in the actual shipped binary - `pgn-extract --help` now
prints `pgn-extract v26-06 (1): ...` where it used to print the real
compile date. This has no effect on anything that is actually checked:
`--version`'s output (the smoke check in `build-pgn-extract.ps1` and
Layer 1 of `verify-engine.ps1`) doesn't use `__DATE__` at all, and
upstream's own `test/Makefile` target that exercises `-h`
(`test-h`, the one target whose recipe line is intentionally
`-`-prefixed so its exit status is ignored) has no oracle-diff against
its output. TRE's sources use neither macro. Net result: on a fixed
toolset, this build is byte-reproducible **including** across different
calendar days, without patching a single upstream source byte.

### What was checked and found to be a non-issue

- **`/Zi`-style debug info / embedded absolute paths**: neither
  `engineFlags` nor `treFlags` include `/Zi`/`/Z7`/`/ZI`, so no PDB is
  generated and no CodeView debug section exists to embed a build-machine
  path into. A byte scan of the linked `.exe` for the build cache's own
  absolute path found no match, and `dumpbin` shows no `.pdb` reference -
  consistent with `link.exe` (no `/DEBUG`) discarding objects' private
  symbol-table entries rather than carrying them into the final image.
- **The embedded UTF-8 manifest** (`/MANIFEST:EMBED /MANIFESTINPUT:...`):
  its *content* is a static, checked-in XML file, and the resource
  directory link.exe builds around it is covered by the same `/Brepro`
  link step as everything else - part of why the final `.exe` hash is
  stable, not a separate mechanism.
- **Intermediate artifacts are a different story, and that's fine**:
  individual `.obj` files and the intermediate `tre.lib` (both live only
  in the gitignored `engine-src/.build/` cache, never shipped or
  checksummed) are **not** necessarily byte-identical between rebuilds
  even with `/Brepro` - a byte diff between two builds' `regcomp.obj`
  showed 13 differing bytes out of 5510, confined to the COFF
  `TimeDateStamp` field and one small cluster, both plausibly
  `/Brepro`'s own per-object repro-hash bookkeeping. This never
  propagates to the final `pgn-extract.exe`, which is the only artifact
  this project checksums (`src-tauri/binaries/checksums.json`) or ships -
  confirmed empirically below.

### The proof

From a fully clean state (`engine-src/.build/` and the installed `.exe`
both removed, so every run re-clones and recompiles from nothing but the
pinned commit + `upstream.lock`'s flags):

```
pwsh -NoProfile -Command "Remove-Item -Recurse -Force engine-src\.build, src-tauri\binaries\pgn-extract-x86_64-pc-windows-msvc.exe -ErrorAction SilentlyContinue"
pwsh ./scripts/build-pgn-extract.ps1
```

Run three times in a row this way, each preserving its installed `.exe`
before the next wipe, then hashed together in one command:

```
03909cf9700d6948588ecf75826b0146c7dc7012d521d62c92f7c2c843a5da52   (run 1)
03909cf9700d6948588ecf75826b0146c7dc7012d521d62c92f7c2c843a5da52   (run 2)
03909cf9700d6948588ecf75826b0146c7dc7012d521d62c92f7c2c843a5da52   (run 3)
```

Identical - 426496 bytes each. `pwsh ./scripts/verify-engine.ps1`
afterwards passed all four layers unchanged (76/76 upstream targets,
6/6 supplemental regex goldens), and `dumpbin /dependents` still shows
`KERNEL32.dll` only - reproducibility didn't cost anything else this
build already had. This is a point-in-time confirmation (this pinned
commit, these flags, this VS2022/MSVC 19.44 toolset) - re-run the steps
above yourself any time you want to re-confirm it, e.g. after an
upstream pin bump or a Visual Studio update.

### What `/Brepro` does not fix: the MSVC toolset is itself an input

The three-run proof above holds the compiler constant. It has since been
measured across *different* compilers, and there the binaries differ -
as they should:

| toolset | sha256 | size |
| --- | --- | --- |
| MSVC 19.44 (VS2022 Build Tools) - the proof above, 3/3 runs | `03909cf9700d6948588ecf75826b0146c7dc7012d521d62c92f7c2c843a5da52` | 426496 |
| MSVC 19.51.36252 (VS 18, GitHub Actions `windows-latest`, run 31432037434) | `afb4d97a5ccc82c466814c1f433341b116d58b623732ecc253eb8af0b6e05f4b` | 427520 |
| a third machine (toolset version not recorded at the time) | `5395d991...` | 427008 |

Three different toolsets, three different binaries, three different
sizes. This is expected and is **not** a `/Brepro` failure: `/Brepro`
neutralizes wall-clock timestamps and the `__DATE__`/`__TIME__`/
`__TIMESTAMP__` macros, and it does exactly that. It cannot make two
different code generators emit the same code. In reproducible-builds
terms the toolchain is part of the input set, and `upstream.lock` pins
the *engine* inputs (`engine.commit`, `regex.windows.commit`, the flag
lists) - it does not pin `cl.exe`/`link.exe`, only the
`vswhereRequirement` component ID needed to *find* some MSVC.

What this does and does not cost:

- **Neither gate of the integrity chain is affected.** Both compare a
  binary against a hash recorded by *the same build that produced it*, so
  neither has an expected value pinned in git or in this document:
  - *Gate 1 (`verify-engine.ps1` Layer 1)* reads
    `src-tauri/binaries/checksums.json`, which `build-pgn-extract.ps1`
    writes at the end of its own run.
  - *Gate 2 (Rust startup self-test)* compares the sidecar against
    `EngineIdentity.sha256`, which `src-tauri/src/engine/capability.rs`
    pulls from `build-info-<triple>.json` via `include_str!` at Rust
    *compile* time. Both files are gitignored build output.

  The real constraint this creates is an ordering one, not a
  reproducibility one: the engine must be built before `cargo`/`tauri
  build` compiles the Rust, on the same machine or CI run, or the crate
  either fails to compile (no `build-info-*.json` to embed) or embeds a
  hash belonging to a different build and fails at startup with
  `ENGINE_TAMPERED`. That is why every workflow that compiles the Rust
  crate has to build the sidecar first.

  `build-info-<triple>.json` also records the exact `compiler` banner,
  which is how the CI row in the table above was identified.
- **The GPL corresponding-source story needs the qualifier.** A rebuilder
  gets a byte-identical binary only if they use the same MSVC toolset
  version; on a different one they get an equivalent, working binary with
  a different hash. That is the normal state of affairs for
  non-toolchain-pinned C projects, but it should be stated rather than
  implied.
- **Do not treat a hash difference across machines as tampering.** Compare
  `build-info-<triple>.json`'s `compiler` field first; if the toolsets
  differ, differing hashes are the expected result, and the meaningful
  comparison is `verify-engine.ps1`'s Layers 2 and 3 (behaviour), not
  Layer 1 (identity of a specific build).

Pinning the toolset exactly (a fixed VS build-tools version in CI, or a
container image) is the only way to promote the claim to a
cross-machine one. That has not been done here, and is not required by
anything the project currently asserts.

### macOS mirror (unverified)

`scripts/build-pgn-extract.sh` mirrors the same intent for
`aarch64-apple-darwin`/`x86_64-apple-darwin`, via `upstream.lock`'s
`engineFlags`/`linkFlags` for those triples:

- `-D__DATE__="1" -D__TIME__="1" -Wno-builtin-macro-redefined` -
  Apple clang has no single flag equivalent to `/Brepro`'s macro
  neutralization, so this overrides the two builtin macros directly on
  the command line (redefining a builtin macro via `-D` is standard,
  well-supported clang/gcc behavior; the warning it would otherwise print
  is suppressed). Deliberately reuses `/Brepro`'s own placeholder value
  (`"1"`) so both platforms' `--help` output matches.
- `-Wl,-no_uuid` - drops Mach-O's `LC_UUID` load command, which `ld64`
  otherwise fills with a fresh random UUID on every link (the closest
  macOS equivalent of the PE `TimeDateStamp` problem).
- No `-g` is used (no debug info), so - as on Windows - there is no PDB/
  dSYM path-embedding concern to address.
- `clang`'s own `*.c` glob expansion is documented to sort its results
  (POSIX `glob(3)`, no `GLOB_NOSORT`), so unlike the Windows script's
  `Get-ChildItem`, no separate explicit-sort step was needed there.

**Update: it has now been run on real macOS hardware** (GitHub Actions
`macos-14` and `macos-15-intel`, run 31497585623). No Mac is available in
this development environment - decisions ledger D-006 - but the CI legs
are real hardware, and the script works on both architectures: Apple
clang 15.0.0 compiles and links the pinned commit, the smoke check
passes, and the sidecar installs (`aarch64-apple-darwin`: 214680 bytes,
sha256 `83c4eae6...`). `verify-engine.ps1` then passes Layer 0, Layer 1,
and **Layer 2 at 76/76** against that binary - the macOS build clears
pgn-extract's entire own regression suite.

Two caveats keep this at "builds", not "verified":

- **The reproducibility flags above are still unmeasured.**
  `-D__DATE__`/`-D__TIME__`/`-Wno-builtin-macro-redefined` and
  `-Wl,-no_uuid` are *applied*, and nothing about them broke the build,
  but no two-build comparison has ever been done on macOS. Their intended
  effect is unproven; do not extend the Windows reproducibility evidence
  to macOS.
- **Layer 3 passes 6/6 on macOS — after a fixture-level fix, not an
  engine change.** It first reported 0/6 there. Layer 3 compares output
  byte-for-byte against `fixtures/golden/regex/*-expected.pgn`, which are
  stored CRLF and pinned that way by `.gitattributes`
  (`fixtures/** -text`); the macOS engine emits LF. Stripping `\r` from
  each committed golden reproduced the macOS run's *actual* SHA-256
  exactly, in all six cases - so the macOS output was content-identical
  to Windows for literal, anchors, bracket, star, backreference, and the
  `grammar.c` odds call site. Apple's libc regex matched TRE on every
  supplemental case, first time.

  Layer 3 now compares byte-exact first and retries with CRLF/LF
  normalized only when that fails, reporting any such case as `[PASS~]`
  with a note, counting it separately in the summary, and recording
  `passedAfterNewlineNormalization` in the JSON report. Byte-exactness is
  not weakened where it already held: the Windows run still reports all
  six as byte-exact and never enters the fallback. Normalizing
  unconditionally would have been fewer lines and strictly worse - it
  would have stopped detecting a genuine CRLF/LF regression on Windows,
  which is precisely what these fixtures exist to catch.

Two bugs did surface on that first run, both since fixed: the script was
committed non-executable (mode 100644, so CI could not run it at all),
and its `--version` smoke check captured stdout only while pgn-extract
writes `--version` to stderr. Keep treating macOS runs as verification
rather than formality - the first one found two real defects.

## Updating the pin

When PGN Studio moves to a newer upstream commit:

1. Re-run the verification steps above against the new commit(s).
2. Update every field for the changed project in `upstream.lock` together
   (commit, gitTree, version, sourceArchiveSha256, lastVerified) - never
   edit just one field. `engine` and `regex.windows` can be updated
   independently of each other (a pgn-extract bump does not necessarily
   require a TRE bump, and vice versa).
3. Re-check the license headers; do not assume the license is unchanged.
4. Re-review `patches/` (including `patches/tre-msvc/README.md`'s
   build-recipe fallback, if TRE moved) for conflicts against the new
   commit.
5. Run `pwsh ./scripts/build-pgn-extract.ps1` then
   `pwsh ./scripts/verify-engine.ps1` and require all layers to pass
   before relying on the new pin - do not hand-wave this step; a version
   bump that silently breaks the MSVC build or regresses a golden fixture
   is exactly what these scripts exist to catch.
