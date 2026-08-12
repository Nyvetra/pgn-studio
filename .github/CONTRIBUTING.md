# Contributing to PGN Studio

Thanks for your interest in PGN Studio, a free and open-source desktop
application for inspecting, validating, consolidating, filtering, cleaning,
deduplicating, and exporting chess games in PGN.

PGN Studio's full product and technical direction lives in
[`architecture.md`](../docs/architecture.md). Please read
the sections relevant to your change before opening a pull request - in
particular §3 (goals/non-goals), §4 (design principles), §5 (release
scope), and §24 (phased implementation plan). The project is being built in
strict phases; please do not implement features from a later phase inside
a change targeting an earlier one.

## Ground rules

- **Original PGN files are immutable.** No code may open a source PGN with
  write access, and no transformation may overwrite a source file
  (architecture.md §4.1, §11.1). This is a hard safety rule, not a style
  preference.
- **Never invoke a shell.** The `pgn-extract` sidecar (and any future
  process) must be started with an explicit argument array, never through
  `sh -c`, `cmd.exe /c`, PowerShell, or a concatenated command string
  (architecture.md §10.3, §16.2).
- **Domain types stay framework-free.** Code under `src-tauri/src/domain/`
  must not depend on `tauri`, and the React presentation layer must not
  embed engine-specific logic (architecture.md §7.1).
- **Never fabricate a metric or claim.** If a value cannot be measured
  (e.g. an unknown game count) show "Not available," never a silent zero
  (architecture.md §3.1, §9.3). If a UI option isn't actually supported by
  the pinned engine, don't expose it (architecture.md §4.3, §10.4).

## Development setup

Prerequisites: Node.js (current LTS or newer; CI pins Node 24, see this
repo's `package.json` `engines` field), Rust (stable, pinned exactly by
`rust-toolchain.toml` at the repo root so every machine and CI compile and
lint against the identical toolchain - see that file's comment for why
this matters for `clippy`), and the platform prerequisites listed at
<https://v2.tauri.app/start/prerequisites/> (on Windows: the MSVC Build
Tools "Desktop development with C++" workload; on macOS: the Xcode
Command Line Tools).

```sh
npm install          # installs frontend deps and pins package-lock.json
npm run dev           # Vite dev server only
npm run tauri dev     # full Tauri app with hot reload
```

```sh
npm run build         # tsc --noEmit type-check, then vite build
npm test               # Vitest (React Testing Library)
npm run lint            # ESLint
```

**Build the engine sidecar before any Rust command - on both Windows and
macOS.** A fresh clone (or a new `git worktree`) has an empty
`src-tauri/binaries/` - everything there except its README is gitignored
build output, by design. Until you build it once, *every* Rust command
that compiles the crate fails, starting with:

```text
resource path `binaries\pgn-extract-x86_64-pc-windows-msvc.exe` doesn't exist
```

That one command is enough for `cargo check`/`test`/`clippy`. It is not
optional tidiness: `tauri.conf.json`'s `bundle.externalBin` makes
`build.rs` fail without the binary, `src-tauri/src/engine/capability.rs`
embeds the generated `build-info-<triple>.json` via `include_str!` at
compile time, and several `engine::sidecar` tests checksum-verify and
actually execute the real pinned binary. Copying a sidecar in by hand is
not a substitute - the build script writes the matching `build-info` and
`checksums.json` alongside it. See `src-tauri/binaries/README.md`.

### Windows

```powershell
pwsh ./scripts/build-pgn-extract.ps1   # ~1 min; fetches pinned sources, compiles, installs
pwsh ./scripts/verify-engine.ps1 -SkipUpstreamSuite -SkipPinProvenance   # fast local sanity check
```

