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

**Phase 0 - repository and compliance scaffold** (see architecture.md
§24). There is no working end-user application yet: this stage sets up the
Tauri 2 + React + TypeScript project structure, licensing/compliance
files, the pinned upstream `pgn-extract` revision, test fixtures, and CI
skeleton that later phases build on. The `get_app_info` command in
`src-tauri/src/lib.rs` exists only to prove the frontend-to-Rust IPC
boundary works end to end - it is not a feature.

## What PGN Studio will do (Version 1 MVP)

- Merge multiple PGN files into one new collection, in a user-chosen
  order.
- Validate game scores and separate broken games.
- Detect duplicate move scores, write a unique-games output, and preserve
  a duplicate-games audit file.
- Optionally strip comments, variations, and NAGs.
- Optionally add ECO/opening classification using the bundled `eco.pgn`.
- Show the exact operation plan before running anything.
- **Never** modify or overwrite source files - every transformation writes
  new artifacts, and the app refuses to run if an output would alias an
  input.
- Run entirely offline, with no accounts, telemetry, or network access.

See architecture.md §5 for the full release-scope breakdown (MVP through
Version 3 "Game Studio").

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

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for the full development,
testing, and lint workflow.

## Repository layout

See architecture.md §8 for the authoritative repository structure and the
rationale behind it. Highlights:

- `src/` - React/TypeScript frontend.
- `src-tauri/` - Rust backend and Tauri configuration. `src-tauri/src/`'s
  subdirectories are currently near-empty placeholders (each has its own
  `README.md`) reserved for Phase 1+ domain/engine/job logic.
- `engine-src/` - pins the exact upstream `pgn-extract` revision PGN
  Studio builds against (`upstream.lock`) and documents how that pin is
  produced and verified.
- `fixtures/` - small, synthetic PGN files used for engine-integration
  testing (see `fixtures/README.md`). No real game database is bundled.
- `docs/` - reserved for user/engine/release documentation as it is
  written (see `docs/README.md`).

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
