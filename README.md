# PGN Studio

PGN Studio is a free, open-source desktop application for inspecting,
validating, consolidating, filtering, cleaning, deduplicating, and
exporting chess games stored in Portable Game Notation (PGN). It is a
standalone Nyvetra product - it is not part of, and does not depend on,
Lucena or any other chess application.

> PGN Studio makes professional-grade PGN processing understandable, safe,
> and accessible through a transparent desktop interface.

The full product and technical direction lives in
[`PGN-Studio-architecture.md`](./PGN-Studio-architecture.md). Read that
document first - this README only orients you.

## Project status

**Phase 6 of 6 - persistence, accessibility, documentation, and release
quality** (see architecture.md §24). The Version 1 MVP workflow described
below is implemented and tested end to end against the real, pinned engine
sidecar - this is a working application, not a scaffold. What remains
before a genuine public release is entirely in packaging/distribution, not
the app itself: no release artifact on either platform is code-signed yet
(no certificates available), and the macOS build, while it now exists, has
never been launched by a human. No Mac is available in this project's
development environment, but CI now builds the pinned sidecar on Apple
Silicon and Intel, passes 76/76 of `pgn-extract`'s own upstream suite plus
all six supplemental regex goldens against it, compiles the Rust crate and
runs its full test suite on both architectures, and produces an unsigned
macOS application bundle on Apple Silicon. Intel currently packages the
`.app` but fails to build its `.dmg`. See
[`docs/release-process.md`](./docs/release-process.md),
[`docs/acceptance-criteria.md`](./docs/acceptance-criteria.md), and
[`DECISIONS-LEDGER.md`](./DECISIONS-LEDGER.md) D-006 for the precise,
honest breakdown of what is verified versus what is not.

As of this phase: 300+ Rust tests and 230+ frontend tests passing,
`clippy`/`fmt`/`eslint`/`tsc` all clean, and a five-step workflow UI backed
by a real Rust job orchestrator and the real bundled `pgn-extract` sidecar
- no mocked or simulated engine behavior anywhere in the shipped app.

## What PGN Studio does today (Version 1 MVP)

- Add multiple PGN files and reorder them - file order is duplicate-
  retention priority (see
  [`docs/duplicate-semantics.md`](./docs/duplicate-semantics.md)).
- Merge them into one new PGN, or run six other built-in presets (Clean
  Collection, Minimal Mainline PGN, Lucena-Ready PGN, Validate Only, New
  Games Against Master) - see
  [`docs/user-guide.md`](./docs/user-guide.md).
- Detect duplicate move scores, keep the first copy in file order, and
  optionally write the diverted copies to an audit file.
- Strip comments, variations, and NAGs independently of each other.
- Add ECO/opening classification using the bundled `eco.pgn`.
- Filter by player, result, Elo, date range, move-count range, checkmate-
  only, starting position, ECO code, FEN pattern, and opening line - see
  [`docs/engine-capabilities.md`](./docs/engine-capabilities.md) for the
  verified, sometimes-surprising rules behind what each filter can and
  cannot safely express.
- Show the exact operation plan and generated engine argument list before
  running anything - inspectable, never a hidden or shell-executed
  command.
- Run without freezing the interface, and cancel an active job cleanly.
- Receive a manifest, a plain-language result summary, and (where
  supported by the engine) honest metrics - an unmeasurable value is shown
  as "Not available," never as a misleading zero.
- **Never** modify or overwrite source files - every transformation writes
  new artifacts, and the app refuses to run if an output would alias an
  input.
- Run entirely offline, with no accounts, telemetry, or network access -
  verified, not just promised (architecture.md §22.3; see
  `src-tauri/src/observability/`).

See architecture.md §5 for the full release-scope breakdown (MVP through
Version 3 "Game Studio") and [`docs/user-guide.md`](./docs/user-guide.md)
for how to actually use each step.

## How it's built

- **Desktop shell:** [Tauri 2](https://v2.tauri.app/)
- **Frontend:** React + TypeScript + Vite
- **Backend:** Rust (application services, job orchestration, filesystem
  safety)
- **Bulk PGN engine:** [`pgn-extract`](https://github.com/kentdjb/pgn-extract),
  bundled as a bounded, sandboxed sidecar process and invoked only as an
  explicit argument array - never through a shell. PGN Studio does not
  reimplement `pgn-extract`; see architecture.md §7.3 for why.

## Getting started

Prerequisites: a current Node.js LTS, Rust (stable, via `rustup`), and the
platform prerequisites at
<https://v2.tauri.app/start/prerequisites/> (on Windows: MSVC Build Tools'
"Desktop development with C++" workload).

```sh
npm install
npm run tauri dev
```

The bundled `pgn-extract` sidecar is not committed to this repository (see
`engine-src/README.md` and `scripts/README.md`) - build it first with
`pwsh ./scripts/build-pgn-extract.ps1` (Windows; see
[`docs/release-process.md`](./docs/release-process.md) for the equivalent
macOS status), or the app will start but report the engine as unavailable.

### Running the tests

```sh
npm test                    # frontend: Vitest + React Testing Library + axe
npm run lint                # ESLint
npx tsc --noEmit            # TypeScript
cd src-tauri
cargo test                  # Rust: unit + integration, against the real sidecar
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for the full development,
testing, and lint workflow.

## Repository layout

See architecture.md §8 for the authoritative repository structure and the
rationale behind it. Highlights:

- `src/` - React/TypeScript frontend: the five-step workflow UI
  (`src/features/`), shared accessible form components (`src/components/`),
  and the typed Tauri IPC client (`src/ipc/`).
- `src-tauri/` - Rust backend: domain model and pure command compiler
  (`domain/`, `engine/`), job orchestration and process safety (`jobs/`),
  filesystem safety (`filesystem/`), the public error taxonomy
  (`errors/`), settings/history persistence (`persistence/`), structured
  local logging (`observability/`), and the Tauri command/event surface
  (`commands/`, `application/`).
- `engine-src/` - pins the exact upstream `pgn-extract` revision PGN
  Studio builds against (`upstream.lock`) and documents how that pin is
  produced and verified.
- `fixtures/` - small, synthetic PGN files used for engine-integration
  testing (see `fixtures/README.md`). No real game database is bundled.
- `docs/` - user guide, engine capability notes, duplicate-detection
  semantics, the release process, and the project's own honest acceptance-
  criteria self-assessment (`docs/acceptance-criteria.md`).

## License and third-party notices

PGN Studio is licensed under the **GNU General Public License v3.0 or
later** (GPL-3.0-or-later) - see [`LICENSE`](./LICENSE). It bundles files
from the third-party `pgn-extract` project (also GPL-3.0-or-later); see
[`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md) for the full
attribution, including the honest provenance note for the bundled
`eco.pgn` data file.

## Security

See [`SECURITY.md`](./SECURITY.md) for how to report a vulnerability.

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) and
[`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md).
