# User guide

PGN Studio is a desktop tool for merging, deduplicating, cleaning, and
filtering PGN (chess game) files in bulk. This guide walks through the
actual five-step workflow as it exists today. It does not cover a
chessboard viewer, a game list/explorer, or a manual PGN editor — none of
those exist in this version; see `architecture.md` §5 for what's
planned for later versions.

Two promises worth knowing up front, because they shape everything below:

- **Your source files are never modified.** Every job reads your inputs and
  writes brand-new output files. Nothing is edited or deleted in place.
- **Nothing is sent anywhere.** PGN Studio does not use the network for
  anything — no telemetry, no update checks by default, no cloud
  processing. Your games stay on your machine. See §22.3 of the
  architecture document and this project's own observability code for how
  that's enforced, not just promised.

## The five steps

The top of the window always shows where you are: Files → Operations →
Filters → Review → Run & Results. You can jump back to any step you've
already reached by clicking it in that bar; steps you haven't reached yet
are shown but disabled, so you can see what's coming without being able to
skip ahead.

### 1. Files

Add the PGN files you want to process, in the order you want them
processed:

- **Add Files** opens a native file picker for one or more `.pgn` files.
- **Add Folder** picks a folder and scans it for `.pgn` files (not
  recursive by default — there's an explicit "include subfolders" option
  if you want that), then shows you what it found before anything is
  actually added, including a warning if the scan had to stop early on an
  unusually large folder.
- You can also drag files onto the window.

Once files are added, each row shows its size and any warnings (not
readable, wrong extension) and offers Move Up / Move Down / Remove. **File
order matters** if you turn on duplicate handling later: PGN Studio always
keeps the *first* copy of a duplicate game, where "first" means first in
this list. See `duplicate-semantics.md` for exactly what counts as a
duplicate.

