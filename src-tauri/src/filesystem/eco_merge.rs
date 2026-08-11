// SPDX-License-Identifier: GPL-3.0-or-later
//! Builds the single ECO classification file the engine's `-e` option is
//! given, by concatenating the bundled `eco.pgn` with PGN Studio's own
//! `eco-supplement.pgn` (architecture.md §12.2; design-02 §4.1's ECO
//! toggle).
//!
//! **Why concatenate at all.** `pgn-extract` accepts exactly one ECO file.
//! Passing `-e` twice does *not* union the two - the second occurrence
//! silently *replaces* the first, and the first file's lines are never
//! loaded at all. Empirically verified against the pinned sidecar
//! (`pgn-extract v26-06`): with `-e<eco.pgn> -e<supplement>`, the game
//! `1. b4 Nh6` - a line present only in `eco.pgn` - came back with **no**
//! `[ECO]`/`[Opening]` tags whatsoever, while the same game with
//! `-e<eco.pgn>` alone came back as `[ECO "A00"] [Opening "Polish"]
//! [Variation "Tuebingen variation"]`. A merged file is therefore the only
//! way to use both datasets.
//!
//! **Why the bundled file goes first, always.** Also empirically verified
//! against the same binary: when one move sequence appears twice in an ECO
//! file, `pgn-extract` resolves it to its **first** occurrence. Emitting
//! the bundled content first therefore makes every classification it
//! already provides authoritative and unoverridable. This is belt-and-
//! braces: `scripts/build-eco-supplement.mjs` also excludes every line that
//! already exists in `eco.pgn` when it generates the supplement, so a
//! collision should not arise in the first place. Both guarantees are
//! independently tested (see this module's tests, plus the end-to-end
//! regression in `tests/eco_supplement_integration.rs`, which replays
//! *every* line of the bundled `eco.pgn` through the real engine both ways
//! and asserts the classifications are identical).
//!
//! **The bundled `eco.pgn` is never written to.** It is a third-party file
//! redistributed byte-for-byte unmodified and checksummed in
//! `resources/pgn-extract/SOURCE.json` (`"modified": false`); this module
//! only ever opens it for reading. The merged artifact is written to the
//! app cache directory, never into `resources/`.

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Basename of the merged artifact inside [`merged_dir`].
const MERGED_FILE_NAME: &str = "eco-merged.pgn";
/// Records which input versions the current merged file was built from, so
/// an unchanged pair is not re-concatenated on every launch.
const STAMP_FILE_NAME: &str = "eco-merged.stamp";

fn merged_dir(cache_root: &Path) -> PathBuf {
    cache_root.join("eco")
}

/// Identity of one input, cheap to compute and sufficient to detect a
/// changed resource across an app upgrade (length alone would miss a
/// same-size edit; mtime alone is unreliable across installers that
/// preserve timestamps).
fn input_stamp(path: &Path) -> std::io::Result<String> {
    let meta = std::fs::metadata(path)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Ok(format!("{}:{}", meta.len(), modified))
}

/// Returns the path the engine's `-e` option should be given.
///
/// Produces `<cache_root>/eco/eco-merged.pgn` (bundled content first, then
/// the supplement), rebuilding it only when either input has changed since
/// the last build.
///
/// **Never fails the caller.** If the supplement is absent (it is an
/// optional resource) or anything about the merge goes wrong - unwritable
/// cache directory, disk full, a torn previous run - this returns
/// `bundled` unchanged, so ECO classification degrades to exactly the
/// pre-supplement behavior instead of breaking. The reason is returned to
/// the caller for logging via the `Result`'s `Err` arm being deliberately
/// absent: callers get a path plus an optional diagnostic.
pub fn resolve_eco_file(bundled: &Path, supplement: &Path, cache_root: &Path) -> EcoFileChoice {
    if !supplement.is_file() {
        return EcoFileChoice {
            path: bundled.to_path_buf(),
            merged: false,
            note: Some("no eco-supplement.pgn resource found".to_string()),
        };
    }
    match build_merged(bundled, supplement, cache_root) {
        Ok(path) => EcoFileChoice {
            path,
            merged: true,
            note: None,
        },
        Err(e) => EcoFileChoice {
            path: bundled.to_path_buf(),
            merged: false,
            note: Some(format!("could not build the merged ECO file: {e}")),
        },
    }
}

