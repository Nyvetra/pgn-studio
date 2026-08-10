// SPDX-License-Identifier: GPL-3.0-or-later
//! Post-run advisory scan of a published duplicates-audit file for
//! annotation markers (architecture.md §24 Phase 3 exit criterion
//! "annotated-duplicate warnings"; §27's named risk "Duplicate copies
//! contain different annotations → useful information could be
//! suppressed").
//!
//! This is a PGN-Studio-owned heuristic, not a re-parse of the whole
//! collection and not a claim of authoritative PGN parsing — the bundled
//! engine, not this module, is the sole authority on move/game structure
//! (architecture.md §29). It answers one narrow question as cheaply as
//! possible: does the *audit file* (the later duplicate copies the engine
//! diverted out of the main output under `-d`) contain anything — a
//! comment, a NAG, or a variation — that the kept first copy might not
//! have? It never inspects the kept copy itself, and the warning text it
//! feeds ([`crate::errors::annotated_duplicates_suppressed`]) never claims
//! to know whether the kept copy is "better" or "worse"
//! (architecture.md §3.3, §10.7; ADR-009 reserves any such judgment for a
//! future review/index layer).
//!
//! Streamed, single pass, bounded (architecture.md §19.2): never loads the
//! audit file into memory, and stops scanning after a fixed number of
//! games so a pathologically large audit file cannot block a worker thread
//! indefinitely. This is a modest, best-effort advisory, not a guarantee to
//! fully examine every audit file however large.

use std::io::{self, BufRead, BufReader};
use std::path::Path;

/// Hard cap on how many games one scan will examine. Chosen to sit far
/// above any realistic duplicate-audit file (duplicates are ordinarily a
/// small fraction of a collection) while still bounding worst-case scan
/// time on an adversarial/pathological input.
const ANNOTATION_SCAN_GAME_CAP: u64 = 200_000;

/// How many human-readable example identifiers the summary retains, so the
/// resulting warning message stays a bounded size regardless of how many
/// games actually had annotations.
const ANNOTATION_SCAN_EXAMPLE_CAP: usize = 5;

/// The result of scanning one duplicates-audit PGN file for annotation
/// markers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnnotationScanSummary {
    /// How many games the scan actually looked at (bounded by the game
    /// cap).
    pub games_scanned: u64,
    /// How many of those games contained at least one comment (`{...}`),
    /// NAG (`$n`), or variation (`(...)`) marker outside their tag section.
    pub games_with_annotations: u64,
    /// Up to [`ANNOTATION_SCAN_EXAMPLE_CAP`] best-effort "White vs Black"
    /// identifiers (falling back to a 1-based ordinal when a game's own
    /// White/Black tags cannot be read this cheaply), for games that had
    /// at least one marker.
    pub examples: Vec<String>,
    /// True if the scan stopped before reaching the end of the file
    /// because the game cap was reached — the reported counts are then a
    /// lower bound, not an exact total.
    pub truncated: bool,
}

/// A line is treated as a tag-pair line (and excluded from marker
/// scanning) when it has the `[Name "Value"]` shape. This keeps a
/// parenthesis or brace *inside a header value itself* (e.g. a `Site`
/// value like `"Somewhere (playoff)"`) from being misread as a movetext
/// annotation. This is a cheap shape check, not a real PGN tag parser —
/// consistent with this module's "modest heuristic" scope.
fn is_tag_line(trimmed: &str) -> bool {
    trimmed.starts_with('[') && trimmed.ends_with(']')
}

/// True if `trimmed` (already known not to be a tag line) contains a PGN
/// comment start (`{`), a variation start (`(`), or a NAG (`$` immediately
/// followed by an ASCII digit).
fn line_has_annotation_marker(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' | b'(' => return true,
            b'$' if bytes.get(i + 1).is_some_and(u8::is_ascii_digit) => return true,
            _ => {}
        }
    }
    false
}

/// Extracts a tag's quoted value from the text immediately following
/// `[TagName "` (i.e. `rest` starts right after the opening quote). Stops
/// at the first `"`; does not understand backslash-escaped quotes (this is
/// a display-only heuristic feeding a warning message, not the
/// authoritative parser).
fn extract_quoted_value(rest: &str) -> Option<String> {
    rest.split('"').next().map(str::to_string)
}