Finally, choose an output folder and a base filename. Every artifact this
job produces is named from that base (`clean.pgn`,
`clean.duplicates.pgn`, `clean.report.json`, and so on — see "Understanding
your results" below). You also choose what happens if an output file with
that name already exists:

- **Add a number to the new file's name** (the friendly default) — writes
  `clean (1).pgn` instead of overwriting anything.
- **Stop instead of writing over anything** — refuses to run rather than
  risk a collision.
- **Replace the existing file, after confirming** — asks you to confirm on
  the Review screen, then renames the previous file to a timestamped
  `.bak` copy before writing the new one. Nothing is ever silently
  overwritten, in any mode.

### 2. Operations

Start from a preset, or build a configuration by hand — a preset is just a
starting point that fills in the controls below it; every field it sets
remains a normal, editable control, never a hidden command.

| Preset | What it does |
|---|---|
| **Merge Safely** | Combine every source file into one PGN. Nothing is removed — comments, variations, NAGs, and results are all kept. |
| **Clean Collection** | Combine every source, keep only the first copy of each duplicated game, and save the later copies to a separate audit file. Comments and variations are kept. |
| **Minimal Mainline PGN** | Combine sources, remove duplicate games (no audit file), and strip comments, variations, and NAGs, leaving plain mainline move scores. |
| **Lucena-Ready PGN** | Combine sources, keep only unique mainline games, remove comments/variations/NAGs (which also removes any clock times or engine evaluations stored inside comments — the engine can only remove comments as a whole, not selectively), and add ECO opening codes. |
| **Validate Only** | Check every source file for errors and produce a report. No merged games file is written. |
| **New Games Against Master** | Compare one or more files against a master database and keep only the games not already in it. The master file itself is never included in the output. |

If you change anything a preset set, the picker shows "Custom
configuration" instead of the preset name — this just means your current
settings no longer exactly match a built-in preset; nothing is lost, and
you can reapply any preset at any time to reset from a known baseline.

Below the presets:

- **Mode**: Process (produce output files) or Validate Only (report only).
- **Duplicate games**: off, "keep first copy + audit file," or "keep first
  copy, discard the rest." Each option's help text restates the moves-based
  duplicate rule — see `duplicate-semantics.md` for the full explanation,
  including the annotated-duplicate warning you may see after running a
  job with the audit-file option on.
- **Cleanup**: remove comments, variations, and NAGs independently of each
  other.
- **ECO classification**: add opening codes/names, when the engine build
  supports it (it does, in the pinned build this app ships). PGN Studio
  classifies against the engine's own ECO database plus a supplementary
  dataset of 10,642 additional opening lines, so a game usually gets a more
  specific name than the base database alone gives — `1. b4 c5` comes back
  as "Polish Opening / Birmingham Gambit" rather than just "Polish
  (Sokolsky) opening." The base database always takes precedence where it
  has an entry, so the supplement only ever adds detail, never changes an
  existing classification. One cosmetic consequence: the two datasets
  capitalise names differently ("Ware (Meadow Hay) opening" vs "Ware
  Opening"), so both styles can appear in the same output.
- **Master/check file**: an optional single `.pgn` to compare against
  without including its own games in the output — what "New Games Against
  Master" uses under the hood.
- **Audit artifacts**: whether to save a log file and/or a processing
  report (JSON + plain text) alongside your results.

Any option the bundled engine build genuinely does not support is shown
disabled with a plain-language explanation, rather than hidden — you should
never wonder whether a missing control was intentional.

### 3. Filters

Every filter is optional — leave a field blank to not filter on it. All
filters combine with AND: a game must satisfy every filter you've set.
**A game missing a tag you're filtering on is always excluded**, even for
an "exclude"/not-equal filter on that tag — there is no case where a
missing tag counts as a match.

Available filters: player name (either color, or scoped to White/Black),
result (a specific value, or "decisive games only"), Elo (either player, or
scoped), a year range, move-count range, checkmate-only, standard-starting-
position-only vs. games starting from a custom position, an ECO code or
prefix (with an "Exclude" option), a FEN pattern, and a textual
opening-line match. Name and result filters always match by prefix
("starts with"), never by exact equality — see `engine-capabilities.md` for
why exact/relational matching silently fails on most tags in this engine
build, and why this app's filter controls are built to avoid that trap
entirely rather than expose it.

### 4. Review

A full summary of the job you're about to run: the operation in plain
language, your ordered source files, every destination artifact that will
be created, how output conflicts will be handled, the estimated input size,
and any warnings or advisories from validation. An expandable "Advanced"
section shows the exact argument list PGN Studio will hand to the engine —
shown for transparency only; it is never something you type or edit, and
it is never run through a shell (PGN Studio never invokes a shell for
anything).

The Run button stays disabled until validation confirms the job is ready.
If your conflict policy is "Replace after confirming" and a real conflict
exists, clicking Run asks you to confirm the replacement first.

### 5. Run & Results

While the job runs, you'll see the current stage (Preparing, Starting,
Processing, Finalizing), elapsed time, the number of games processed so
far, and the tail of the engine's own log — never a fabricated progress
percentage, since the engine does not report one. You can cancel at any
point; cancelling never touches your source files or any output already
published by an earlier job.

Once the job finishes, the same screen becomes the Results screen:

- A clear success/failure/cancelled status, in text and color together
  (never color alone).
- Metrics — input/output file and game counts, sizes. Anything the engine
  build cannot measure for this job (for example, the broken-games count —
  see `engine-capabilities.md`) is shown as **"Not available,"** never as a
  misleading zero.
- The list of output files actually published, with buttons to open a file
  or reveal it in your file manager.
- **Save Job** exports the complete, reproducible job manifest (every
  setting, the exact argument list, timestamps, and the engine identity
  used) to a file you choose — useful for records or for sharing exactly
  what a job did.
- **Rerun Job** takes you back to Review with the same configuration, so
  you can run it again (a fresh job, with a new job ID).
- **Start New Job** clears everything and returns you to Files.

Every completed job is also recorded in a local, bounded job history
automatically (oldest entries are dropped once the bound is reached) — this
is separate from "Save Job," which is an explicit, portable export you
control.

## Understanding your output files

For a base name like `clean`, PGN Studio only creates the files you asked
for — never an empty duplicates file unless you explicitly turned on
"always create the audit file, even if empty":

| File | Contents |
|---|---|
| `clean.pgn` | The main output — unique games (or all games, if you didn't dedupe) |
| `clean.duplicates.pgn` | Later copies of duplicate games, if you kept the audit file |
| `clean.report.json` / `clean.report.txt` | The processing report, if you asked for one |
| `clean.log.txt` | The engine's own log, if you asked for one |

There is deliberately no `clean.broken.pgn` — see `engine-capabilities.md`
for why that specific file cannot exist with this engine build.

## Accessibility

Every screen is fully operable by keyboard: Tab moves forward through
controls in a logical order, Shift+Tab moves back, and moving to a new step
moves focus to that step's own heading so both keyboard and screen-reader
users get a clear signal the screen changed. Stage and status changes
during a run are announced to screen readers automatically. No information
is conveyed by color alone — every status indicator pairs color with text
and, for banners, an icon. The interface respects your operating system's
reduced-motion preference and text-size/zoom settings.

## Troubleshooting

Every error PGN Studio shows has a title, a plain-language message, and —
where one exists — a remediation suggestion. Internal detail that would not
be meaningful to act on (raw OS error text, stack traces) is never shown in
the UI; it goes to a local log instead, tagged with a technical ID you can
reference if you report a problem. A few you may encounter:

- **"Output already exists"** — pick a different base filename, or change
  the conflict policy on the Files screen.
- **"Output would overwrite a source file"** — your chosen output folder
  and filename resolve to one of your own input files. Choose a different
  output folder or base name; PGN Studio refuses to run rather than risk
  your source data.
- **"A job is already running"** — PGN Studio runs one job at a time by
  design; wait for the current job to finish or cancel it first.
- **"The bundled processing engine did not pass verification"** — the
  installed copy of the engine sidecar does not match its expected
  checksum. Reinstall PGN Studio; do not attempt to work around this
  message.
