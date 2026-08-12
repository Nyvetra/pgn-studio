# Security Policy

## Project status

PGN Studio is pre-release (Phase 0 of the implementation plan in
`architecture.md` §24). There are no published binary releases
yet, so there is currently only one supported line: the `main` branch.
This policy will be revised once versioned releases exist.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for a suspected security
vulnerability.

Instead, use GitHub's private vulnerability reporting for this repository
(repository **Security** tab -> **Report a vulnerability**), which creates
a private advisory visible only to you and the maintainers. If that
feature is not enabled on this repository yet, open a regular issue asking
a maintainer to enable it or provide a private contact, without including
vulnerability details in that request.

Please include, where relevant:

- the PGN Studio version/commit and platform (OS, architecture);
- the bundled `pgn-extract` version/commit reported by the app, if
  applicable;
- reproduction steps, and whether reproduction requires a specific PGN
  file (please describe the file's structure rather than attaching real
  private game data if you can avoid it);
- the observed impact (crash, memory exposure, code execution, path
  traversal, etc.).

We aim to acknowledge reports promptly and will work with you on a
disclosure timeline appropriate to severity. Please give us a reasonable
opportunity to address a report before any public disclosure.

## What's in scope

- The PGN Studio Rust application (`src-tauri/`) and frontend (`src/`).
- The Tauri configuration and capability grants (`src-tauri/tauri.conf.json`,
  `src-tauri/capabilities/`) - in particular anything that would widen the
  frontend's access beyond what architecture.md §16.3 intends (narrow
  dialog/app-config/app-cache access and a scoped ability to spawn only
  the named bundled sidecar; never broad shell or filesystem access).
- The bundled `pgn-extract` sidecar's **integration** into PGN Studio:
  how it is invoked (must never go through a shell - architecture.md
  §10.3, §16.2), how its output is trusted/validated, and whether its
  checksum is verified before execution.
- Supply-chain integrity of the pinned upstream `pgn-extract` revision
  (`engine-src/upstream.lock`) and of PGN Studio's own release artifacts
  once they exist.

## What's likely out of scope

- Vulnerabilities purely within `pgn-extract`'s own C source that do not
  depend on how PGN Studio invokes it - please also consider reporting
  those upstream at <https://github.com/kentdjb/pgn-extract>.
  If in doubt, report it to us anyway and we will help route it.
- Issues that require the attacker to already control the local user
  account running PGN Studio (PGN Studio has no network service and no
  elevated-privilege component to cross a trust boundary from).

## Project security posture (for context)

PGN Studio is designed to be offline-first with no telemetry, analytics,
or remote logging in Version 1 (architecture.md §4.5, §16, §22.3), to treat
all source PGN files as immutable, and to invoke the bundled engine only
as an argument array - never through a shell (architecture.md §10.3,
§16.2). See architecture.md §16 ("Security and privacy") for the full
threat model and controls this project is designed against.
