# MVP acceptance criteria — self-assessment

This is the project's own item-by-item assessment against
`PGN-Studio-architecture.md` §25, written at the end of Phase 6 (the last
planned MVP phase). Every item below is marked **Verified**, **Partially
verified**, **Not verified**, or **Not achievable here**, each with the
evidence behind that call. The rule followed while writing this: overstating
readiness is worse than an honest gap, so where evidence is incomplete or
absent, this document says so plainly rather than rounding up.

"Verified" means there is a specific, checkable artifact behind the claim —
a test that exercises the real engine, a specific file/line, or a command
whose output is quoted. "Not achievable here" is reserved for the two
physical constraints repeated throughout this project: no Mac is available
to this development environment, and no code-signing/notarization
credentials are available (decisions ledger D-006).

At the time of writing: **322 Rust tests** (`cd src-tauri && cargo test`)
and **235 frontend tests** (`npm test`) pass; `cargo clippy --all-targets --
-D warnings`, `cargo fmt --check`, `npx tsc --noEmit`, `npm run lint`, and
`npm run build` are all clean; generated TypeScript bindings have no drift
from their Rust source of truth.

---

## Functional

- [x] **Users can select multiple PGNs and reorder them.** — Verified.
  `src/features/inputs/FilesScreen.tsx` (Add Files/Add Folder/drag-and-drop),
  `src/features/inputs/SourceList.tsx` (Move Up/Down, disabled at the
  boundaries). Tested in `FilesScreen.test.tsx`/`SourceList.test.tsx` and
  confirmed live via the accessibility tree during this phase's browser
  verification.

- [x] **Users can merge files into one new PGN.** — Verified against the
  real engine. `src-tauri/tests/job_orchestration_integration.rs::real_merge_preserves_sources_and_publishes_expected_output`
  runs a real merge through the bundled sidecar and asserts the published
  output's content; `phase4_integration.rs::preset_merge_safely_retains_everything_and_does_not_dedupe`
  covers the "Merge Safely" preset specifically.

- [x] **Users can create unique and duplicate audit outputs.** — Verified
  against the real engine. `src-tauri/tests/duplicate_integration.rs` (14
  tests), e.g. `report_and_keep_first_diverts_duplicates_and_retains_first_copy`,
  `input_order_forward_keeps_alpha_and_diverts_bravo`. Full semantics
  documented in `docs/duplicate-semantics.md`.

- [x] **Users can remove comments, variations, and NAGs independently.** —
  Verified against the real engine, tested both individually and combined:
  `phase4_integration.rs::cleanup_remove_comments_alone_...`,
  `cleanup_remove_variations_alone_...`, `cleanup_remove_nags_alone_...`,
  `cleanup_all_three_combined_...`.

- [x] **Users can run supported validation and ECO operations.** —
  Verified against the real engine. `phase4_integration.rs::validate_only_reports_errors_and_publishes_no_transformed_collection`,
  `eco_classification_attached_form_adds_correct_eco_opening_and_variation_tags`,
  `preset_validate_only_matches_its_documented_effect_and_publishes_no_games_file`.

- [x] **Users can configure the agreed MVP filters.** — Verified against
  the real engine. `phase5_filters_integration.rs` (36 tests) covers every
  filter exposed on the Filters screen (player/name, result, Elo, date
  range, move-count range, checkmate-only, setup position, ECO, FEN
  pattern, textual opening line), including the specific empirically-
  verified traps documented in `docs/engine-capabilities.md` (e.g.
  `result_equality_operator_would_have_silently_matched_nothing_empirical_proof`,
  `date_bare_year_upper_bound_would_have_silently_excluded_dec_31_empirical_proof`).

- [x] **Users can cancel an active job.** — Verified against the real
  engine. `job_orchestration_integration.rs::cancellation_leaves_sources_and_prior_outputs_untouched`;
  `duplicate_integration.rs::external_duplicate_table_creates_virtual_tmp_and_cancellation_removes_it`;
  `RunScreen.test.tsx`'s Cancel test; the Cancel button is live in the UI.

