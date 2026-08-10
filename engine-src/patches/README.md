# engine-src/patches/

Local patches applied on top of the pinned `pgn-extract` commit recorded in
`../upstream.lock`, if any. **Currently empty - no source patches exist**
(zero pgn-extract source edits, per D-002 in the decisions ledger).

## `tre-msvc/` is not a source patch

`tre-msvc/README.md` documents a **build-recipe deviation**, not a source
diff: TRE's own `win32/tre.vcxproj` cannot be used as-is on this project
(x86-only, builds a DLL not a static lib, references a file that does not
exist at the pinned commit), so `scripts/build-pgn-extract.ps1` compiles
`lib/*.c` directly instead. Zero bytes of TRE source are modified either -
see that README for the full recipe and rationale. It lives under
`patches/` (rather than e.g. `docs/`) because it is exactly the kind of
"how does the shipped binary differ from 'clone upstream and run its own
build system'" disclosure that policy point 2 below exists to capture,
even though no unified diff is involved.

## Policy

1. **Prefer upstream first.** If pgn-extract has a bug or missing capability
   PGN Studio needs, prefer reporting/fixing it upstream
   (`https://github.com/kentdjb/pgn-extract`) over carrying a permanent
   fork-only patch. A local patch is a stopgap, not a default.
2. **One file per patch**, named
   `NNNN-short-description.patch` (e.g. `0001-fix-windows-crlf-handling.patch`),
   in unified diff format (`git diff` / `git format-patch` output) against
   the exact commit in `upstream.lock`.
3. **Every patch file must be accompanied by a short rationale** at the top
   of the patch (or a matching `NNNN-short-description.md`) explaining:
   - what upstream behavior is being changed and why;
   - whether it has been reported/submitted upstream, and a link if so;
   - what happens if the patch silently fails to apply on a version bump
     (it must be a hard build failure, not a silent skip).
4. **Patches must be re-verified on every upstream pin update.** Bumping
   `upstream.lock`'s `commit` without checking whether existing patches
   still apply cleanly is not acceptable.
5. **License.** A patch to GPL-3.0-or-later code is itself distributed
   under the same license as part of the combined work; do not introduce
   patch content under an incompatible license.
6. Patches are applied by `scripts/build-pgn-extract.*` (Phase 1, not yet
   implemented) before compiling the sidecar. That script must fail loudly
   if a patch does not apply cleanly - never fall back to unpatched
   sources silently.