/// Finalizes whatever game is currently open, folding it into `summary` if
/// one actually was (a fresh scan, or a stray file with content before its
/// first `[Event "` line, has none). `example_cap` bounds how many labeled
/// examples accumulate — threaded as a parameter (rather than reading the
/// module constant directly) so [`scan_with_bounds`]'s test-only custom
/// bounds are actually honored, not just the game cap.
fn finalize_game(
    summary: &mut AnnotationScanSummary,
    game_open: bool,
    has_marker: bool,
    white: &Option<String>,
    black: &Option<String>,
    example_cap: usize,
) {
    if !game_open {
        return;
    }
    summary.games_scanned += 1;
    if has_marker {
        summary.games_with_annotations += 1;
        if summary.examples.len() < example_cap {
            let label = match (white, black) {
                (Some(w), Some(b)) => format!("{w} vs {b}"),
                _ => format!("game {}", summary.games_scanned),
            };
            summary.examples.push(label);
        }
    }
}

/// Scans `path` (a published duplicates-audit PGN, per architecture.md
/// §11.6's `<base>.duplicates.pgn`) for games that contain at least one
/// annotation marker, streaming line-by-line with a bounded 64 KiB buffer
/// (matching [`super::count_games_in_file`]'s approach) — never loading the
/// file into memory at once.
pub fn scan_duplicate_audit_for_annotations(path: &Path) -> io::Result<AnnotationScanSummary> {
    scan_with_bounds(path, ANNOTATION_SCAN_GAME_CAP, ANNOTATION_SCAN_EXAMPLE_CAP)
}