- [x] **Users receive a manifest and understandable result.** — Verified,
  and materially strengthened this phase. The manifest
  (`src-tauri/src/filesystem/manifest.rs::FinalManifest`) now carries every
  field architecture.md §15.3 lists — schema version, PGN Studio version,
  OS/arch, engine identity, job configuration, ordered input metadata,
  criteria-file hashes, sanitized argv, timestamps, exit code,
  warnings/errors, and artifact paths/sizes (`app_version`/`os`/`arch`/
  `exit_code`/`inputs` were added this phase — previously absent; see
  `filesystem::manifest`'s tests). `src/features/results/ResultsScreen.tsx`
  presents this in plain language, with unmeasurable metrics shown as "Not
  available."

---

## Safety

- [x] **Original input hashes are unchanged in integration tests.** —
  Verified. `job_orchestration_integration.rs::real_merge_preserves_sources_and_publishes_expected_output`
  records SHA-256 of both source files before the run and asserts they are
  byte-identical afterward (lines ~148-149 record `sha_a_before`/
  `sha_b_before`; the same computation is re-asserted equal after the run).

- [x] **The application rejects source/output collisions.** — Verified.
  `job_orchestration_integration.rs::input_output_collision_is_rejected_and_source_is_untouched`;
  `filesystem/validate.rs::input_output_collision_is_detected_when_output_dir_is_input_dir`
  and related unit tests; enforced via `filesystem::identity`'s file-identity
  comparison (not string path comparison), which additionally catches
  hard-link aliasing a naive path check would miss.

- [x] **The application never invokes a shell.** — Verified. Every process
  spawn in this codebase (`jobs/process.rs`, `engine/sidecar.rs`) uses
  `tokio::process::Command` with an explicit argument array, never a shell
  interpreter. `display_command_is_inert_text_even_with_shell_metacharacters`
  specifically proves the human-readable command preview shown on the
  Review screen is inert display text, never executed. Confirmed by
  design: no code path in this crate constructs a shell command string.

- [x] **Partial outputs are not published as successful artifacts.** —
  Verified. `filesystem/publish.rs` writes to a temp name and only
  publishes via a same-directory rename after confirming the sidecar
  exited successfully and the temp file exists and is readable; see
  `missing_temp_output_is_reported_as_output_missing`,
  `matched_games_positive_but_empty_output_is_invalid`,
  `fail_policy_never_overwrites_and_leaves_temp_for_diagnosis`.

- [x] **Existing output conflicts are handled explicitly.** — Verified.
  Three explicit `ConflictPolicy` variants (Fail / AddNumericSuffix /
  ReplaceAfterConfirmation), none of which silently overwrite:
  `add_numeric_suffix_finds_the_first_free_pair`,
  `replace_after_confirmation_backs_up_and_never_silently_overwrites_on_race`,
  and the Review-screen confirmation dialog
  (`src/components/ConfirmDialog.tsx`) for the replace case.

- [x] **Each run has an isolated work directory.** — Verified.
  `filesystem::workspace::create_job_workspace` creates
  `<app-cache>/jobs/<job-uuid>/` per job; `create_job_workspace_makes_the_full_tree`.
  Large sources are referenced in place, never copied into it.

- [x] **Cancellation does not remove source or prior output files.** —
  Verified. `cancellation_leaves_sources_and_prior_outputs_untouched`
  integration test; cancellation cleanup is scoped to the job's own
  `.pgnstudio-tmp-*`-prefixed temporary files and `virtual.tmp` only (see
  `filesystem::workspace`'s swept-prefix allowlist, also exercised by
  `sweep_never_deletes_a_path_outside_the_swept_prefix`).

---

## Quality

- [~] **Golden fixtures pass on Windows x64 and both macOS architectures.**
  — Partially verified: **Windows x64 verified, macOS not achievable here.**
  On Windows: `scripts/verify-engine.ps1` Layer 2 (pgn-extract's own ~76
  upstream test targets) passes 76/76 with an empty, committed skip list;
  Layer 3 (`fixtures/golden/regex/`, 6 supplemental goldens proving TRE is
  actually linked and functioning) passes 6/6. On macOS: **now actually
  run, and passing everything except a fixture artifact.** No Mac is
  available in this development environment (decisions ledger D-006), but
  the CI legs have since executed on `macos-14` and `macos-15-intel`:
  `scripts/build-pgn-extract.sh` builds and installs the sidecar on both,
  and `verify-engine.ps1` passes **all four layers** against it —
  including Layer 2 at 76/76 and Layer 3 at 6/6. Layer 3 initially
  reported 0/6 there, which turned out to be a *fixture* problem rather
  than an engine one: the goldens are compared byte-for-byte and are
  stored CRLF (`.gitattributes` pins `fixtures/** -text`), while the macOS
  engine emits LF. Stripping `\r` from each committed golden reproduced
  the macOS run's actual SHA-256 exactly, in all six cases — so macOS
  output is content-identical to Windows for literal, anchors, bracket,
  star, backreference, and the `grammar.c` odds call site. Layer 3 now
  compares byte-exact first and falls back to a newline-normalized
  comparison only on failure, reporting any such case as `[PASS~]` rather
  than silently; Windows still reports all six as byte-exact. See
  `docs/release-process.md`.

- [~] **Unicode paths work.** — Partially verified: **Windows verified,
  macOS not achievable here.** On Windows:
  `job_orchestration_integration.rs::unicode_bengali_input_and_destination_round_trip`
  runs a real job with Bengali-script input/output paths and directory
  names through the real sidecar; `engine::sidecar`'s
  `probe_unicode_paths_is_true_for_the_real_utf8_manifest_sidecar` confirms
  the runtime capability probe. On macOS: not run (no Mac available) —
  `docs/release-process.md` notes this is architecturally lower-risk on
  macOS (Apple filesystem APIs are natively UTF-8, unlike Windows, which
  needs the embedded manifest this project ships), but "architecturally
  lower-risk" is not the same as verified, and this document does not
  claim it is.

- [~] **UI remains responsive during large fixture processing.** —
  Partially verified / not load-tested at production scale. The
  architecture supports this by design: job execution runs on Tokio's
  async runtime with non-blocking, throttled event streaming to the
  frontend (`jobs/process.rs`), and every filesystem-scanning operation
  reachable from a command handler is dispatched via `spawn_blocking`
  rather than run on the async/UI-adjacent thread
  (`application::run_blocking`). However, `fixtures/` is explicitly "small,
  synthetic PGN files" (`fixtures/README.md`) — no test in this suite
  exercises a genuinely large collection (the sizes architecture.md §19.1
  targets), and no load/stress test was run in this phase or evidenced
  from an earlier one. This is a real gap: the design is sound, but "the
  UI stays responsive" has not been empirically demonstrated at scale.

- [x] **Unknown metrics are not displayed as zero.** — Verified.
  `ProcessingMetrics::broken_games` is unconditionally `None` (never
  computed — see `docs/engine-capabilities.md`'s "verified surprise" #3 for
  why deriving it would have been actively misleading, not just
  unavailable); the frontend's `formatCount`/`NOT_AVAILABLE` formatter
  renders any `null`/`None` metric as "Not available," and
  `ResultsScreen.test.tsx::renders unknown metrics as "Not available",
  never 0 (§9.3, §13.7, §25 binding rule)` asserts this directly by name.

- [x] **Core workflow is keyboard accessible.** — Verified, and
  strengthened this phase. Every control across all five steps is a native
  semantic element (real `<button>`/`<input>`/`<select>`/
  `<fieldset><legend>`/native `<dialog>` for the one modal) — no custom
  widget reimplements keyboard behavior. This phase added: focus movement
  to each step's own heading on arrival (`src/components/useFocusOnMount.ts`,
  wired into all six screen components), so both keyboard and
  screen-reader users get a clear signal when a step changes, which was
  previously missing; a screen-reader announcement of the terminal job
  outcome (succeeded/failed/cancelled) on the Results screen, which
  previously had no reliable announcement path (`ResultsScreen.tsx`'s new
  `useEffect` + `useAnnounce()`). Automated coverage: 6 new `jest-axe`
  tests (one per workflow screen, including Review with errors/warnings
  visible and Run mid-execution), all passing with zero violations —
  `src/features/*/​*.test.tsx`, `src/test/a11y.ts`. Verified live via the
  browser this phase: real Tab-key navigation moves focus in DOM order
  with no repeats or traps, and `:focus-visible` styling is confirmed to
  render correctly (including on the newly-focused step headings). See
  "Accessibility" below for the full account, including what automated
  axe checks in a jsdom test environment can and cannot verify.

