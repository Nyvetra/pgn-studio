// SPDX-License-Identifier: GPL-3.0-or-later
//! File-identity aliasing detection and canonical-path prediction
//! (architecture.md §11.1, §11.2; design-02 §3.1, Decision D-16).
//!
//! **Binding rule:** aliasing is decided by *file identity*
//! (Windows: volume serial + file index; Unix: dev+ino), never by comparing
//! path strings. String comparison cannot catch hard links, junctions, 8.3
//! short names, `subst` drives, UNC-vs-mapped-drive, or case differences.
//!
//! This module uses the `same-file` crate's [`same_file::Handle`] for the
//! primary (paths-that-exist) check rather than hand-rolled
//! `GetFileInformationByHandle`/`stat` FFI — design-02 §3.1 explicitly
//! allows either ("the `same-file` crate's `Handle`, or direct
//! `windows-sys` calls"), and `same-file` already implements exactly the
//! `(volume serial, file index)` / `(dev, ino)` comparison design-02
//! specifies (verified by reading its `win.rs`/`unix.rs` source: the
//! Windows `Key` struct is `{volume, index}`, nothing else), is a widely
//! used, narrowly-scoped crate (it is `walkdir`'s own symlink-loop
//! detector), and works on directories as well as files - which the
//! pre-publication re-check (§3.4 step 6a) needs for the destination
//! directory.

use std::io;
use std::path::{Path, PathBuf};

/// Returns `true` iff `a` and `b` refer to the same file or directory on
/// disk, by identity (not path string). Both paths must already exist;
/// callers with a not-yet-existing candidate path must use
/// [`predict_final_path`] + [`paths_equal_case_folded`] instead (layer 2).
pub fn is_same_file(a: &Path, b: &Path) -> io::Result<bool> {
    same_file::is_same_file(a, b)
}

/// Canonicalizes `dir` (which must already exist - every call site in this
/// codebase validates the destination directory before reaching here) and
/// joins `file_name` at the string level, **without** touching the
/// filesystem for the joined result (design-02 §3.1 layer 2: "canonicalize
/// the destination directory... append the artifact file names").
///
/// The returned path keeps whatever form `std::fs::canonicalize` produces
/// (on Windows, a `\\?\`-prefixed extended-length path) - that form is the
/// *more* robust one for actual filesystem operations (it bypasses `MAX_PATH`
/// and drive-mapping ambiguity), so it is not stripped here. Comparison-only
/// code must go through [`paths_equal_case_folded`], which does the
/// stripping "for comparison purposes only" as design-02 specifies.
pub fn predict_final_path(dir: &Path, file_name: &str) -> io::Result<PathBuf> {
    let canonical_dir = std::fs::canonicalize(dir)?;
    Ok(canonical_dir.join(file_name))
}

/// Strips the `\\?\` / `\\?\UNC\` extended-length prefix Windows'
/// `canonicalize` (`GetFinalPathNameByHandleW`) produces, "for comparison
/// purposes only" (design-02 §3.1). Non-Windows paths pass through
/// unchanged (canonical Unix paths never carry this prefix).
fn strip_verbatim_prefix(p: &Path) -> String {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.into_owned()
    }
}