/// Which ECO file the engine will be given, and why.
#[derive(Debug, Clone)]
pub struct EcoFileChoice {
    pub path: PathBuf,
    /// `true` when `path` is the merged artifact; `false` when it is the
    /// bundled `eco.pgn` fallback.
    pub merged: bool,
    /// Human-readable reason the fallback was taken, for logging only.
    pub note: Option<String>,
}

fn build_merged(bundled: &Path, supplement: &Path, cache_root: &Path) -> std::io::Result<PathBuf> {
    let dir = merged_dir(cache_root);
    let merged = dir.join(MERGED_FILE_NAME);
    let stamp_path = dir.join(STAMP_FILE_NAME);

    let want_stamp = format!("{}\n{}\n", input_stamp(bundled)?, input_stamp(supplement)?);

    if merged.is_file() {
        if let Ok(existing) = std::fs::read_to_string(&stamp_path) {
            if existing == want_stamp {
                return Ok(merged);
            }
        }
    }

    std::fs::create_dir_all(&dir)?;

    // Write to a temp file and rename into place, so a process killed
    // mid-write can never leave a truncated ECO file that would silently
    // misclassify games on the next run (the same atomic-publication rule
    // `filesystem::publish` applies to job output).
    let tmp = dir.join(format!("{MERGED_FILE_NAME}.tmp"));
    {
        let out = File::create(&tmp)?;
        let mut writer = BufWriter::with_capacity(64 * 1024, out);
        // Order is load-bearing: bundled FIRST - see this module's doc
        // comment.
        for input in [bundled, supplement] {
            let mut reader = BufReader::with_capacity(64 * 1024, File::open(input)?);
            std::io::copy(&mut reader, &mut writer)?;
            // A supplement whose first tag lands on the same line as the
            // bundled file's last token would be unparseable; a blank line
            // between the two is always safe and never changes semantics.
            writer.write_all(b"\n\n")?;
        }
        writer.flush()?;
    }
    std::fs::rename(&tmp, &merged)?;

    // Best-effort: a missing/unwritable stamp only costs a rebuild next
    // launch, so it must not fail the merge that already succeeded.
    let _ = std::fs::write(&stamp_path, want_stamp);

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = "{header}\n\n[ECO \"A00\"]\n[Opening \"Polish\"]\n\n1. b4 *\n";
    const SUPPLEMENT: &str = "{supp}\n\n[ECO \"A01\"]\n[Opening \"Nimzo-Larsen\"]\n\n1. b3 *\n";

    fn fixture(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let bundled = dir.join("eco.pgn");
        let supplement = dir.join("eco-supplement.pgn");
        let cache = dir.join("cache");
        std::fs::write(&bundled, BUNDLED).unwrap();
        std::fs::write(&supplement, SUPPLEMENT).unwrap();
        (bundled, supplement, cache)
    }

    #[test]
    fn merged_file_contains_bundled_content_first_then_the_supplement() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundled, supplement, cache) = fixture(tmp.path());

        let choice = resolve_eco_file(&bundled, &supplement, &cache);

        assert!(choice.merged, "note: {:?}", choice.note);
        let text = std::fs::read_to_string(&choice.path).unwrap();
        let bundled_at = text.find("Polish").expect("bundled content present");
        let supplement_at = text
            .find("Nimzo-Larsen")
            .expect("supplement content present");
        assert!(
            bundled_at < supplement_at,
            "bundled content must come first - pgn-extract resolves a \
             duplicated line to its first occurrence"
        );
    }

    #[test]
    fn a_missing_supplement_falls_back_to_the_bundled_file_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundled, _, cache) = fixture(tmp.path());
        let absent = tmp.path().join("does-not-exist.pgn");

        let choice = resolve_eco_file(&bundled, &absent, &cache);

        assert!(!choice.merged);
        assert_eq!(choice.path, bundled);
        assert!(choice.note.is_some());
    }

    #[test]
    fn the_bundled_file_is_never_modified_by_a_merge() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundled, supplement, cache) = fixture(tmp.path());

        resolve_eco_file(&bundled, &supplement, &cache);

        assert_eq!(std::fs::read_to_string(&bundled).unwrap(), BUNDLED);
        assert_eq!(std::fs::read_to_string(&supplement).unwrap(), SUPPLEMENT);
    }

    #[test]
    fn the_merged_artifact_is_written_outside_the_resource_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundled, supplement, cache) = fixture(tmp.path());

        let choice = resolve_eco_file(&bundled, &supplement, &cache);

        assert!(choice.path.starts_with(&cache));
        assert_ne!(choice.path.parent(), bundled.parent());
    }

    #[test]
    fn an_unchanged_input_pair_is_not_rebuilt_on_the_next_call() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundled, supplement, cache) = fixture(tmp.path());

        let first = resolve_eco_file(&bundled, &supplement, &cache);
        // A sentinel that only survives if the second call reuses the file
        // instead of regenerating it.
        std::fs::write(&first.path, "SENTINEL").unwrap();

        let second = resolve_eco_file(&bundled, &supplement, &cache);

        assert_eq!(first.path, second.path);
        assert_eq!(std::fs::read_to_string(&second.path).unwrap(), "SENTINEL");
    }

    #[test]
    fn a_changed_supplement_forces_a_rebuild() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundled, supplement, cache) = fixture(tmp.path());

        let first = resolve_eco_file(&bundled, &supplement, &cache);
        std::fs::write(&first.path, "SENTINEL").unwrap();

        std::fs::write(
            &supplement,
            format!("{SUPPLEMENT}\n[ECO \"A02\"]\n[Opening \"Bird\"]\n\n1. f4 *\n"),
        )
        .unwrap();
        let second = resolve_eco_file(&bundled, &supplement, &cache);

        let text = std::fs::read_to_string(&second.path).unwrap();
        assert_ne!(text, "SENTINEL", "a changed supplement must rebuild");
        assert!(text.contains("Bird"));
        assert!(text.contains("Polish"), "bundled content still present");
    }

    #[test]
    fn a_changed_bundled_file_forces_a_rebuild() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundled, supplement, cache) = fixture(tmp.path());

        let first = resolve_eco_file(&bundled, &supplement, &cache);
        std::fs::write(&first.path, "SENTINEL").unwrap();

        std::fs::write(
            &bundled,
            format!("{BUNDLED}\n[ECO \"A00\"]\n[Opening \"Grob\"]\n\n1. g4 *\n"),
        )
        .unwrap();
        let second = resolve_eco_file(&bundled, &supplement, &cache);

        let text = std::fs::read_to_string(&second.path).unwrap();
        assert_ne!(text, "SENTINEL");
        assert!(text.contains("Grob"));
    }

    #[test]
    fn an_unwritable_cache_root_falls_back_to_the_bundled_file() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundled, supplement, _) = fixture(tmp.path());
        // A *file* where the cache directory should be: `create_dir_all`
        // cannot succeed here on any platform, which is a portable way to
        // exercise the failure path without needing real permission games.
        let blocked = tmp.path().join("blocked");
        std::fs::write(&blocked, b"not a directory").unwrap();

        let choice = resolve_eco_file(&bundled, &supplement, &blocked);

        assert!(!choice.merged);
        assert_eq!(choice.path, bundled);
        assert!(choice.note.is_some());
    }

    #[test]
    fn a_duplicated_line_resolves_to_the_bundled_copy_by_position() {
        // Mirrors the engine's verified first-occurrence rule at the file
        // level: if the supplement ever did contain a line eco.pgn already
        // has, the bundled copy must appear earlier in the merged file.
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        let bundled = tmp.path().join("eco.pgn");
        let supplement = tmp.path().join("supp.pgn");
        std::fs::write(
            &bundled,
            "[ECO \"A00\"]\n[Opening \"BUNDLED\"]\n\n1. b4 *\n",
        )
        .unwrap();
        std::fs::write(
            &supplement,
            "[ECO \"Z99\"]\n[Opening \"SUPP\"]\n\n1. b4 *\n",
        )
        .unwrap();

        let choice = resolve_eco_file(&bundled, &supplement, &cache);

        let text = std::fs::read_to_string(&choice.path).unwrap();
        assert!(text.find("BUNDLED").unwrap() < text.find("SUPP").unwrap());
    }
}