- [x] **Error messages include actionable remediation.** — Verified.
  `PublicError` carries an optional `remediation` field
  (`src-tauri/src/domain/result.rs`); nearly every one of the ~20 error
  constructors in `src-tauri/src/errors/mod.rs` supplies one (e.g.
  "Choose a different output folder or base name," "Free up disk space and
  run again," "Reinstall PGN Studio"). The sole deliberate exception is
  `job_cancelled()`, which has none by design — cancellation is a normal,
  user-initiated outcome, not a fault to recover from
  (`job_cancelled_has_no_remediation` test asserts this is intentional,
  not an oversight).

---

## Distribution and compliance

- [ ] **Windows installer is signed for public release.** — Not achievable
  here. No Windows code-signing certificate is available in this
  environment. `.github/workflows/engine.yml` produces a genuine,
  installable, **unsigned** NSIS/MSI bundle via Tauri's own bundler; a
  real (currently inert, secret-gated) signing step is stubbed in that
  same workflow, ready to activate once a certificate is provisioned. See
  `docs/release-process.md`.

- [ ] **macOS app and sidecar are signed and notarized.** — Not achievable
  here. No Mac and no Apple Developer ID Application certificate are
  available in this environment (decisions ledger D-006). The macOS CI
  legs have since executed on real hardware and build a working sidecar,
  but they remain marked `unverified: true` and no macOS *bundle* has
  ever been produced — the legs stop at engine verification, well before
  `tauri build`. A notarization step is stubbed, inert, and secret-gated
  in `.github/workflows/engine.yml`, ready to activate once credentials
  are provisioned.

- [x] **Exact engine source and patches are available.** — Verified.
  `engine-src/upstream.lock` pins the exact commit (with a mirror for
  resilience against an upstream force-push/deletion);
  `engine-src/patches/` documents that zero source patches exist for
  pgn-extract, and the one build-recipe deviation for TRE (not a source
  patch). The Windows build is independently confirmed byte-reproducible
  on a fixed MSVC toolset (a clean rebuild with cache and binary wiped
  reproduced an identical SHA-256) — the strongest available evidence
  that "exact source" claims are actually true, not merely asserted.
  Across *different* MSVC toolsets the binary differs, which is expected
  and does not weaken the claim: `/Brepro` removes time, not compiler
  version, as a build input, and nothing in the integrity chain compares
  against an externally-pinned hash. See `engine-src/README.md`'s "What
  `/Brepro` does not fix" for the measured evidence.

- [~] **GPL and third-party notices are bundled.** — Partially verified.
  `LICENSE` (GPL-3.0-or-later) and `THIRD_PARTY_NOTICES.md` are complete
  and thorough for the three components actually bundled at the binary
  level — pgn-extract, TRE, and `eco.pgn` (including `eco.pgn`'s
  deliberately unresolved licensing status, stated plainly rather than
  guessed at). What is **not** done: runtime dependency (Rust crate / npm
  package) license notices are not yet collected —
  `THIRD_PARTY_NOTICES.md`'s own "Runtime dependency notices" section says
  so, and `scripts/generate-notices.*` (the tool meant to automate this)
  is not yet implemented. A real public release needs this closed first.

- [~] **Release checksums are published.** — Partially verified. The
  mechanism exists and is exercised for the engine binary specifically:
  `build-pgn-extract.ps1` writes `checksums.json` at build time, and both
  CI and the running app itself verify against it (two independent gates —
  package time and startup). What does not yet exist is a full
  release-artifact checksum publication step — `scripts/package-release.*`
  (which would checksum and publish the actual installer files alongside a
  GitHub Release) is not yet implemented.

- [x] **A clean system does not require the user to install `pgn-extract`
  manually.** — Verified by construction, on Windows. The sidecar is
  declared in `tauri.conf.json`'s `bundle.externalBin` and is resolved at
  runtime from the app's own bundled resource directory in release builds
  (`engine::sidecar::SidecarLocation`, `application::startup::sidecar_location`)
  — no separate install step, PATH entry, or user action is required. This
  has not been confirmed via an actual clean-machine install test in this
  phase (that would require installing the produced, unsigned `.msi`/NSIS
  installer on a genuinely clean Windows machine with no prior development
  tooling) — the claim rests on code/configuration inspection and the
  existing engine startup self-test, not an end-to-end clean-install
  smoke test.

---

## Accessibility detail (architecture.md §13.8)

Beyond the §25 "core workflow is keyboard accessible" line item, this phase
did the full accessibility pass §13.8 asks for:

- **WCAG 2.2 AA contrast** — actually computed, not asserted. Every
  text/background and non-text UI-component (border, focus-ring) color
  pair in `src/styles/tokens.css`, for both the light and dark palettes,
  was run through the real WCAG relative-luminance/contrast-ratio formula.
  Every text pair already passed comfortably (best light-mode case 17.79:1,
  worst 5.72:1, both well over the 4.5:1 AA floor). Ten non-text pairs
  failed the 3:1 floor (WCAG 2.2 SC 1.4.11) — as low as 1.43:1 — and were
  corrected: `--color-border` (both modes) and all four status
  `*-border` tokens (both modes). Every one now clears 3:1 against every
  background it is actually used with, verified against the worst-case
  background variant for `--color-border` specifically (`--color-bg-subtle`,
  not just `--color-bg`). Exact before/after values and ratios are recorded
  as comments directly in `tokens.css`. One apparent failure
  (`--color-focus-ring` against `--color-primary`, ~1:1) was investigated
  and confirmed *not* a real defect: `outline-offset: 2px` means the focus
  ring is drawn entirely outside a focused element's own fill, over the
  surrounding background — confirmed both from CSS outline semantics and
  by inspecting a live, focused primary button in the browser
  (`outlineOffset: "2px"`, ring color vs. actual page background is
  6.70:1/8.99:1).
- **Keyboard navigation, focus states, tab order** — see the §25 item
  above.
- **Screen-reader announcements** — a shared, persistent polite live
  region (`src/components/LiveAnnouncer.tsx`) already announced job stage
  transitions; this phase closed the one real gap found (the terminal
  succeeded/failed/cancelled outcome was not reliably announced, because
  `RunScreen` unmounts before it can react to that transition and a freshly
  -inserted `role="status"`/`role="alert"` subtree is not reliably picked
  up by screen readers the way a mutation to an already-present live
  region is).
- **No meaning by color alone** — verified by inspection:
  `src/components/Banner.tsx` pairs every color with a text label and a
  distinct glyph; `src/components/Stepper.tsx` pairs current-step styling
  with `aria-current="step"` and a visible number/label.
- **Reduced motion** — `@media (prefers-reduced-motion: reduce)` in
  `src/styles/global.css` zeroes every animation/transition duration
  globally; confirmed present and correctly scoped by inspection (not
  independently re-verified live this phase, since the browser tool used
  here does not expose a way to toggle this media feature — the light/dark
  `prefers-color-scheme` toggle was exercised live, `prefers-reduced-motion`
  was not).
- **Automated assertions** — `jest-axe`/`axe-core` added as devDependencies
  only (never bundled into the shipped app), confirmed to run entirely
  offline (axe-core ships its full rule set inside the package; nothing
  about running these tests performs network I/O). Important, honest
  caveat: this project's Vitest config sets `css: false` (CSS imports are
  stripped in tests for speed), so axe's `color-contrast` rule has no real
  computed styles to evaluate inside these component tests — the axe
  checks are genuinely meaningful for structural/semantic accessibility
  (labels, roles, ARIA validity, accessible names, duplicate ids) but
  contribute nothing to contrast verification. Contrast compliance rests
  entirely on the direct WCAG-formula computation described above, which
  is the more rigorous approach in any case.

