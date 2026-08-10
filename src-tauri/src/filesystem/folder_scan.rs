// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, deterministic, symlink-safe directory scan for the Files
//! screen's "Add Folder" (architecture.md §13.2; §11.2's case-insensitive
//! `.pgn` rule; §16.1's "disk exhaustion... unbounded... memory use"
//! threat, applied here to a runaway *scan* rather than a runaway output).
//!
//! Pure, synchronous, blocking filesystem walk - callers on the async
//! runtime must run it via `tokio::task::spawn_blocking`
//! (`application::run_blocking`), matching
//! `filesystem::validate::validate_job`'s own precedent.
//!
//! **Recursion default (documented product decision):** a scan is
//! **non-recursive by default** - only the picked folder's own immediate
//! files are matched unless the caller explicitly opts in via
//! [`ScanOptions::recursive`]. Architecture.md §16.1 lists "disk
//! exhaustion... unbounded... memory use" as a threat this app must guard
//! against; silently recursing into every subfolder by default is exactly
//! the kind of surprise that threat model warns about (a user pointing "Add
//! Folder" at a large synced/archive tree could otherwise pull in
//! thousands of unrelated files before ever seeing a count). Recursion
//! stays fully available - callers pass `recursive: true` - but it is an
//! explicit opt-in the frontend must surface as its own control, never a
//! hidden default (see `src/features/inputs/AddFolderPanel.tsx`).
//!
//! **Symlink/reparse-point safety.** Any directory entry that is itself a
//! symlink or NTFS junction/mount point is never followed - neither
//! descended into (if it looks like a directory) nor read through (if it
//! looks like a file) - detected via `DirEntry::file_type()`, which (unlike
//! `Path::metadata`) reports the identity of the link itself rather than
//! its target. Empirically verified on this development machine (both a
//! real NTFS junction created with `mklink /J` and a real directory symlink
//! created with `mklink /D`/`std::os::windows::fs::symlink_dir` are
//! reported by `DirEntry::file_type().is_symlink() == true`, confirmed by a
//! standalone probe binary - see this crate's Phase 2c report) that this
//! single check catches both reparse-point kinds this app can encounter on
//! Windows, so the walk can never leave the tree rooted at the picked
//! folder and can never loop through a directory that links back to an
//! ancestor.
//!
//! As defense in depth beyond that primary guard - and reusing
//! `filesystem::identity`'s own underlying mechanism rather than inventing
//! new path logic (that module's binding rule: "aliasing is decided by file
//! identity... never by comparing path strings") - every directory actually
//! descended into is also checked against a `same_file::Handle` ancestor
//! chain before being entered, so a loop cannot occur even through some
//! reparse configuration `is_symlink()` might not flag on a filesystem this
//! was not tested against.
//!
//! **Bounds.** [`MAX_SCAN_DEPTH`] caps how many folder levels a recursive
//! scan will descend; [`MAX_MATCHED_FILES`] caps how many matching files a
//! single scan returns; [`MAX_ENTRIES_VISITED`] caps the total number of
//! directory entries considered, independent of how many match (guards a
//! folder with huge fan-out but few/no matching files). Hitting any bound
//! sets [`ScanOutcome::truncated`] and appends a human-readable note rather
//! than silently returning an incomplete list or hanging.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Depth of the immediate children of the picked root (non-recursive scans
/// never exceed this - see this module's doc comment for the recursion
/// default decision). Recursive scans are additionally capped here so a
/// pathologically deep tree cannot make the walk run unbounded.
pub const MAX_SCAN_DEPTH: u32 = 32;

/// Hard cap on the number of matching files a single scan returns. Chosen
/// well above architecture.md §19.1's "Small" collection tier (10,000
/// *games*, not files) so ordinary PGN collections are never truncated in
/// practice, while still bounding worst-case memory/time on a pathological
/// folder.
pub const MAX_MATCHED_FILES: usize = 10_000;

