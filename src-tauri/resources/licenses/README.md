# resources/licenses/

Reserved for the license texts of third-party **runtime** dependencies
(Rust crates and npm packages that end up compiled/bundled into a released
PGN Studio binary) that require their license text to be distributed with
the binary.

Empty in Phase 0. The planned `scripts/generate-notices.*` (see
`scripts/README.md`) should scan `Cargo.lock` / `package-lock.json` and
populate this directory automatically before a release, rather than
maintaining it by hand. `pgn-extract`'s own license lives next to it in
`../pgn-extract/COPYING`, not here, because it is a bundled *component*
with its own upstream identity (see `SOURCE.json`), not a Rust/npm build
dependency.