## Observability detail (architecture.md §22)

- **Structured local logging** — a `tracing-subscriber` writer, installed
  at app startup, now actually captures what were previously no-op
  `tracing::*!` calls (see `src-tauri/src/observability/mod.rs`). Every
  event carries a timestamp and level (subscriber-provided); `job_id` is
  attached automatically for the whole life of a job via
  `#[tracing::instrument]` on `jobs::run::run_job`; `component` and
  (for job stage/state/completion events specifically) `stage`/`status`/
  `error_code` fields were added at the relevant call sites this phase.
- **Bounded retention** — `observability::enforce_retention` caps both file
  count (14) and total size (50 MiB), tested (`enforce_retention_deletes_oldest_first_beyond_the_file_count_bound`,
  `..._beyond_the_total_size_bound`).
- **"Clear Logs"** — implemented as a real, tested, registered IPC command
  (`clear_logs`, `src-tauri/src/commands/logs.rs`), not just a backend
  function — callable end to end, though no dedicated diagnostics/settings
  screen consumes it yet (see "Known gaps" below).
- **No telemetry (architecture.md §22.3, binding)** — verified by
  construction: this project's own source contains no HTTP client, no
  `fetch`/`XMLHttpRequest`/`WebSocket` call anywhere in `src/`, and no
  analytics/crash-reporting SDK is a dependency. `reqwest`/`hyper` do
  appear in `Cargo.lock` and are compiled into the final binary — but only
  as an inert transitive dependency of the Tauri framework core itself
  (`cargo tree -i reqwest` traces the only path back to `tauri`, not to
  any code this project wrote); no auto-update plugin is enabled
  (`UpdateCheckPolicy` has exactly one variant, `Off`), and nothing in this
  codebase constructs a `reqwest`/`hyper` client or makes an outbound call.

