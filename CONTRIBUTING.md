# Contributing to PGN Studio

Thanks for your interest in PGN Studio, a free and open-source desktop
application for inspecting, validating, consolidating, filtering, cleaning,
deduplicating, and exporting chess games in PGN.

PGN Studio's full product and technical direction lives in
[`PGN-Studio-architecture.md`](./PGN-Studio-architecture.md). Please read
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

Prerequisites: Node.js (current LTS or newer), Rust (stable, via
`rustup`), and the platform prerequisites listed at
<https://v2.tauri.app/start/prerequisites/> (on Windows: the MSVC Build
Tools "Desktop development with C++" workload).

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

**Build the engine sidecar before any Rust command.** A fresh clone (or a
new `git worktree`) has an empty `src-tauri/binaries/` - everything there
except its README is gitignored build output, by design. Until you build
it once, *every* Rust command that compiles the crate fails, starting
with:

```text
resource path `binaries\pgn-extract-x86_64-pc-windows-msvc.exe` doesn't exist
```

```powershell
pwsh ./scripts/build-pgn-extract.ps1   # ~1 min; fetches pinned sources, compiles, installs
```

That one command is enough for `cargo check`/`test`/`clippy`. It is not
optional tidiness: `tauri.conf.json`'s `bundle.externalBin` makes
`build.rs` fail without the binary, `src-tauri/src/engine/capability.rs`
embeds the generated `build-info-<triple>.json` via `include_str!` at
compile time, and several `engine::sidecar` tests checksum-verify and
actually execute the real pinned binary. Copying a sidecar in by hand is
not a substitute - the build script writes the matching `build-info` and
`checksums.json` alongside it. See `src-tauri/binaries/README.md`.

```sh
cd src-tauri
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check      # run `cargo fmt` (no --check) to fix formatting
```

All five checks above (`npm run build`, `npm test`, `npm run lint`,
`cargo test`, `cargo clippy`) are expected to pass before you open a pull
request; CI runs the same commands (see `.github/workflows/`).

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
