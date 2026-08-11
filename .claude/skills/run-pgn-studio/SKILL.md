---
name: run-pgn-studio
description: Build, launch, drive, and screenshot the PGN Studio desktop app on Windows. Use when asked to run or start the app, screenshot it, confirm a change works in the real application, or verify the engine sidecar's startup self-test. Covers the pgn-extract sidecar build, the Tauri bundle, and the UI Automation driver.
---

# Running PGN Studio

Tauri v2 desktop app: React/TypeScript front end in a **WebView2** webview,
Rust back end, plus a pinned `pgn-extract` binary shipped as a sidecar.
Windows only in practice — the macOS legs build in CI but no one has ever
launched one (`DECISIONS-LEDGER.md` D-006).

The app is driven by **`.claude/skills/run-pgn-studio/driver.ps1`**, which
launches it, reads the live UI through UI Automation, types into it,
screenshots the window, and always terminates it.

All paths below are relative to the repo root. Everything here was run on
Windows 11 with PowerShell 7, VS 2022 Build Tools, Node 24, and Rust stable.

## Build (required before anything can run)

**The sidecar comes first.** `src-tauri/binaries/*` is gitignored, so a
fresh clone has none, and the Rust crate then does not compile at all —
`tauri.conf.json`'s `externalBin` fails the build, and
`engine::capability` embeds `build-info-<triple>.json` via `include_str!`.

```powershell
pwsh ./scripts/build-pgn-extract.ps1
```

~1 min. Then verify it (optional but cheap, and the only check of the
engine itself):

```powershell
pwsh ./scripts/verify-engine.ps1
```

Expect `RESULT: PASS` with 76/76 upstream targets and 6/6 goldens. Then
build the app:

```powershell
npm run tauri build
```

~2 min warm. Produces `src-tauri/target/release/pgn-studio.exe` plus MSI
and NSIS installers under `src-tauri/target/release/bundle/`.

## Run (agent path)

```powershell
pwsh ./.claude/skills/run-pgn-studio/driver.ps1 -Action smoke
```

Launches the app, waits for its window, dumps every named UI element,
asserts the Files screen rendered its expected buttons, screenshots to
`src-tauri/target/release/pgn-studio-window.png`, and terminates. Ends
`RESULT: PASS`.

Other actions:

| Action | What it does |
|---|---|
| `smoke` | tree dump + control assertions + screenshot (default) |
| `text` | print the live UI tree only — fastest way to see what rendered |
| `screenshot` | capture the window to PNG only |
| `flow` | type into Base filename, re-read the DOM to prove React responded, screenshot |

```powershell
pwsh ./.claude/skills/run-pgn-studio/driver.ps1 -Action flow
```

Prints `Base filename now reads: 'driver-smoke'` and writes
`src-tauri/target/release/pgn-studio-flow.png`. Use this when you need
evidence the webview is *live*, not merely painted.

**Look at the screenshot.** The driver throws if the capture has fewer
than 3 distinct sampled colours, but a rendered-yet-wrong screen still
needs a human/agent eye.

Useful flags: `-Exe <path>` to drive a different build, `-OutDir <path>`
to redirect screenshots.

## Direct invocation — the engine self-test

Most changes here touch the Rust engine layer, not the window. To verify
the sidecar's two-gate integrity check and startup self-test against the
**real** binary, skip the GUI entirely:

```powershell
cargo test engine::sidecar -- --nocapture
```

Run from `src-tauri/`. 8 tests, including
`run_self_test_passes_against_the_real_sidecar`,
`startup_check_end_to_end_against_the_real_sidecar` (the exact function
`application::startup::initialize` calls), and
`resolve_and_verify_reports_engine_tampered_for_a_modified_copy` — the
negative control proving the check can still fail.

This is stronger evidence than the GUI for engine work, because a failed
self-test does **not** crash the app: the error is stored in `AppContext`
and only surfaces when an engine-dependent command runs.

## Run (human path)

```powershell
npm run tauri dev
```

Opens the window with hot reload. Useless for an agent — it blocks and
never returns.

## Test

```powershell
npm test
```

235 frontend tests (Vitest + React Testing Library), ~11 s.

```powershell
cargo test
```

From `src-tauri/`. 322 Rust tests, several of which execute the real
sidecar.

## Gotchas

- **The WebView2 accessibility tree is built lazily, and the first query
  returns almost nothing.** A single `FindAll` yields 17 descendants with
  2 named — the two host panes — which looks exactly like "UI Automation
  cannot see into WebView2." It can. That first query is also the nudge
  that makes Chromium construct the tree; poll again a second or two later
  and you get 64 descendants, 45 named, the whole DOM. `Get-UiElements`
  in the driver does this. Query once and you will draw the wrong
  conclusion.
- **`PrintWindow` needs `PW_RENDERFULLCONTENT` (flag `2`).** WebView2
  content is composited by DirectComposition and is not in the window's
  own DC, so `BitBlt` or `PrintWindow(h, hdc, 0)` captures a blank
  rectangle.
- **Do not screenshot the screen.** `SetForegroundWindow` is refused for a
  process launched from a background shell, so a full-screen capture
  silently grabs whatever the user actually had in front. Capture the
  window by handle.
- **A clean startup writes no log file.** Logging goes to
  `%LOCALAPPDATA%\com.nyvetra.pgnstudio\logs`, but the startup path only
  emits on notable events (e.g. sweeping interrupted workspaces). Waiting
  for a "self-test passed" line is waiting for something that never
  arrives. Also, `tracing-appender` buffers; killing the process can drop
  whatever was pending.
- **`Next: Operations` is present but disabled until files are added**, and
  adding them goes through a native file dialog the driver does not
  automate. Drive the Destination fields instead — they need no dialog.
- **Use `ValuePattern.SetValue`, not `SendKeys`.** It writes into the input
  and fires the events React listens for without needing the window
  focused or in front.
- **The sidecar's hash is machine-specific.** `/Brepro` makes rebuilds
  byte-identical on a *fixed* MSVC toolset only; a different toolset
  legitimately produces a different binary. Never treat a hash difference
  from CI as tampering — see `engine-src/README.md`, "What `/Brepro` does
  not fix".

## Troubleshooting

| Symptom | Fix |
|---|---|
| `resource path binaries\pgn-extract-...exe doesn't exist` | Sidecar not built. `pwsh ./scripts/build-pgn-extract.ps1`. |
| `couldn't read .../build-info-<triple>.json` | Same cause: the build script writes that file. |
| Driver throws `App binary not found at ...` | `npm run tauri build` first. |
| Driver throws `Screenshot looks blank` | The webview had not painted. Re-run; raise the `Start-Sleep` before `Save-WindowPng` if it persists. |
| `UI tree never grew past N elements` | The webview did not render — check the app actually launched, and that a real window handle appeared. |
| `ENGINE_TAMPERED` at startup | The installed sidecar no longer matches `build-info`. Rebuild it, then rebuild the app so `include_str!` re-embeds the new hash. |