/// Compares two canonicalized paths for equality under design-02 §3.1's
/// platform case rules: "Windows & macOS: Unicode simple case-fold; Linux:
/// byte-exact... Additionally on macOS compare NFC-normalized forms... on
/// Windows/Linux do **not** Unicode-normalize (would create false
/// positives)."
///
/// **Known gap (documented, not silently assumed correct):** the macOS
/// branch below does NOT NFC-normalize before comparing - it uses the same
/// byte-exact rule as Linux. This project has no macOS machine to verify
/// case-fold/normalization behavior against (DECISIONS-LEDGER.md D-006),
/// and guessing at ICU-grade NFC normalization without any way to check it
/// against a real HFS+/APFS volume was judged riskier than clearly leaving
/// it unimplemented. The residual exposure is narrow: this function is only
/// layer 2 (prediction for **not-yet-existing** paths); the moment a path
/// exists, layer 1 ([`is_same_file`], identity-based, not string-based) is
/// authoritative and does not have this gap on any platform, including
/// macOS, because HFS+/APFS itself resolves NFC/NFD spellings of the same
/// name to the same directory entry at the OS level. See this crate's
/// top-level report for the full writeup.
pub fn paths_equal_case_folded(a: &Path, b: &Path) -> bool {
    let a = strip_verbatim_prefix(a);
    let b = strip_verbatim_prefix(b);
    if cfg!(windows) {
        // Windows: Unicode simple case-fold. `str::to_lowercase` performs
        // full Unicode case conversion (not bytewise ASCII-only), which is
        // a reasonable, dependency-free approximation of "simple case-fold"
        // for path comparison purposes - it is not ICU-grade Unicode
        // case-folding (that would need a dedicated crate this project does
        // not otherwise need), but the divergence only matters for a
        // handful of locale-specific letters (e.g. Turkish dotless i) that
        // are not realistic false-negative risks for the file systems this
        // app targets.
        a.to_lowercase() == b.to_lowercase()
    } else {
        // Linux: byte-exact (correct as specified). macOS: also byte-exact
        // here - see the gap documented above.
        a == b
    }
}

/// The authoritative input/output collision check (design-02 §3.1,
/// combining both layers): if `candidate_output` already exists, layer 1
/// (identity) decides; otherwise layer 2 (canonical-path prediction)
/// decides. `existing_input` must already exist (every call site validates
/// inputs before reaching here).
pub fn is_aliased(existing_input: &Path, candidate_output: &Path) -> io::Result<bool> {
    if candidate_output.exists() {
        is_same_file(existing_input, candidate_output)
    } else {
        let input_canonical = std::fs::canonicalize(existing_input)?;
        let output_dir = candidate_output.parent().unwrap_or_else(|| Path::new("."));
        let output_name = candidate_output
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let output_canonical = predict_final_path(output_dir, &output_name)?;
        Ok(paths_equal_case_folded(&input_canonical, &output_canonical))
    }
}

/// Windows reserved device names (design-02 §3.2 step 1): `CON`, `PRN`,
/// `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`, matched case-insensitively,
/// **bare or with any extension** (`nul.pgn` is just as reserved as `NUL`).
pub fn is_reserved_windows_device_name(base_name: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    // "bare or with any extension": take the portion before the first '.'
    // (Windows treats `NUL.anything.pgn` as reserved too - the device name
    // check is on the leading component, not just `Path::file_stem`, which
    // would strip only the *last* extension).
    let stem = base_name.split('.').next().unwrap_or(base_name);
    RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem))
}

