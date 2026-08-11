# src-tauri/binaries/

This directory is where the platform-specific `pgn-extract` **sidecar
executables** are placed so Tauri can bundle them as external binaries
(architecture.md §10.2, §16.3; `tauri.conf.json`'s
`bundle.externalBin: ["binaries/pgn-extract"]`).

## Nothing is committed here - by design

`scripts/build-pgn-extract.ps1` (Windows, implemented and verified) /
`.sh` (macOS, written but unverified - no Mac available) write into this
directory, but **everything they produce here is gitignored** except this
README (root `.gitignore`): the compiled binary, `checksums.json`,
`build-info-<triple>.json`, `verify-report-<triple>.json`, and the
upstream-suite log. All of it is reproducible build/verify *output* tied
1:1 to a binary that is itself never committed (architecture.md §8: "Do
not commit undocumented third-party binaries"), so committing a stale
copy of any of it would go out of sync the moment the binary is rebuilt
and actively mislead whoever reads it later. The durable, reviewable
input is `engine-src/upstream.lock` plus the two build scripts - anyone
can reproduce everything in this directory from those.

## The Rust crate does not compile until you build one

Because nothing here is committed, a fresh clone or a new `git worktree`
starts with this README and nothing else - and the `src-tauri` crate
cannot be compiled at all in that state. `cargo check`, `cargo test`,
`cargo clippy`, and `cargo run -p xtask -- export-bindings` all fail
immediately with:

```text
resource path `binaries\pgn-extract-x86_64-pc-windows-msvc.exe` doesn't exist
```

Run `pwsh ./scripts/build-pgn-extract.ps1` once and they all work. Three
separate things depend on the contents of this directory at *build* time,
which is why a hand-copied binary is not a substitute for running the
script:

1. `tauri.conf.json`'s `bundle.externalBin: ["binaries/pgn-extract"]` -
   `tauri_build::build()` checks the file exists (the error above);
2. `src-tauri/src/engine/capability.rs` embeds
   `build-info-x86_64-pc-windows-msvc.json` via `include_str!`, so the
   pinned identity can never drift from the actual binary;
3. `src-tauri/src/engine/sidecar.rs`'s tests resolve, SHA-256-verify, and
   *execute* the real binary (`run_self_test`, `probe_unicode_paths`,
   `startup_check`).

`.github/workflows/rust.yml` builds the sidecar for exactly this reason
before its Clippy/test/bindings steps.

## Naming convention

Tauri requires each sidecar to be suffixed with the Rust target triple it
was built for (`rustc --print host-tuple`; `scripts/build-pgn-extract.ps1`
does this automatically, defaulting to `x86_64-pc-windows-msvc` with a
warning if `rustc` is not on `PATH`):

```text
src-tauri/binaries/pgn-extract-x86_64-pc-windows-msvc.exe   (built and verified)
src-tauri/binaries/pgn-extract-aarch64-apple-darwin         (unverified - no Mac available)
src-tauri/binaries/pgn-extract-x86_64-apple-darwin          (unverified - no Mac available)
```

At bundle time Tauri installs the sidecar next to the app executable with
the target-triple suffix stripped - the exact installed name/path
resolution is Phase 1 ("Engine adapter proof") scope for the Rust code in
`src-tauri/src/engine/` that spawns it.

## Building and verifying

```powershell
pwsh ./scripts/build-pgn-extract.ps1    # fetch pinned sources, compile, smoke-check, install here
pwsh ./scripts/verify-engine.ps1        # identity + upstream regression suite + supplemental regex goldens
```

Every binary placed here must come from `scripts/build-pgn-extract.*`
against the pinned commit in `engine-src/upstream.lock` - never placed
here ad hoc. `checksums.json` (written by the build script, re-checked by
the verify script's Layer 1) is the single source of truth for what the
binary's SHA-256 and size are supposed to be; `build-info-<triple>.json`
records the compiler identity, flags, and build timestamp for the same
binary (architecture.md §10.1 "compiler and target information for
release binaries").
