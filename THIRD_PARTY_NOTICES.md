# Third-Party Notices

PGN Studio is licensed under the GNU General Public License, version 3 or
later (GPL-3.0-or-later) - see `LICENSE`. This file lists third-party
components distributed with PGN Studio and their license obligations, per
architecture.md §17.2.

This file currently covers the two third-party items bundled or pinned in
Phase 0, plus the `eco.json` opening dataset added later (see "eco.json"
below). Rust crate and npm package license notices (compile-time/runtime
dependencies) are **not** enumerated here yet - see "Runtime dependency
notices" below for why, and `scripts/README.md` for the planned
`generate-notices.*` automation that will populate them before a release.

---

## pgn-extract

- **What it is:** A command-line PGN search/manipulation/formatting tool.
  PGN Studio bundles it as a sidecar process and never modifies or
  redistributes a reimplementation of it (architecture.md §7.3, ADR-004).
- **Author:** David J. Barnes (d.j.barnes@kent.ac.uk)
- **Upstream repository:** <https://github.com/kentdjb/pgn-extract>
- **Upstream site:** <https://www.cs.kent.ac.uk/people/staff/djb/pgn-extract/>
- **Pinned commit:** `e69e863b70f2fb8ed7916752db95e1c771daf4f0` (branch `main`)
- **Pinned version string:** `v26-06` (from `CURRENT_VERSION` in `argsfile.c`)
- **Commit date:** 2026-07-18
- **License:** GNU GPL v3 or later (GPL-3.0-or-later)
- **License evidence:** Source file headers (e.g. `main.c`) and the
  repository's `copyright` file both read: "either version 3 of the
  License, or (at your option) any later version."
- **Source archive used for verification:**
  `https://github.com/kentdjb/pgn-extract/archive/e69e863b70f2fb8ed7916752db95e1c771daf4f0.tar.gz`,
  SHA-256 `8acaa7167ea3f7dc9e87210368f51efacf85f7ad8f8a2c94719a44490999c1cb`
  (see `engine-src/upstream.lock` and `engine-src/README.md` for how this
  was produced and how to reproduce it).
- **Local patches:** none.
- **What is bundled:** the unmodified GPLv3 license text
  (`src-tauri/resources/pgn-extract/COPYING`), the unmodified `eco.pgn`
  data file (below), and, on Windows, a compiled `pgn-extract.exe`
  sidecar built by `scripts/build-pgn-extract.ps1` from these exact
  pinned sources with **zero source modifications** (macOS builds via
  `scripts/build-pgn-extract.sh` are written but unverified - no Mac is
  available to this project; see `engine-src/upstream.lock`). Build
  flags, target triple, compiler identity, and the binary's own SHA-256
  are recorded per-release in `src-tauri/binaries/build-info-<triple>.json`
  and `checksums.json` (generated at build time, not committed - see
  `src-tauri/binaries/README.md`). The corresponding source for the
  exact binary is the pinned commit above, reproducible via
  `scripts/build-pgn-extract.ps1`; release archives bundle this source
  per architecture.md §17.2/§21.5.
- **Build instructions:** compile the pinned commit's sources with the
  compiler/target recorded at build time; see `engine-src/README.md` for
  the pin-verification procedure that must precede any build, and
  `scripts/build-pgn-extract.ps1`/`.sh` for the build itself.

pgn-extract is not modified by PGN Studio. Its full, unmodified license
text is included in every distribution at
`src-tauri/resources/pgn-extract/COPYING` and also applies via the
project's own `LICENSE` file at the repository root.

---

## TRE (regular expression library, Windows builds only)

- **What it is:** A POSIX-compatible, BSD-licensed regular expression
  library. PGN Studio's Windows sidecar statically links TRE to provide
  the `<regex.h>` implementation pgn-extract's own source expects
  (`grammar.c`, `lists.c`) - this is not a PGN Studio addition or
  deviation: upstream's own Windows build files
  (`Makefile-windows`, `Makefile-mingw`) link TRE for exactly the same
  reason, so TRE **is** upstream's own Windows regex dependency, only
  built with MSVC here instead of MinGW. macOS builds use the system
  libc's `<regex.h>` instead and do not link TRE at all (see
  `engine-src/upstream.lock`, `regex.macos`).