/// Probes whether `dir` is writable by creating and immediately deleting a
/// uniquely-named probe file (design-02 §3.2 step 5: "probe: create +
/// delete `.pgnstudio-probe-<uuid8>` via `CREATE_NEW`"). `create_new(true)`
/// maps to Windows `CREATE_NEW` / Unix `O_CREAT|O_EXCL`, so this can never
/// silently truncate or overwrite an existing file even under an
/// astronomically unlucky name collision.
pub fn probe_writable(dir: &Path) -> io::Result<()> {
    let probe_name = format!(
        ".pgnstudio-probe-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    let probe_path = dir.join(probe_name);
    {
        let _file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe_path)?;
    }
    std::fs::remove_file(&probe_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_device_names_bare_and_with_extension() {
        for name in ["NUL", "nul", "Nul.pgn", "COM1", "com1.pgn", "LPT9.tar.gz"] {
            assert!(
                is_reserved_windows_device_name(name),
                "{name} should be reserved"
            );
        }
    }

    #[test]
    fn ordinary_names_are_not_reserved() {
        for name in ["master-clean", "COM10", "LPT0", "NULL", "console"] {
            assert!(
                !is_reserved_windows_device_name(name),
                "{name} should not be reserved"
            );
        }
    }

    // Windows-only by construction, not for convenience. The non-Windows
    // arm of `paths_equal_case_folded` is byte-exact *on purpose* (see that
    // function's doc comment and DECISIONS-LEDGER.md D-006: with no Mac to
    // verify against, guessing at ICU-grade NFC normalization and
    // APFS/HFS+ case-folding was judged riskier than clearly leaving it
    // unimplemented). Running this assertion on macOS would demand a
    // behaviour the project has deliberately not implemented.
    #[cfg(windows)]
    #[test]
    fn case_folded_equality_is_case_insensitive_on_windows_paths() {
        let a = Path::new(r"C:\Games\Out.pgn");
        let b = Path::new(r"c:\games\out.PGN");
        assert!(paths_equal_case_folded(a, b));
    }

    /// The documented gap, asserted rather than left silent - the same
    /// "documented, non-silent skip" pattern D-006 establishes elsewhere
    /// (see `filesystem::folder_scan`'s symlink test). Off Windows the
    /// comparison is byte-exact, so two paths differing only in case are
    /// **not** equal.
    ///
    /// If this test ever starts failing, someone has implemented
    /// case-folding for this platform: update `paths_equal_case_folded`'s
    /// doc comment and D-006 to match, rather than deleting this.
    #[cfg(not(windows))]
    #[test]
    fn case_folded_equality_is_byte_exact_off_windows() {
        let a = Path::new("/games/Out.pgn");
        let b = Path::new("/games/out.PGN");
        assert!(
            !paths_equal_case_folded(a, b),
            "off Windows this comparison is byte-exact by design (D-006); \
             case-insensitivity here would be an unverified guess at APFS/HFS+ semantics"
        );
    }

    #[test]
    fn case_folded_equality_strips_verbatim_prefix() {
        let a = Path::new(r"\\?\C:\Games\Out.pgn");
        let b = Path::new(r"C:\Games\Out.pgn");
        assert!(paths_equal_case_folded(a, b));
    }

    #[test]
    fn case_folded_equality_rejects_genuinely_different_paths() {
        let a = Path::new(r"C:\Games\Out.pgn");
        let b = Path::new(r"C:\Games\Other.pgn");
        assert!(!paths_equal_case_folded(a, b));
    }

    #[test]
    fn is_same_file_true_for_identical_path_twice() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.pgn");
        std::fs::write(&file, b"x").unwrap();
        assert!(is_same_file(&file, &file).unwrap());
    }

    #[test]
    fn is_same_file_false_for_distinct_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pgn");
        let b = dir.path().join("b.pgn");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        assert!(!is_same_file(&a, &b).unwrap());
    }

    #[test]
    fn is_aliased_detects_predicted_not_yet_existing_collision() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.pgn");
        std::fs::write(&input, b"x").unwrap();
        // Candidate output has the same name, same directory, but does not
        // exist yet - must still be caught via layer 2.
        let candidate = dir.path().join("source.pgn");
        assert!(is_aliased(&input, &candidate).unwrap());
    }

    #[test]
    fn is_aliased_false_for_distinct_not_yet_existing_output() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.pgn");
        std::fs::write(&input, b"x").unwrap();
        let candidate = dir.path().join("does-not-exist-yet.pgn");
        assert!(!is_aliased(&input, &candidate).unwrap());
    }

    #[test]
    fn is_aliased_detects_hard_link_via_identity() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("source.pgn");
        std::fs::write(&input, b"x").unwrap();
        let linked = dir.path().join("alias-via-hardlink.pgn");
        std::fs::hard_link(&input, &linked).unwrap();
        // The candidate output *exists* (as a hard link) - layer 1 must
        // catch this even though the file names are completely different.
        assert!(is_aliased(&input, &linked).unwrap());
    }

    #[test]
    fn probe_writable_succeeds_and_leaves_no_trace() {
        let dir = tempfile::tempdir().unwrap();
        probe_writable(dir.path()).unwrap();
        let leftover = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .count();
        assert_eq!(leftover, 0, "probe must delete what it creates");
    }
}