/// Hard cap on the total number of directory entries (files + directories)
/// considered, independent of how many actually match.
pub const MAX_ENTRIES_VISITED: usize = 250_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanOptions {
    /// Whether to descend into subfolders at all. **Default should be
    /// `false`** at every call site unless the user explicitly asked for
    /// subfolders - see this module's doc comment.
    pub recursive: bool,
    /// The §11.2 "advanced override": when `true`, every regular file is a
    /// candidate regardless of extension; when `false` (the default),
    /// files are matched only when their extension is `.pgn`, compared
    /// case-insensitively (§11.2's binding default rule).
    pub include_all_extensions: bool,
}

/// The result of one bounded scan.
#[derive(Debug, Clone, Default)]
pub struct ScanOutcome {
    /// Matching file paths, in a **deterministic, reproducible order**:
    /// sorted by [`Ord`] on [`PathBuf`] (component-wise, platform-native
    /// ordinal comparison - case-sensitive, not locale-aware) after the
    /// walk completes, independent of filesystem/OS enumeration order.
    /// Input order determines duplicate-retention priority elsewhere in
    /// this app (architecture.md §10.7), so a nondeterministic scan order
    /// would silently change which copy of a duplicate survives - this is
    /// why the final list is always re-sorted rather than merely relying on
    /// `read_dir`'s (unspecified, platform-dependent) enumeration order.
    pub files: Vec<PathBuf>,
    pub directories_visited: u64,
    pub entries_visited: u64,
    pub truncated: bool,
    /// Human-readable reasons the scan stopped early (depth cap, file-count
    /// cap, entry-count cap) - each distinct reason appears at most once,
    /// even if triggered repeatedly across many branches of the walk.
    pub truncation_notes: Vec<String>,
}

/// Injectable bounds, so tests can exercise the *mechanism* of truncation
/// without literally creating hundreds of thousands of files on disk.
/// [`scan_pgn_directory`] (the real entry point every non-test caller uses)
/// always wires the real [`MAX_SCAN_DEPTH`]/[`MAX_MATCHED_FILES`]/
/// [`MAX_ENTRIES_VISITED`] constants here.
#[derive(Debug, Clone, Copy)]
struct ScanLimits {
    max_depth: u32,
    max_matched_files: usize,
    max_entries_visited: usize,
}

const PRODUCTION_LIMITS: ScanLimits = ScanLimits {
    max_depth: MAX_SCAN_DEPTH,
    max_matched_files: MAX_MATCHED_FILES,
    max_entries_visited: MAX_ENTRIES_VISITED,
};

/// Scans `root` (a folder the user explicitly picked, e.g. via
/// `select_input_directory`) for candidate PGN input files.
///
/// `root` itself is resolved with `std::fs::canonicalize` (following its
/// own reparse chain, if any - the user explicitly chose this exact
/// folder); every reparse point *encountered while walking inside it* is
/// then never followed (see this module's doc comment).
///
/// Returns `Err` only for a failure to use `root` itself (does not exist,
/// is not a directory, or cannot be read/canonicalized) - a permission
/// failure on some *nested* subdirectory partway through the walk is not
/// fatal to the whole scan (that subdirectory simply contributes no files),
/// matching `application::inputs::inspect_inputs`'s own "one bad entry
/// never fails the whole batch" philosophy.
pub fn scan_pgn_directory(root: &Path, options: &ScanOptions) -> std::io::Result<ScanOutcome> {
    scan_pgn_directory_with_limits(root, options, PRODUCTION_LIMITS)
}

fn scan_pgn_directory_with_limits(
    root: &Path,
    options: &ScanOptions,
    limits: ScanLimits,
) -> std::io::Result<ScanOutcome> {
    let root_metadata = std::fs::metadata(root)?;
    if !root_metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the selected path is not a folder",
        ));
    }
    let canonical_root = std::fs::canonicalize(root)?;
    let root_handle = same_file::Handle::from_path(&canonical_root)?;

    let mut state = WalkState::new(limits);
    let mut ancestors = vec![root_handle];
    walk_dir(&canonical_root, 0, &mut ancestors, options, &mut state);

    // Deterministic final ordering - see `ScanOutcome::files`'s doc comment.
    state.matched.sort();

    Ok(ScanOutcome {
        files: state.matched,
        directories_visited: state.directories_visited,
        entries_visited: state.entries_visited as u64,
        truncated: state.truncated,
        truncation_notes: state.truncation_notes,
    })
}

