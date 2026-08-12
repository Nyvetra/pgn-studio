# scripts/

Build/release automation referenced by architecture.md §8 and §21.1.

| Script | Status | Purpose |
|---|---|---|
| `build-pgn-extract.ps1` | **Implemented and verified** (Windows/MSVC) | Fetches pgn-extract + TRE at the commits pinned in `engine-src/upstream.lock` (upstream first, Nyvetra mirror on failure), applies `engine-src/patches/`, compiles TRE as a static lib and pgn-extract against it with zero source edits, embeds the UTF-8 manifest, smoke-checks `--version`, and installs to `src-tauri/binaries/pgn-extract-<triple>.exe` with `checksums.json`/`build-info-<triple>.json`. Run: `pwsh ./scripts/build-pgn-extract.ps1`. |
| `build-pgn-extract.sh` | Written, **runs green on CI** (macOS/Apple clang) | Mirrors the Windows script's contract for `aarch64-apple-darwin`/`x86_64-apple-darwin` (no TRE - system libc regex). No Mac is available in this development environment (decisions ledger D-006), but it has now been executed on GitHub Actions `macos-14` and `macos-15-intel`: Apple clang 15.0.0 builds, smoke-checks, and installs the sidecar on both. Its reproducibility flags remain unverified for *effect* (no two-build comparison has been done on macOS). |
| `lib/engine-common.ps1` | **Implemented** | Shared helpers (`Get-PinnedCheckout`, placeholder/commit validation, PATH refresh, `Get-HostTriple`) dot-sourced by both `build-pgn-extract.ps1` and `verify-engine.ps1`, so pin-checkout logic exists in exactly one place. Not meant to be run directly. |
| `build-eco-supplement.mjs` | **Implemented and verified** | Regenerates `src-tauri/resources/eco-supplement/eco-supplement.pgn` from the MIT-licensed eco.json dataset vendored at `engine-src/eco-json/`, adding only opening lines the bundled `eco.pgn` does not classify at all (bundled content always wins on overlap - see the script's own header comment and `src-tauri/tests/eco_supplement_integration.rs`). Output is deterministic, so regenerating from unchanged inputs is byte-identical. Run: `node scripts/build-eco-supplement.mjs` (add `--check` to verify the committed file is up to date without writing it; exits 1 if stale - this is what CI runs). |
| `verify-engine.ps1` | **Implemented and verified** | Four layers: (0) re-download + checksum the pgn-extract source archive against `upstream.lock` (original Phase 0 check, preserved); (1) SHA-256/size of the installed binary against `checksums.json` plus a `--version` probe; (2) runs pgn-extract's own `test/Makefile` (~76 targets) against the built binary, every oracle diff a failure unless justified in `verify-skips.json`; (3) `fixtures/golden/regex/` - proves the platform regex engine (TRE on Windows) is actually wired in, since upstream's suite has zero `=~` coverage. Run: `pwsh ./scripts/verify-engine.ps1` (add `-SkipPinProvenance` for faster local iteration; CI should run with nothing skipped). |
| `verify-skips.json` | **Implemented** (currently empty) | Checked-in, justified skip list for Layer 2. Empty because the full upstream suite passes 76/76 against the MSVC+TRE build - no test has needed a skip. Add an entry only with a written reason if a future upstream test proves genuinely environment-sensitive; never to hide a real regression. |
| `generate-notices.*` | Not yet implemented (Phase 6) | Will scan `Cargo.lock` and `package-lock.json` for runtime-dependency licenses and populate `src-tauri/resources/licenses/` and the release notices bundle, instead of that content being maintained by hand. |
| `package-release.*` | Not yet implemented (Phase 6) | Will assemble a full release: platform installer(s), checksums, source archive, `pgn-extract`/TRE corresponding source/patches, license texts, third-party notices, and changelog (architecture.md §21.5). |

## Engine build/verify quick start (Windows)

```powershell
pwsh ./scripts/build-pgn-extract.ps1     # fetch, compile, smoke-check, install
pwsh ./scripts/verify-engine.ps1         # identity + upstream suite + supplemental goldens
```

Requires: PowerShell 7+, git, VS 2022 Build Tools with the
`Microsoft.VisualStudio.Component.VC.Tools.x86.x64` component (build
only), and GNU make (verify's Layer 2 only - e.g.
`winget install --id ezwinports.make -e`). Nothing else; no MSYS2/MinGW
is required anywhere in this pipeline.

## Why Phase 0b added two new scripts and a `lib/` directory instead of one

Phase 0's `verify-engine.ps1` only checked the source pin (a network
download + checksum, no binary existed yet to check). Phase 0b needed to
actually build a binary (`build-pgn-extract.ps1`, new), then verify that
binary three more ways that Phase 0's single check couldn't express
(identity/tamper-detection, upstream's own regression suite, and
PGN Studio's own regex-engine-liveness goldens) - so `verify-engine.ps1`
was extended in place rather than left as just the archive check.
`lib/engine-common.ps1` exists because both scripts need identical,
security-relevant checkout-and-verify logic (commit + tree hash
verification with mirror fallback); duplicating that by copy-paste would
have meant two places that could silently drift out of sync.

## Why PowerShell (mostly)

`build-pgn-extract.ps1`/`verify-engine.ps1`/`lib/engine-common.ps1`
target PowerShell 7+ (`pwsh`), preinstalled on every GitHub-hosted runner
(Windows, macOS, and Linux). `verify-engine.ps1` itself is written to run
on any of the three (`$IsWindows`/`$IsMacOS`/`$IsLinux` branch the few
platform-specific bits: the `.exe` suffix, and Windows's need to point
`make` at Git Bash's `sh.exe` since its default `cmd.exe` recipe shell
doesn't have `diff`/`cmp` on `PATH` and mangles quoted `echo` output).
`build-pgn-extract.sh` is bash, not PowerShell, because it invokes
Apple's Xcode toolchain (`clang`, `xcrun`) the way macOS build tooling
conventionally does, and because there is no MSVC-equivalent toolchain
dance to share with the Windows script - the two build scripts have very
little in common beyond the pin-validation and checksum/build-info
JSON shape, which is simple enough not to need cross-language sharing.