- **Author:** Ville Laurikari (vl@iki.fi)
- **Upstream repository:** <https://github.com/laurikari/tre>
- **Pinned tag/commit:** `v0.9.0` / `d0e0c997336b3210f05b3e1daa7bb5cb9900d274`
- **License:** 2-clause BSD (verbatim text below, from TRE's own `LICENSE` file)
- **Linkage:** statically linked into the Windows engine binary only
  (`pgn-extract-x86_64-pc-windows-msvc.exe`). Not used, not linked, and
  not distributed on macOS.
- **Modifications:** none. `engine-src/patches/tre-msvc/README.md`
  documents a *build-recipe* deviation (TRE's own `win32/tre.vcxproj` is
  unusable as shipped at this commit - x86-only, builds a DLL not a
  static lib, and references a file that does not exist at this commit -
  so `lib/*.c` is compiled directly instead), but this changes no source
  bytes in either TRE or pgn-extract.
- **Corresponding source:** the pinned commit above, reproducible via
  `scripts/build-pgn-extract.ps1`; release archives bundle
  `tre-src-<commit>.tar.gz` alongside the pgn-extract corresponding
  source per architecture.md §21.5.

License text, reproduced verbatim from TRE's `LICENSE` file at the pinned
commit:

> This is the license, copyright notice, and disclaimer for TRE, a regex
> matching package (library and tools) with support for approximate
> matching.
>
> Copyright (c) 2001-2009 Ville Laurikari \<vl@iki.fi\>
> All rights reserved.
>
> Redistribution and use in source and binary forms, with or without
> modification, are permitted provided that the following conditions
> are met:
>
>   1. Redistributions of source code must retain the above copyright
>      notice, this list of conditions and the following disclaimer.
>
>   2. Redistributions in binary form must reproduce the above copyright
>      notice, this list of conditions and the following disclaimer in the
>      documentation and/or other materials provided with the distribution.
>
> THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDER AND CONTRIBUTORS
> ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
> LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
> A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
> HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
> SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
> LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
> DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
> THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
> (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
> OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

---

## eco.pgn (ECO opening classification data)

- **What it is:** A PGN-formatted database of ECO (Encyclopaedia of Chess
  Openings) codes and opening names, distributed together with
  `pgn-extract` and used by its `-e`/`--ecofile` classification option
  (architecture.md §12.2 "Lucena-Ready PGN", §10.6).
- **Bundled at:** `src-tauri/resources/pgn-extract/eco.pgn`
- **Obtained from:** the same pinned `pgn-extract` commit as above
  (`e69e863b70f2fb8ed7916752db95e1c771daf4f0`), unmodified.
- **Provenance, quoted verbatim from the file's own header comment:**

  > "A PGN file of ECO classifications distributed with the PGN extraction
  > program, pgn-extract. I believe that the original file from which I
  > generated this was put together by Ewart Shaw, Franz Hemmer and
  > others, to whom appropriate thanks and acknowledgement is due.
  > Permission has been granted for its inclusion with the pgn-extract
  > program [...] David J. Barnes"

- **License status: not separately stated.** Neither `eco.pgn` itself nor
  the upstream repository declares an independent open-source license for
  this file. The header above is a statement that its compilers (Ewart
  Shaw, Franz Hemmer, and unnamed others) gave David J. Barnes permission
  to redistribute it *together with pgn-extract*; it is not a copyright
  license grant (e.g. not CC0, not MIT, not GPL) in its own right. PGN
  Studio redistributes `eco.pgn` on the same basis pgn-extract itself
  does - as a component of, and under the same redistribution permission
  as, the pgn-extract distribution - and makes no independent claim about
  what license terms would apply to `eco.pgn` if extracted and used
  outside that context. Chess opening names and ECO codes are also
  generally understood to be functional/factual classifications rather
  than creative expression, which independently limits what copyright
  could attach to the raw classification data (as opposed to Barnes'
  specific compiled file). This project takes no position beyond what is
  stated above; if your use case requires a clearer license for `eco.pgn`
  specifically, contact the upstream author.
- **Not modified** by PGN Studio.

See `src-tauri/resources/pgn-extract/SOURCE.json` for the machine-readable
version of both entries above, including file checksums.

---

## eco.json (supplementary opening classification data)

- **What it is:** A JSON dataset of ECO codes, opening names, and the move
  sequences that reach them, aggregated from several public sources. PGN
  Studio uses it to generate a *supplement* to the bundled `eco.pgn`,
  covering opening lines `eco.pgn` does not classify at all.
- **Upstream repository:** <https://github.com/hayatbiralem/eco.json>
- **License:** MIT (declared by the repository; notice reproduced below).
- **Vendored at:** `engine-src/eco-json/ecoA.json` ... `ecoE.json` (five
  volume files, SHA-256 recorded in
  `src-tauri/resources/eco-supplement/SOURCE.json`). These are build
  inputs and are **not** shipped in the application bundle.
- **Upstream commit: not yet pinned.** The files were supplied as a local
  download rather than a verified checkout, so - unlike pgn-extract above -
  no upstream commit/tag is recorded yet. The per-file SHA-256 values are
  recorded so the exact bytes used are identifiable, but the
  pin-and-verify step this project applies to the engine has not been
  completed for this dataset. See `SOURCE.json`'s `vendored.commitNote`.
- **Attribution (from the upstream README):** credits @niklasf for the
  original eco project, Shane Hudson for the SCID data, and Ömür Yanıkoğlu
  for the original eco.json compilation; @JeffML is noted as the primary
  contributor, with a maintained fork at
  <https://github.com/JeffML/eco.json>. The dataset itself aggregates the
  Lichess chess-openings database, the SCID project, Wikipedia's "List of
  Chess Openings" and Wikibooks' "Chess Opening Theory", ChessTempo, the
  chess-graph project, and additional PGN databases and the icsbot
  project. Each record names its own origin in a `src` field.

### What PGN Studio derives from it, and what it does not touch

`scripts/build-eco-supplement.mjs` generates
`src-tauri/resources/eco-supplement/eco-supplement.pgn` (10,642 entries)
from the vendored dataset. That generated file **is** shipped, and is a
derivative work of the MIT-licensed dataset, distributed under the same
MIT terms.

The generator opens the bundled `eco.pgn` **read-only**, solely to
determine which opening lines are already classified so it can exclude
them. `eco.pgn` is not modified, not rewritten, and not merged into on
disk; its recorded SHA-256 in `resources/pgn-extract/SOURCE.json` remains
authoritative and `"modified": false` remains accurate. At runtime the two
files are concatenated into a **cache** directory, bundled content first,
and only that cached copy is passed to the engine.

Because the bundled content is emitted first and `pgn-extract` resolves a
duplicated line to its first occurrence, no classification `eco.pgn`
already provides can be overridden by this supplement. That guarantee is
verified end-to-end against the real engine, not merely argued: see
`src-tauri/tests/eco_supplement_integration.rs`.

### MIT license notice

> MIT License
>
> Copyright (c) 2017 Ömür Yanıkoğlu
>
> Permission is hereby granted, free of charge, to any person obtaining a
> copy of this software and associated documentation files (the
> "Software"), to deal in the Software without restriction, including
> without limitation the rights to use, copy, modify, merge, publish,
> distribute, sublicense, and/or sell copies of the Software, and to
> permit persons to whom the Software is furnished to do so, subject to
> the following conditions:
>
> The above copyright notice and this permission notice shall be included
> in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
> OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
> MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
> IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
> CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
> TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
> SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

---

## Test fixtures

`fixtures/**` contains only synthetic PGN files authored by PGN Studio
contributors for this project (architecture.md §17.3). No real game
database, and no third-party PGN collection, is bundled or was used as a
source. See `fixtures/README.md`.

---

## Runtime dependency notices (Rust crates / npm packages)

Not yet generated. PGN Studio's Rust and TypeScript dependencies (Tauri,
React, etc.) each carry their own (mostly permissive: MIT/Apache-2.0)
licenses, which must be collected and included in release artifacts per
architecture.md §17.2 and §21.5. `scripts/generate-notices.*`
(not yet implemented - see `scripts/README.md`) is intended to scan
`Cargo.lock` and `package-lock.json` and populate
`src-tauri/resources/licenses/` automatically before a release, rather
than this file enumerating a dependency tree by hand that would go stale
immediately. Do not rely on this file alone for release compliance until
that automation exists and has been run.