struct WalkState {
    limits: ScanLimits,
    matched: Vec<PathBuf>,
    directories_visited: u64,
    entries_visited: usize,
    truncated: bool,
    truncation_notes: Vec<String>,
    noted_reasons: HashSet<&'static str>,
}

impl WalkState {
    fn new(limits: ScanLimits) -> Self {
        Self {
            limits,
            matched: Vec::new(),
            directories_visited: 0,
            entries_visited: 0,
            truncated: false,
            truncation_notes: Vec::new(),
            noted_reasons: HashSet::new(),
        }
    }

    fn note_once(&mut self, key: &'static str, text: impl FnOnce() -> String) {
        self.truncated = true;
        if self.noted_reasons.insert(key) {
            self.truncation_notes.push(text());
        }
    }
}

fn walk_dir(
    dir: &Path,
    depth: u32,
    ancestors: &mut Vec<same_file::Handle>,
    options: &ScanOptions,
    state: &mut WalkState,
) {
    state.directories_visited += 1;

    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        // A nested directory we cannot read simply contributes no files -
        // not fatal to the overall scan (see this module's top doc comment).
        Err(_) => return,
    };

    // Sort this directory's own children before processing so that *which*
    // files survive a mid-walk truncation is itself deterministic, not
    // incidentally dependent on filesystem enumeration order.
    let mut children: Vec<std::fs::DirEntry> = read_dir.filter_map(|e| e.ok()).collect();
    children.sort_by_key(|e| e.file_name());

    for entry in children {
        if state.entries_visited >= state.limits.max_entries_visited {
            let cap = state.limits.max_entries_visited;
            state.note_once("entries-cap", || {
                format!(
                    "Stopped after checking {cap} files/folders; some files may be missing from \
                     this list."
                )
            });
            return;
        }
        state.entries_visited += 1;

        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_symlink() {
            // Never follow a symlink/junction/reparse point, whether it
            // looks like a file or a directory - see this module's doc
            // comment for the empirical verification behind this rule.
            continue;
        }

        if file_type.is_dir() {
            if !options.recursive {
                continue;
            }
            if depth + 1 > state.limits.max_depth {
                let cap = state.limits.max_depth;
                state.note_once("depth-cap", || {
                    format!(
                        "Stopped descending past {cap} folder levels; some files may be missing \
                         from this list."
                    )
                });
                continue;
            }
            let path = entry.path();
            // Defense-in-depth identity check (reuses the same
            // `same_file::Handle` primitive `filesystem::identity` is built
            // on - see this module's doc comment): refuse to re-enter a
            // directory whose *identity* already appears on the current
            // ancestor chain, even though the reparse-point check above
            // already refuses to follow the two link kinds actually
            // observed on this platform.
            match same_file::Handle::from_path(&path) {
                Ok(handle) => {
                    if ancestors.contains(&handle) {
                        continue;
                    }
                    ancestors.push(handle);
                    walk_dir(&path, depth + 1, ancestors, options, state);
                    ancestors.pop();
                }
                Err(_) => continue,
            }
            if state.entries_visited >= state.limits.max_entries_visited
                || state.matched.len() >= state.limits.max_matched_files
            {
                return;
            }
            continue;
        }

        if !file_type.is_file() {
            continue; // devices, pipes, etc. - never relevant
        }

        let path = entry.path();
        if !options.include_all_extensions {
            let is_pgn = path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("pgn"))
                .unwrap_or(false);
            if !is_pgn {
                continue;
            }
        }

        if state.matched.len() >= state.limits.max_matched_files {
            let cap = state.limits.max_matched_files;
            state.note_once("files-cap", || {
                format!(
                    "Stopped after finding {cap} matching files; some files may be missing from \
                     this list."
                )
            });
            return;
        }
        state.matched.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(recursive: bool, include_all_extensions: bool) -> ScanOptions {
        ScanOptions {
            recursive,
            include_all_extensions,
        }
    }

    #[test]
    fn finds_pgn_files_case_insensitively_in_the_root_only_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.pgn"), b"x").unwrap();
        std::fs::write(tmp.path().join("B.PGN"), b"x").unwrap();
        std::fs::write(tmp.path().join("c.Pgn"), b"x").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        let sub = tmp.path().join("subfolder");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.pgn"), b"x").unwrap();

        let outcome = scan_pgn_directory(tmp.path(), &options(false, false)).unwrap();
        let names: Vec<String> = outcome
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["B.PGN", "a.pgn", "c.Pgn"]);
        assert!(!outcome.truncated);
    }

    #[test]
    fn recursive_true_descends_into_subfolders() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.pgn"), b"x").unwrap();
        let sub = tmp.path().join("subfolder");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.pgn"), b"x").unwrap();

        let outcome = scan_pgn_directory(tmp.path(), &options(true, false)).unwrap();
        let names: HashSet<String> = outcome
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            HashSet::from(["a.pgn".to_string(), "nested.pgn".to_string()])
        );
        assert!(outcome.directories_visited >= 2);
    }

    #[test]
    fn include_all_extensions_matches_non_pgn_files_too() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.pgn"), b"x").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), b"x").unwrap();

        let default_scan = scan_pgn_directory(tmp.path(), &options(false, false)).unwrap();
        assert_eq!(default_scan.files.len(), 1);

        let override_scan = scan_pgn_directory(tmp.path(), &options(false, true)).unwrap();
        assert_eq!(override_scan.files.len(), 2);
    }

    #[test]
    fn result_order_is_deterministic_across_repeated_scans() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["zeta.pgn", "alpha.pgn", "Mu.pgn", "beta.pgn"] {
            std::fs::write(tmp.path().join(name), b"x").unwrap();
        }
        let first = scan_pgn_directory(tmp.path(), &options(false, false)).unwrap();
        let second = scan_pgn_directory(tmp.path(), &options(false, false)).unwrap();
        assert_eq!(first.files, second.files);
        // Pinned exact order (component-wise Ord: ASCII-uppercase sorts
        // before ASCII-lowercase) so this test fails loudly if the ordering
        // rule ever silently changes.
        let names: Vec<String> = first
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["Mu.pgn", "alpha.pgn", "beta.pgn", "zeta.pgn"]);
    }

    #[test]
    fn nonexistent_root_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert!(scan_pgn_directory(&missing, &options(false, false)).is_err());
    }

    #[test]
    fn a_file_path_as_root_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.pgn");
        std::fs::write(&file, b"x").unwrap();
        assert!(scan_pgn_directory(&file, &options(false, false)).is_err());
    }

    #[test]
    fn an_empty_nested_directory_does_not_prevent_other_matches_from_being_found() {
        // No portable, unprivileged way to make a directory genuinely
        // unreadable on Windows in a test (see `walk_dir`'s `Err(_) =>
        // return` branch for the code path this would otherwise exercise)
        // - this instead proves the adjacent, sibling-level guarantee: one
        // directory contributing zero files (here, an *empty* subfolder)
        // never prevents matches found elsewhere in the same tree from
        // being returned.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.pgn"), b"x").unwrap();
        let empty_sub = tmp.path().join("empty");
        std::fs::create_dir(&empty_sub).unwrap();
        let outcome = scan_pgn_directory(tmp.path(), &options(true, false)).unwrap();
        assert_eq!(outcome.files.len(), 1);
    }

    #[test]
    fn matched_files_cap_truncates_deterministically_and_reports_it() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..10 {
            std::fs::write(tmp.path().join(format!("f{i:02}.pgn")), b"x").unwrap();
        }
        let limits = ScanLimits {
            max_depth: MAX_SCAN_DEPTH,
            max_matched_files: 3,
            max_entries_visited: MAX_ENTRIES_VISITED,
        };
        let outcome =
            scan_pgn_directory_with_limits(tmp.path(), &options(false, false), limits).unwrap();
        assert_eq!(outcome.files.len(), 3);
        assert!(outcome.truncated);
        assert_eq!(outcome.truncation_notes.len(), 1);
        assert!(outcome.truncation_notes[0].contains("3 matching files"));
        // Deterministic: the *first three* in sorted order, not whichever
        // three the filesystem happened to enumerate first.
        let names: Vec<String> = outcome
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["f00.pgn", "f01.pgn", "f02.pgn"]);
    }

    #[test]
    fn entries_visited_cap_truncates_before_scanning_everything() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..10 {
            std::fs::write(tmp.path().join(format!("f{i:02}.pgn")), b"x").unwrap();
        }
        let limits = ScanLimits {
            max_depth: MAX_SCAN_DEPTH,
            max_matched_files: MAX_MATCHED_FILES,
            max_entries_visited: 3,
        };
        let outcome =
            scan_pgn_directory_with_limits(tmp.path(), &options(false, false), limits).unwrap();
        assert_eq!(outcome.entries_visited, 3);
        assert!(outcome.truncated);
        assert!(outcome.truncation_notes[0].contains("checking 3 files/folders"));
    }

    #[test]
    fn depth_cap_excludes_files_beyond_the_real_production_limit() {
        // Exercises the REAL `MAX_SCAN_DEPTH` constant (not an injected
        // small value) with a genuinely nested tree, kept well under
        // Windows' ~260-char MAX_PATH by using very short segment names.
        let tmp = tempfile::tempdir().unwrap();
        let mut dir = tmp.path().to_path_buf();
        // One file safely inside the limit...
        for i in 0..(MAX_SCAN_DEPTH - 2) {
            dir = dir.join(format!("d{i}"));
        }
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("within.pgn"), b"x").unwrap();
        // ...and a few levels further, past it.
        for i in (MAX_SCAN_DEPTH - 2)..(MAX_SCAN_DEPTH + 4) {
            dir = dir.join(format!("d{i}"));
        }
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("beyond.pgn"), b"x").unwrap();

        let outcome = scan_pgn_directory(tmp.path(), &options(true, false)).unwrap();
        let names: HashSet<String> = outcome
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains("within.pgn"), "found: {names:?}");
        assert!(!names.contains("beyond.pgn"), "found: {names:?}");
        assert!(outcome.truncated);
        assert!(outcome
            .truncation_notes
            .iter()
            .any(|n| n.contains("folder levels")));
    }

    #[test]
    fn a_directory_symlink_is_never_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(real.join("inside.pgn"), b"x").unwrap();
        std::fs::write(tmp.path().join("outside.pgn"), b"x").unwrap();

        let link = tmp.path().join("link-to-real");
        // Windows-verified (this development machine, D-006's framing) via
        // `std::os::windows::fs::symlink_dir`; the `#[cfg(unix)]` branch
        // mirrors `filesystem::platform::unix`'s own precedent - written in
        // good faith against documented `std` behavior, unverified by
        // compilation here.
        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_dir(&real, &link);
        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(&real, &link);

        if let Err(e) = link_result {
            // Creating a directory symlink needs a privilege that is not
            // guaranteed to be held on every machine (unlike NTFS
            // junctions). Empirically confirmed present, unprivileged, on
            // this development machine (see this module's doc comment) -
            // documented, non-silent skip elsewhere is this project's
            // established pattern (DECISIONS-LEDGER.md D-006) for exactly
            // this situation.
            eprintln!(
                "skipping a_directory_symlink_is_never_followed: could not create a test \
                 symlink on this machine ({e}); the reparse-point skip logic itself is still \
                 exercised indirectly by every other scan test since it runs unconditionally"
            );
            return;
        }

        let outcome = scan_pgn_directory(tmp.path(), &options(true, false)).unwrap();
        let names: HashSet<String> = outcome
            .files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains("outside.pgn"));
        // `inside.pgn` is reachable directly through `real/`, which is not
        // itself a symlink - it must still be found via the real path...
        assert!(names.contains("inside.pgn"));
        // ...but the walk must never have entered *through* the symlink -
        // proven by the total match count staying at exactly 2 (one real
        // path to `inside.pgn`, not two).
        assert_eq!(outcome.files.len(), 2, "found: {names:?}");
    }
}
