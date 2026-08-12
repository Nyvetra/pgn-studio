# First run on a Mac

A walkthrough for setting up PGN Studio on a Mac for the first time, when
the Windows machine is not in front of you.

`.github/CONTRIBUTING.md` remains the authoritative contributor reference
for the day-to-day workflow and for what every command does. This file is
the narrower thing: the exact order to run things on a fresh Mac, the
traps that cost time on the first attempt, and the one task only a Mac can
do.

> **Status of this document.** Nothing here has been executed on real Mac
> hardware. It is assembled from what CI demonstrably does on GitHub's
> `macos-14` and `macos-15-intel` runners, and from what the repo's own
> scripts and docs establish. Per the project's standing rule
> (`docs/DECISIONS-LEDGER.md` D-006), unverified platform work is written
> in good faith and labelled as unverified rather than presented as tested.
> **Treat your first run as the verification.** If a command here is wrong,
> that is a finding worth recording, not a mistake to work around silently.

## What CI already proves about macOS

Useful to know before you start, because it tells you which failures are
surprising and which are expected:

- The pinned `pgn-extract` sidecar **builds** on both Apple Silicon and
  Intel (Apple clang 15.0.0 on the runners).
- `scripts/verify-engine.ps1` passes **all four layers** there, including 76/76 of
  pgn-extract's own upstream test suite and 6/6 supplemental regex goldens.
- The Rust crate **compiles and passes its full test suite** on both
  architectures.
- CI **produces unsigned `.app` bundles** on both.
- Nobody has ever **launched** one. That is the gap you can close.

## Prerequisites

```bash
xcode-select --install
```

Apple clang, which `scripts/build-pgn-extract.sh` uses to compile the
engine. Skip if you already have Xcode.

```bash
brew install --cask powershell
```

**Not optional.** `scripts/verify-engine.ps1` is the only verification
tool in the project and is deliberately cross-platform (it branches on
`$IsWindows`/`$IsMacOS` internally). Without `pwsh` you can build a
sidecar but have no way to check it.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The official rustup installer. Do **not** pick a Rust version: the repo's
`rust-toolchain.toml` pins `1.97.1`, and rustup installs exactly that on
the first `cargo` command inside the repo.

Node 24 or newer, however you prefer to install it (`brew install node`,
`fnm`, `nvm`). `package.json` declares `"engines": { "node": ">=24" }`,
matching what CI pins. Check with `node --version`.

## Setup, in this order

```bash
git clone https://github.com/Nyvetra/pgn-studio.git
cd pgn-studio
npm ci
```

```bash
./scripts/build-pgn-extract.sh
```

**This is the step that is easy to skip and impossible to skip.** Until a
sidecar exists, the Rust crate does not compile *at all* —
`src-tauri/tauri.conf.json`'s `bundle.externalBin` fails
`src-tauri/build.rs`, and `src-tauri/src/engine/capability.rs`
embeds `build-info-<triple>.json` via `include_str!` at compile time. The
resulting error names a missing resource path and reads like a broken
checkout rather than a missing build step. Everything in
`src-tauri/binaries/` except its README is gitignored build output, by
design.

If it fails with `Permission denied`, the executable bit did not survive —
`chmod +x scripts/build-pgn-extract.sh`. (The bit is committed as mode
`100755`; a missing one broke CI once.)

```bash
pwsh ./scripts/verify-engine.ps1
```

Expect `RESULT: PASS` with four layers, 76/76 upstream targets, and 6/6
goldens. Add `-SkipUpstreamSuite -SkipPinProvenance` for a faster run that
skips the network download and the make-based suite.

Some goldens may report `[PASS~]` rather than `[PASS]`. That is correct on
macOS and not a problem — see "Line endings" below.

## Confirm the toolchain end to end