/// The real implementation, parameterized over its two bounds so tests can
/// exercise the cap/truncation logic cheaply (a handful of games) instead
/// of needing a multi-hundred-thousand-game fixture. Production code only
/// ever calls this through [`scan_duplicate_audit_for_annotations`], which
/// pins both bounds to their real constants.
fn scan_with_bounds(
    path: &Path,
    game_cap: u64,
    example_cap: usize,
) -> io::Result<AnnotationScanSummary> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut raw_line = String::new();

    let mut summary = AnnotationScanSummary::default();
    let mut game_open = false;
    let mut has_marker = false;
    let mut white: Option<String> = None;
    let mut black: Option<String> = None;

    loop {
        if summary.games_scanned >= game_cap {
            summary.truncated = true;
            break;
        }
        raw_line.clear();
        let bytes_read = reader.read_line(&mut raw_line)?;
        if bytes_read == 0 {
            break; // natural EOF, not the cap — `truncated` stays false
        }
        let trimmed = raw_line.trim_end_matches(['\r', '\n']);

        if trimmed.starts_with("[Event \"") {
            finalize_game(
                &mut summary,
                game_open,
                has_marker,
                &white,
                &black,
                example_cap,
            );
            game_open = true;
            has_marker = false;
            white = None;
            black = None;
            continue;
        }
        if !game_open {
            continue; // stray content before the first recognized game
        }
        if let Some(rest) = trimmed.strip_prefix("[White \"") {
            white = extract_quoted_value(rest);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("[Black \"") {
            black = extract_quoted_value(rest);
            continue;
        }
        if is_tag_line(trimmed) {
            continue;
        }
        if !has_marker && line_has_annotation_marker(trimmed) {
            has_marker = true;
        }
    }
    // Only finalize the in-progress game on a genuine EOF — if the loop
    // broke because the cap was hit, the "current" game was cut off
    // mid-read and must not be counted as a fully-scanned game (it would
    // otherwise silently push `games_scanned` one past `game_cap`).
    if !summary.truncated {
        finalize_game(
            &mut summary,
            game_open,
            has_marker,
            &white,
            &black,
            example_cap,
        );
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_temp(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.pgn");
        std::fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn empty_file_has_no_annotated_games() {
        let (_dir, path) = write_temp("");
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_scanned, 0);
        assert_eq!(summary.games_with_annotations, 0);
        assert!(summary.examples.is_empty());
        assert!(!summary.truncated);
    }

    #[test]
    fn plain_game_with_no_annotations_is_not_flagged() {
        let (_dir, path) = write_temp(
            "[Event \"E\"]\n[White \"A\"]\n[Black \"B\"]\n[Result \"1-0\"]\n\n1. e4 e5 1-0\n",
        );
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_scanned, 1);
        assert_eq!(summary.games_with_annotations, 0);
        assert!(summary.examples.is_empty());
    }

    #[test]
    fn detects_comment_marker() {
        let (_dir, path) = write_temp(
            "[Event \"E\"]\n[White \"A\"]\n[Black \"B\"]\n[Result \"1-0\"]\n\n\
             1. e4 {a good move} e5 1-0\n",
        );
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_with_annotations, 1);
        assert_eq!(summary.examples, vec!["A vs B".to_string()]);
    }

    #[test]
    fn detects_nag_marker() {
        let (_dir, path) =
            write_temp("[Event \"E\"]\n[White \"A\"]\n[Black \"B\"]\n\n1. e4 $1 e5 1-0\n");
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_with_annotations, 1);
    }

    #[test]
    fn dollar_not_followed_by_a_digit_is_not_a_nag_marker() {
        let (_dir, path) = write_temp(
            "[Event \"E\"]\n[White \"A\"]\n[Black \"B\"]\n\n1. e4 e5 $x 2. Nf3 Nc6 1-0\n",
        );
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_with_annotations, 0);
    }

    #[test]
    fn detects_variation_marker() {
        let (_dir, path) = write_temp(
            "[Event \"E\"]\n[White \"A\"]\n[Black \"B\"]\n\n1. e4 e5 (1... c5) 2. Nf3 1-0\n",
        );
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_with_annotations, 1);
    }

    #[test]
    fn parenthesis_in_a_tag_value_is_not_a_false_positive() {
        let (_dir, path) = write_temp(
            "[Event \"Somewhere (playoff)\"]\n[White \"A\"]\n[Black \"B\"]\n\n1. e4 e5 1-0\n",
        );
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(
            summary.games_with_annotations, 0,
            "a '(' inside a tag VALUE must not be misread as a variation marker"
        );
    }

    #[test]
    fn brace_in_a_tag_value_is_not_a_false_positive() {
        let (_dir, path) =
            write_temp("[Event \"{weird} name\"]\n[White \"A\"]\n[Black \"B\"]\n\n1. e4 e5 1-0\n");
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_with_annotations, 0);
    }

    #[test]
    fn multiple_games_only_counts_the_ones_with_markers() {
        let (_dir, path) = write_temp(
            "[Event \"E\"]\n[White \"A\"]\n[Black \"B\"]\n\n1. e4 e5 1-0\n\n\
             [Event \"E\"]\n[White \"C\"]\n[Black \"D\"]\n\n1. e4 {x} e5 1-0\n",
        );
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_scanned, 2);
        assert_eq!(summary.games_with_annotations, 1);
        assert_eq!(summary.examples, vec!["C vs D".to_string()]);
    }

    #[test]
    fn falls_back_to_ordinal_when_white_or_black_is_missing() {
        let (_dir, path) = write_temp("[Event \"E\"]\n\n1. e4 {x} e5 1-0\n");
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.examples, vec!["game 1".to_string()]);
    }

    #[test]
    fn examples_are_capped_but_the_total_count_is_not() {
        let mut content = String::new();
        for i in 0..(ANNOTATION_SCAN_EXAMPLE_CAP + 3) {
            content.push_str(&format!(
                "[Event \"E\"]\n[White \"W{i}\"]\n[Black \"B{i}\"]\n\n1. e4 {{x}} e5 1-0\n\n"
            ));
        }
        let (_dir, path) = write_temp(&content);
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(
            summary.games_with_annotations,
            (ANNOTATION_SCAN_EXAMPLE_CAP + 3) as u64
        );
        assert_eq!(summary.examples.len(), ANNOTATION_SCAN_EXAMPLE_CAP);
    }

    #[test]
    fn stops_scanning_at_the_game_cap_and_reports_truncated() {
        let mut content = String::new();
        for i in 0..5 {
            content.push_str(&format!(
                "[Event \"E\"]\n[White \"W{i}\"]\n[Black \"B{i}\"]\n\n1. e4 e5 1-0\n\n"
            ));
        }
        let (_dir, path) = write_temp(&content);
        let summary = scan_with_bounds(&path, 3, 10).unwrap();
        assert_eq!(summary.games_scanned, 3);
        assert!(summary.truncated);
    }

    #[test]
    fn does_not_report_truncated_when_the_file_ends_exactly_at_the_cap() {
        let mut content = String::new();
        for i in 0..3 {
            content.push_str(&format!(
                "[Event \"E\"]\n[White \"W{i}\"]\n[Black \"B{i}\"]\n\n1. e4 e5 1-0\n\n"
            ));
        }
        let (_dir, path) = write_temp(&content);
        let summary = scan_with_bounds(&path, 3, 10).unwrap();
        assert_eq!(summary.games_scanned, 3);
        assert!(!summary.truncated);
    }

    #[test]
    fn nonexistent_file_returns_an_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = scan_duplicate_audit_for_annotations(&dir.path().join("missing.pgn"));
        assert!(result.is_err());
    }

    #[test]
    fn crlf_line_endings_are_handled_like_lf() {
        let (_dir, path) = write_temp(
            "[Event \"E\"]\r\n[White \"A\"]\r\n[Black \"B\"]\r\n\r\n1. e4 {x} e5 1-0\r\n",
        );
        let summary = scan_duplicate_audit_for_annotations(&path).unwrap();
        assert_eq!(summary.games_scanned, 1);
        assert_eq!(summary.games_with_annotations, 1);
        assert_eq!(summary.examples, vec!["A vs B".to_string()]);
    }
}
