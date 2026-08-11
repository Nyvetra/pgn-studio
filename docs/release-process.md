# Release process

This document describes how a PGN Studio release is meant to be built and
verified, and — as honestly as possible — which parts of that process have
actually been executed and verified versus which parts are written but
unverified because the hardware to verify them does not exist in this
project's development environment. (Some of that gap has since been
closed by CI runners rather than by local hardware — where it has, the
sections below say so specifically, and say what is still open.)
Overstating verification here would
defeat the entire point of the document, so read the "not verifiable from
this machine" section as binding, not as boilerplate.

## Engine provenance and reproducibility

PGN Studio bundles a pinned, unmodified build of `pgn-extract` as a sidecar
process. Nothing about the engine's own source is ever changed.

1. **The pin.** `engine-src/upstream.lock` records the exact upstream
   commit, a mirrored copy under the project's own GitHub organization (in
   case upstream ever force-pushes or disappears), and the expected source
   archive checksum. `scripts/verify-engine.ps1`'s "Layer 0" re-downloads
   that archive and re-hashes it against the lock file before anything else
   runs.
2. **The build.** `scripts/build-pgn-extract.ps1` fetches the pinned
   commit, compiles it (plus a statically-linked TRE regex library on
   Windows only — see `THIRD_PARTY_NOTICES.md`) with zero source patches,
   and installs the resulting binary to
   `src-tauri/binaries/pgn-extract-<target-triple>.exe`, alongside a
   `checksums.json` and a `build-info-<triple>.json` recording the exact
   SHA-256, compiler identity, and build flags used.
3. **Byte-for-byte reproducibility.** The Windows build uses `/Brepro` on
   `cl.exe`/`lib.exe`/`link.exe` plus deterministic source/object ordering.
   This has been independently verified: a clean rebuild (with the cache
   and previous binary wiped) reproduced an identical SHA-256. This is not
   a theoretical claim — it was actually done, twice, with matching hashes
   both times. This matters beyond due diligence: GPL's corresponding-
   source guarantee is only meaningful if a third party can rebuild the
   exact bytes you shipped from the exact source you point them to.
4. **Verification.** `scripts/verify-engine.ps1` runs four layers after a
   build: the pin-provenance check from step 1; a binary identity check
   (SHA-256 + a `--version` probe against the built executable); the
   upstream project's own regression test suite (`test/Makefile`, ~76
   targets — every one passes against this build, with an empty,
   committed skip list); and a supplemental set of regex-engine goldens
   under `fixtures/golden/regex/` (upstream's own suite has zero coverage
   of the `=~` regex operator, so this layer exists specifically to prove
   the statically-linked TRE library is actually wired in and working, not
   merely linked and unused).
5. **Startup self-test.** The two engine-identity checks above happen
   again, every time, inside the shipped app itself — see
   `src-tauri/src/engine/sidecar.rs`. A tampered or corrupted sidecar
   binary is refused, not merely logged.

## Checksum gates

There are two independent points where the sidecar's checksum is checked,
by design, not one:

1. **Package time**: `checksums.json`, written by the build script, is
   what `verify-engine.ps1` and CI check against.
2. **Startup time**: the *installed* app hashes its *own* bundled sidecar
   and compares it against the identity baked into the app binary at
   compile time (`engine::capability::pinned_identity`, sourced from the
   same `build-info-*.json` the build script wrote — never a hand-copied
   literal, so the two can never silently drift apart).

A mismatch at either point is a hard failure: packaging refuses to
continue, and a running app refuses to execute the sidecar at all.

## CI

`.github/workflows/rust.yml` and `frontend.yml` run on every push/PR:
Rust format/clippy/test, frontend lint/test/typecheck/build, a check that
the generated TypeScript IPC bindings have not drifted from their Rust
source of truth, an engine build-and-verify job (steps 1–4 above, run for
real by CI, not merely referenced), and a Tauri bundle build. See those
workflow files for the exact job matrix and which platforms are marked
unverified (next section).

## What is **not** verifiable from this development machine

This project has been developed entirely on Windows, with no Mac
available at any point. The following are honest, structural gaps, not
oversights waiting to be filled in casually:

- **The macOS engine builds; the macOS product does not exist yet.**
  `scripts/build-pgn-extract.sh` mirrors the Windows script's contract
  (fetch the same pin, compile with Apple clang and the system libc regex
  implementation — no TRE on macOS), and it has now actually been run on
  GitHub Actions `macos-14` and `macos-15-intel`. It works: both build,
  smoke-check and install a real sidecar, and `verify-engine.ps1` passes
  Layers 0–2 against it, including 76/76 of pgn-extract's own upstream
  test suite. That first run also found two real defects (a missing
  executable bit and a stdout-only `--version` capture), which is exactly
  why it was worth running rather than assuming.

  What that does **not** mean: macOS reproducibility is still unmeasured
  (the `-D__DATE__`/`-Wl,-no_uuid` flags have never been checked for
  effect), `verify-engine.ps1` Layer 3 still reports 0/6 there for
  line-ending reasons documented in `engine-src/README.md`, the Rust
  crate does not compile on macOS at all yet (`engine::capability`
  hardcodes the Windows build-info), and consequently **no macOS
  application bundle has ever been produced.** Do not ship a macOS
  release on the strength of a working sidecar.
- **macOS code signing and notarization cannot be done here.** Both
  require an Apple Developer ID Application certificate and access to
  Apple's notarization service — neither is available in this environment.
  The macOS CI jobs are real, executable workflow definitions and now
  genuinely execute, but they remain marked `unverified: true` (with
  `continue-on-error`) and labeled as unverified in their own job names.
  Nothing about their presence should be read as "macOS support has been
  tested end to end" — the engine has been; the application has not.
- **Windows Authenticode signing is not configured.** No code-signing
  certificate is available in this environment either. The Windows
  installer PGN Studio's CI produces today is a genuine, working,
  unsigned NSIS/MSI bundle (via Tauri's own bundler) — functionally real,
  but not signed for public distribution. Producing a signed release
  requires a certificate this project does not have; the corresponding CI
  step exists as a clearly-labeled, inert placeholder (see
  `.github/workflows/`) rather than being silently skipped or faked.
- **`scripts/generate-notices.*` and `scripts/package-release.*` are not
  yet implemented.** `scripts/README.md` has tracked this honestly since
  Phase 0. Runtime dependency (Rust crate / npm package) license notices
  are not yet auto-collected — see `THIRD_PARTY_NOTICES.md`'s own
  "Runtime dependency notices" section, which says the same thing rather
  than pretending the gap doesn't exist. A real public release needs this
  automation (or an equivalent manual pass) before it goes out.

## What a real public release still requires, beyond what exists today

In order, roughly:

1. Run `build-pgn-extract.sh` on real Apple Silicon and Intel hardware for
   the first time, and verify it the same four ways the Windows build is
   verified.
2. Obtain an Apple Developer ID Application certificate and a Windows
   code-signing certificate.
3. Implement `generate-notices.*` (or do the equivalent by hand once,
   carefully) so every runtime dependency's license is actually bundled,
   not merely promised.
4. Implement `package-release.*` to assemble the full release: signed
   installers for all three targets, checksums, the source archive
   (including exact `pgn-extract`/TRE corresponding source), license
   texts, third-party notices, and a changelog.
5. Run the resulting signed installers through a clean-machine smoke test
   on real, unmodified Windows and macOS systems — not a development
   machine with build tools already installed.

Until all of the above is true, any release built from this repository
should be treated as a development/testing build, not a public release
candidate, regardless of how complete the application itself is.
