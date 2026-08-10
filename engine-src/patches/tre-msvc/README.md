# engine-src/patches/tre-msvc/

Build-recipe record for compiling TRE v0.9.0 as a static library under
MSVC on Windows (architecture.md §10.1/§10.2, design-01-engine-build.md
§6.2). **This directory contains no source-code diff against TRE or
pgn-extract - zero bytes of either upstream project are modified.** It
exists to satisfy the same disclosure obligation a real patch would:
recording, in one place, exactly how the Windows build deviates from
"run upstream's own build system unmodified," and why.

## Why this exists: the win32/ project files are not usable as shipped

TRE ships an in-repo Visual Studio build (`win32/tre.vcxproj`,
`win32/tre.sln`). At the pinned commit
(`d0e0c997336b3210f05b3e1daa7bb5cb9900d274`, tag `v0.9.0`) this project
file is not directly usable for PGN Studio's needs:

1. **It only targets the `Win32` (x86) platform.** The `.vcxproj`
   defines `Debug|Win32` and `Release|Win32` configurations only - no
   `x64` platform is configured. PGN Studio ships
   `x86_64-pc-windows-msvc`.
2. **It builds a DLL, not a static library** (`<ConfigurationType>
   DynamicLibrary</ConfigurationType>`, with a `tre.def` module-definition
   file). The approved design (D-002 in the decisions ledger) requires
   TRE statically linked into the sidecar so the shipped binary has no
   runtime DLL dependency beyond `KERNEL32.dll`.
3. **It references a file that does not exist at this commit.** Its
   `<ClInclude Include="..\lib\regex.h" />` entry points at
   `lib/regex.h`, but the pinned commit has no such file - the
   POSIX-compatibility header actually lives at
   `local_includes/regex.h`. Building via this project file as checked
   in fails to resolve that include.

Retargeting the `.vcxproj` in place (add an `x64` platform, change
`ConfigurationType` to `StaticLibrary`, fix the stale include) would
itself be a bigger and more fragile deviation than simply not using it -
it would mean maintaining a hand-edited copy of an MSBuild project file
in sync with upstream. Per the pre-approved fallback in the design
(D-002, design-01-engine-build.md §6.2, §9), this build instead compiles
`lib/*.c` **directly** with `cl.exe`, which upstream's own conditional
`#ifdef HAVE_CONFIG_H` / `_MSC_VER` support already makes possible
without touching a single source byte.

## Exact recipe (implemented in `scripts/build-pgn-extract.ps1`)

1. **Compile every `lib/*.c` file** (`regcomp.c`, `regerror.c`,
   `regexec.c`, `tre-ast.c`, `tre-compile.c`, `tre-filter.c`,
   `tre-match-approx.c`, `tre-match-backtrack.c`, `tre-match-parallel.c`,
   `tre-mem.c`, `tre-parse.c`, `tre-stack.c`, `xmalloc.c` - discovered by
   glob, not hand-enumerated, so a future TRE bump picks up added files
   automatically) with:

   ```text
   cl /c /std:c11 /O2 /MT /W3 /DHAVE_CONFIG_H /I <tre>/win32 /I <tre>/lib <tre>/lib/*.c
   ```

   `/DHAVE_CONFIG_H` plus `/I <tre>/win32` makes `lib/tre-internal.h`'s
   `#ifdef HAVE_CONFIG_H #include <config.h> #endif` resolve to TRE's own
   **unmodified, upstream-checked-in** `win32/config.h`. This is the
   real "minimal config header" the design anticipated needing to write
   by hand - TRE already ships one for exactly this purpose, so PGN
   Studio consumes it as-is instead of inventing a duplicate that would
   drift out of sync on every TRE upgrade. No `TRE_EXPORTS`/`_USRDLL` are
   defined, and no source file in `lib/` uses `__declspec`, so the
   resulting objects carry no DLL export/import decoration - they link
   cleanly into a plain static archive.