## Known, honest gaps not covered by a §25 line item

- **A dedicated Settings screen does not exist.** `SettingsDto`/
  `update_settings` are fully implemented and tested on the backend, and
  registered end to end over IPC, but there is no UI to change them —
  `src/features/settings/README.md` still says "not implemented yet." This
  is not required by any §25 acceptance line, and none of §5.1's MVP
  functional list mentions an app-wide settings screen, so it was
  deliberately left out of this phase's scope in favor of the
  explicitly-required work.
- **A "recent jobs" browsing screen and history-based rerun do not exist.**
  This is not an oversight: architecture.md §5.2 explicitly places "recent
  jobs and rerun" in Version 1.1, not the V1 MVP, and no §25 line item
  mentions it. The backend persistence this would need is already fully
  implemented and tested (`persistence::history`, bounded eviction wired
  to the `maxRecentJobs` setting, `list_recent_jobs`/`get_job`/
  `delete_job_history` registered and callable end to end). What the
  workflow *does* offer today, entirely within V1 scope, is an immediate
  "Rerun Job" action on the Results screen that returns to Review with the
  same configuration for a fresh run — a different, smaller feature than
  browsing persisted history, but a real one.
- **`scripts/generate-notices.*` and `scripts/package-release.*` remain
  unimplemented** — see the two "Partially verified" distribution items
  above.