```bash
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

334 Rust tests, including several that execute the real sidecar. Then:

```bash
cd ..
npm test
npm run lint
npx tsc --noEmit
node scripts/build-eco-supplement.mjs --check
```

235 frontend tests, and the ECO supplement check should report
`up to date (10642 entries)` — that generator produces byte-identical
output on macOS and Windows, which CI confirms.

```bash
npm run tauri build
```

Produces `src-tauri/target/release/bundle/macos/PGN Studio.app` and a
`.dmg`.

## Four things that will surprise you

**Your sidecar's hash will differ from the Windows machine's.** That is
correct, not tampering. Different toolsets legitimately produce different
binaries; `/Brepro` removes *time* as a build input, not *toolchain
version*. Both integrity gates compare against a hash recorded by **the
same build that produced the binary**, so each machine is internally
consistent. Never compare hashes across machines. Background:
`engine-src/README.md`, "What `/Brepro` does not fix".

**Rebuild ordering matters.** If you rebuild the sidecar, rebuild the Rust
crate too. `engine::capability` embeds the hash at *compile* time, so a
fresh sidecar against a stale crate gives `ENGINE_TAMPERED` at startup —
which looks alarming and is merely stale.

**Do not touch `core.autocrlf`.** `.gitattributes` already forces LF in the
index (`* text=auto eol=lf`), with deliberate exemptions under
`fixtures/**`. It is correct as-is on both machines, and "fixing" it would
break the CRLF-pinned goldens that let Layer 3 catch a genuine line-ending
regression on Windows.

**Line endings in Layer 3.** The regex goldens are stored CRLF; the macOS
engine writes LF. Layer 3 compares byte-exact first and only then retries
with newlines normalized, reporting such a case as `[PASS~]` with a note
and counting it separately. `[PASS~]` on macOS is expected; `[PASS]` on
Windows is expected. A silent pass would have been the bug.

## The one task only you can do

```bash
open "src-tauri/target/release/bundle/macos/PGN Studio.app"
```

CI builds this bundle on both architectures and has never run it. "Builds
and packages" is not "runs correctly", and the ledger has been careful
never to claim otherwise.

What to look for:

- The window opens and the **Files** step renders, with later steps
  disabled until files are added.
- No engine error. A failed startup self-test does **not** crash the app —
  the error is stored in `AppContext` and surfaces when an
  engine-dependent command runs, so an error banner or a failure when
  adding files is the signal.
- Then actually use it: add a PGN, set a destination, run a merge.

For direct evidence independent of the UI, the same self-test runs here:

```bash
cd src-tauri && cargo test engine::sidecar -- --nocapture
```

Eight tests, including `startup_check_end_to_end_against_the_real_sidecar`
(the exact function startup calls) and a tampered-copy negative control
proving the check can still fail.

## If something goes wrong

The useful response is to record it, not to work around it. Two specific
asks:

- **If a command in this file is wrong**, fix it here. This document was
  written without a Mac and expects to be corrected.
- **If macOS behaves differently from what the ledger claims**, amend
  `docs/DECISIONS-LEDGER.md` D-006. It currently records that the engine
  builds and passes its suites on macOS, that bundles are produced, and
  that **nobody has launched one**. Your first launch changes that entry
  either way — success and failure are both worth recording, and the entry
  is written to be amended rather than replaced.

Known-open macOS items, so you can tell a new problem from a known one:

- macOS build **reproducibility is unmeasured** — no two-build comparison
  has ever been run there, so the `-D__DATE__`/`-Wl,-no_uuid` flags are
  applied but unproven for effect.
- `filesystem::identity`'s case-folded path comparison is **byte-exact off
  Windows** by deliberate decision, so it does not currently treat
  `Out.pgn` and `out.pgn` as the same path on APFS. The gap is covered by
  a test that asserts the documented behaviour rather than hiding it.
- The Intel `.dmg` step fails intermittently in CI — a known `hdiutil`
  runner-image problem, not a project fault. The bundle step retries three
  times and passes `--verbose`.
- Nothing on either platform is code-signed or notarized; no credentials
  exist.

## Where to read further

| | |
| --- | --- |
| `.github/CONTRIBUTING.md` | authoritative contributor workflow, both OSes |
| `docs/architecture.md` | the project's technical constitution (§8 is the repo layout) |
| `docs/DECISIONS-LEDGER.md` | why things are the way they are; D-006 is the macOS entry |
| `engine-src/README.md` | engine pinning, reproducibility evidence, the macOS toolchain notes |
| `src-tauri/binaries/README.md` | why the sidecar is not committed and what depends on it |
| `scripts/README.md` | what each build/verify script does and its verification status |