2. **Archive the objects into `tre.lib`** with MSVC's librarian
   (`lib.exe /OUT:tre.lib *.obj`) - no `.def` file involved (that's only
   needed for a DLL's export table).

3. **Stage TRE's own headers into a synthesized "install" layout** -
   three verbatim, byte-identical copies (not edits) of files TRE already
   ships, arranged the way a real `make install` would place them under
   an install prefix, because no POSIX `make`/autotools is used to build
   TRE on Windows here:

   ```text
   <prefix>/include/regex.h          <- copy of tre/local_includes/regex.h
   <prefix>/include/tre/tre.h        <- copy of tre/local_includes/tre.h
   <prefix>/include/tre/tre-config.h <- copy of tre/win32/tre-config.h
   <prefix>/lib/tre.lib              <- the archive from step 2
   ```

   This reproduces exactly the layout upstream's own
   `Makefile-mingw` assumes (`-I$(TRE_PREFIX)/include/tre`), which is
   also what design-01-engine-build.md §5 committed to
   (`-I<tre>/include/tre -I<tre>/include`). Tracing why each of the three
   copied files needs to be at that exact path (verified against the
   pinned TRE source during Phase 0b's scratch-directory probe):

   - `local_includes/regex.h` is the legacy compatibility shim: with
     `USE_LOCAL_TRE_H` **not** defined (PGN Studio does not define it),
     it does `#include <tre/tre.h>` and `#define regcomp tre_regcomp`
     (and `regexec`/`regerror`/`regfree` likewise) - this is the exact
     "maps plain POSIX names onto `tre_*` via `#define`" mechanism
     design-01-engine-build.md §2.1 described.
   - `local_includes/tre.h`, reached via the `<tre/tre.h>` angle-include
     above, itself does `#include "tre-config.h"` (quoted) - quote-form
     lookup checks the including file's own directory first, so placing
     our copy of `tre-config.h` next to it (`<prefix>/include/tre/`)
     resolves it with no further `-I` needed.
   - `win32/tre-config.h` is, like `win32/config.h`, TRE's own
     upstream-maintained, non-template, MSVC-oriented header (as opposed
     to `local_includes/tre-config.h.in`, which is an autoconf template
     with no generated `.in`-free counterpart checked in for this
     commit). Using it here is the same "consume upstream's own
     Windows-targeted header verbatim" choice as step 1, applied to the
     public API side instead of the internal-library side.

4. **Compile pgn-extract's own `*.c` files** (unmodified, discovered by
   glob against the pinned checkout) with:

   ```text
   cl /c /std:c11 /O2 /MT /W3 /D_CRT_SECURE_NO_WARNINGS /I <prefix>/include /I <prefix>/include/tre <pgn-extract>/*.c
   ```

   `grammar.c`'s and `lists.c`'s `#include <regex.h>` (the only two
   `<regex.h>` includes in pgn-extract, unchanged) resolve via
   `-I <prefix>/include` to the staged `regex.h` from step 3 - reaching
   TRE's real `regcomp`/`regexec`/`regfree`/`regex_t` with **zero
   pgn-extract source edits**, exactly as D-002 requires.

5. **Link** the resulting objects against `tre.lib`, with the UTF-8
   manifest (`engine-src/manifest/pgn-extract.manifest`) embedded via
   `link /MANIFEST:EMBED /MANIFESTINPUT:...` (see that file's own header
   comment for why the manifest is needed).

## Verification performed before this recipe was accepted

Phase 0b proved this recipe end-to-end in a scratch directory before any
of it was written into the project tree, per the coordinator's
de-risking instruction:

- All 13 `lib/*.c` files compiled with only the benign narrowing/sign-
  compare/pointer-truncation warnings already expected from a 32-bit-
  origin C codebase built as 64-bit (C4244/C4267/C4311) - no errors, no
  source edits.
- The resulting `pgn-extract.exe` linked with **zero** pgn-extract
  source changes, ran, printed `pgn-extract v26-06`, exit 0, and
  `dumpbin /dependents` showed **`KERNEL32.dll` only** - identical
  footprint to the earlier stub-regex probe recorded in the decisions
  ledger (D-002).
- **Functional correctness**, not just linking, was verified: a two-game
  fixture with a `White =~ "^F.*r$"` tag-criteria file correctly
  extracted only the game whose White value starts with `F` and ends
  with `r`, proving TRE's `regcomp`/`regexec` are actually being called
  and are returning correct results through the whole chain
  (`taglines.c` → `lists.c` → staged `regex.h` → `tre_regcomp`/
  `tre_regexec`).

## If a future TRE upgrade breaks this recipe

1. Re-check whether the new commit's `win32/tre.vcxproj` has been fixed
   to build an x64 static lib with a valid `lib/regex.h` reference - if
   so, prefer switching to it and deleting this fallback (update this
   README to record that decision).
2. If the direct-compile fallback is still needed, re-verify the exact
   file list in `lib/*.c` (compiled by glob, so new/removed files are
   picked up automatically - but a *renamed* internal header the `.c`
   files depend on could still break the build) and re-run the
   scratch-directory verification steps above before updating
   `engine-src/upstream.lock`'s `regex.windows.commit`.
3. This is registered in `engine-src/upstream.lock` under
   `regex.windows.buildMethod` / `buildMethodNote` - update both
   together with any recipe change, per `engine-src/patches/README.md`
   policy point 4 ("patches must be re-verified on every upstream pin
   update").