Requires PowerShell 7+, VS 2022 Build Tools with the
`Microsoft.VisualStudio.Component.VC.Tools.x86.x64` component, and (only
if you run `verify-engine.ps1`'s full Layer 2) GNU make - see
`scripts/README.md`.

### macOS

```sh
xcode-select --install                 # Xcode Command Line Tools (Apple clang), if not already installed
brew install --cask powershell         # required - see below
./scripts/build-pgn-extract.sh         # fetches pinned sources, compiles, installs
pwsh ./scripts/verify-engine.ps1 -SkipUpstreamSuite -SkipPinProvenance
```

A few things about macOS that are easy to get wrong the first time:

- **PowerShell 7 (`pwsh`) is required, not optional.**
  `scripts/verify-engine.ps1` is the *only* verification tool this project
  has, and it is deliberately written to run on any of the three CI
  platforms (`$IsWindows`/`$IsMacOS`/`$IsLinux` branches - see the
  script's own header comment). `scripts/build-pgn-extract.sh` is plain
  bash and builds the sidecar fine without `pwsh`, but nothing else here
  can check what it built - without PowerShell 7 a Mac contributor can
  build the sidecar but cannot verify it.
- **Rebuild ordering hazard, same root cause as Windows.**
  `src-tauri/src/engine/capability.rs` embeds the sidecar's
  `build-info-<triple>.json` (a build-time identity hash) into the
  compiled Rust binary via `include_str!`. If you rebuild the sidecar
  (`build-pgn-extract.sh`) without also recompiling the Rust crate
  afterwards, the previously-compiled app still carries the *old* build's
  embedded hash and fails its startup self-test with `ENGINE_TAMPERED` -
  not because anything was actually tampered with, but because the
  embedded expectation is stale. Always rebuild the sidecar first, then
  `cargo build`/`cargo test`/`tauri dev` again afterward.
- **A sidecar hash that differs from another machine's is expected, not
  tampering.** Windows builds with MSVC + `/Brepro`; macOS builds with
  Apple clang and its own reproducibility flags
  (`-D__DATE__="1" -D__TIME__="1" -Wno-builtin-macro-redefined`,
  `-Wl,-no_uuid`). Different toolchains legitimately produce different
  bytes for the same pinned source - see `engine-src/README.md`'s "What
  `/Brepro` does not fix" section, which measures this directly (even
  three different *Windows* MSVC toolset versions already produce three
  different hashes there, before macOS is in the picture at all). Both
  integrity gates (`checksums.json` and the Rust startup self-test)
  compare a binary only against a hash recorded by *that same build*, so
  a cross-machine hash difference fails neither of them.
- **Do not "fix" line endings.** `.gitattributes` already forces LF in the
  index and on checkout (`* text=auto eol=lf`) regardless of either
  machine's local `core.autocrlf` setting, so `core.autocrlf` needs no
  adjustment on Windows or macOS - please leave it alone if you notice it
  unset.
- **A new `.sh` file created or edited from Windows needs its executable
  bit set explicitly before committing.** A missing executable bit on
  `scripts/build-pgn-extract.sh` already broke CI once outright
  ("Permission denied" - see `docs/DECISIONS-LEDGER.md` D-006). Run:

  ```sh
  git update-index --chmod=+x path/to/script.sh
  ```

```sh
cd src-tauri
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check      # run `cargo fmt` (no --check) to fix formatting
```

All five checks above (`npm run build`, `npm test`, `npm run lint`,
`cargo test`, `cargo clippy`) are expected to pass before you open a pull
request, on whichever of the two platforms above you develop on; CI runs
the same commands on both (see `.github/workflows/`).

## Code style

- **SPDX headers.** New source files (`.rs`, `.ts`, `.tsx`, `.css`, shell
  scripts) should start with `// SPDX-License-Identifier: GPL-3.0-or-later`
  (or the language's comment equivalent).
- **TypeScript:** the frontend is typed and linted (`tsc --strict`,
  ESLint's `recommended` + `typescript-eslint` + `react-hooks` +
  `react-refresh` configs - see `eslint.config.js`). Fix lint/type errors
  rather than suppressing them; if a suppression is genuinely correct,
  comment why.
- **Rust:** run `cargo fmt` and keep `cargo clippy --all-targets -- -D
  warnings` clean.
- **IPC boundary:** the frontend must only call Tauri commands through
  `src/ipc/client.ts` (never `@tauri-apps/api/core#invoke` directly from
  feature code), and must only build domain/filter logic through typed
  Rust DTOs, never by composing engine flag strings in React
  (architecture.md §13.4).

## Tests

- Prefer a unit test close to the code you change (Rust: `#[cfg(test)]
  mod tests` or `src-tauri/tests/`; TypeScript: colocated `*.test.tsx`).
- Command-generation logic must have "golden" argument-vector tests, not
  just display-string tests (architecture.md §20.2).
- New engine-integration test cases should use or extend the fixtures in
  `fixtures/` (see `fixtures/README.md`) rather than embedding PGN text
  inline, and must remain small, synthetic, and either
  contributor-authored or clearly public domain/CC0 (architecture.md
  §17.3) - never a real game database.

## Licensing and provenance

PGN Studio is distributed under **GPL-3.0-or-later** (see `LICENSE`). By
submitting a contribution, you agree it is licensed under the same terms
and that you have the right to submit it.

There is no mandatory Developer Certificate of Origin (`Signed-off-by`) or
contributor license agreement at this time (architecture.md §17.4 leaves
this as Nyvetra's call, deferred until it is determined to be necessary).
That may change; if it does, this document will say so clearly before it
becomes a requirement.

## Reporting bugs / requesting features

Please use GitHub Issues. If your report includes PGN file paths, note
that PGN Studio deliberately keeps everything local
(architecture.md §4.5) - please redact anything sensitive from paths,
filenames, or log excerpts before attaching them, since sharing is always
your choice, never automatic (architecture.md §22.2).

See `SECURITY.md` for how to report a security issue instead of a public
bug report.
