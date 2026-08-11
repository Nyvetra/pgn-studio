// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-end proof that the generated ECO supplement enriches
//! classification **without ever changing a classification the bundled
//! `eco.pgn` already provides** (see `filesystem::eco_merge` and
//! `resources/eco-supplement/SOURCE.json`).
//!
//! Mirrors `phase4_integration.rs`'s pattern: the real checksum-verified
//! bundled sidecar, the real resource files, no mocks. The central test
//! turns every one of `eco.pgn`'s own lines into a game, classifies the
//! whole set twice - once with `eco.pgn` alone, once with the merged file -
//! and asserts the two results are identical entry for entry. That is the
//! guarantee the whole design rests on, so it is verified against the
//! engine rather than argued from the file's construction.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use pgn_studio_lib::engine::sidecar::{startup_check, SidecarLocation};
use pgn_studio_lib::engine::EngineExecutable;
use pgn_studio_lib::filesystem::eco_merge;

fn resources_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")
}

fn bundled_eco_path() -> PathBuf {
    resources_dir().join("pgn-extract").join("eco.pgn")
}

fn supplement_path() -> PathBuf {
    resources_dir()
        .join("eco-supplement")
        .join("eco-supplement.pgn")
}

async fn resolve_engine() -> EngineExecutable {
    let result = startup_check(&SidecarLocation::dev_default()).await.expect(
        "the real, checksum-pinned sidecar must resolve and pass its startup self-test - if \
         this fails, `src-tauri/binaries/pgn-extract-x86_64-pc-windows-msvc.exe` is missing \
         or does not match the pinned checksum",
    );
    result.engine
}

/// One parsed ECO record: the tag values plus the move text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EcoEntry {
    eco: String,
    opening: String,
    variation: String,
    moves: String,
}

/// Minimal parser for the ECO-file dialect of PGN (a brace-comment header,
/// then records of `[ECO]`/`[Opening]`/`[Variation]` tags followed by a
/// move sequence terminated by `*`).
fn parse_eco_file(text: &str) -> Vec<EcoEntry> {
    // Drop the leading brace comment, if present.
    let body = match (text.find('{'), text.find('}')) {
        (Some(open), Some(close)) if open < close && text[..open].trim().is_empty() => {
            &text[close + 1..]
        }
        _ => text,
    };

    let mut entries = Vec::new();
    let mut current: Option<EcoEntry> = None;
    let mut moves = String::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("[ECO \"") {
            // A new record starts; flush the previous one.
            if let Some(mut entry) = current.take() {
                entry.moves = moves.trim().to_string();
                if !entry.moves.is_empty() {
                    entries.push(entry);
                }
            }
            moves.clear();
            current = Some(EcoEntry {
                eco: rest.trim_end_matches("\"]").to_string(),
                opening: String::new(),
                variation: String::new(),
                moves: String::new(),
            });
        } else if let Some(rest) = trimmed.strip_prefix("[Opening \"") {
            if let Some(entry) = current.as_mut() {
                entry.opening = rest.trim_end_matches("\"]").to_string();
            }
        } else if let Some(rest) = trimmed.strip_prefix("[Variation \"") {
            if let Some(entry) = current.as_mut() {
                entry.variation = rest.trim_end_matches("\"]").to_string();
            }
        } else if trimmed.starts_with('[') || trimmed.is_empty() {
            continue;
        } else if current.is_some() {
            moves.push(' ');
            moves.push_str(trimmed.trim_end_matches('*').trim());
        }
    }
    if let Some(mut entry) = current.take() {
        entry.moves = moves.trim().to_string();
        if !entry.moves.is_empty() {
            entries.push(entry);
        }
    }
    entries
}

/// Writes one game per entry, tagged `IDX<n>` so results can be correlated
/// back to the line they came from.
fn write_probe_games(path: &Path, entries: &[EcoEntry]) {
    let mut out = String::new();
    for (i, entry) in entries.iter().enumerate() {
        out.push_str(&format!(
            "[Event \"IDX{i}\"]\n[Site \"?\"]\n[Date \"????.??.??\"]\n[Round \"?\"]\n\
             [White \"?\"]\n[Black \"?\"]\n[Result \"*\"]\n\n{} *\n\n",
            entry.moves
        ));
    }
    std::fs::write(path, out).unwrap();
}

/// Runs the real engine over `games` with `-e<eco_file>` and returns the
/// resulting tags keyed by probe index.
fn classify(
    engine: &EngineExecutable,
    eco_file: &Path,
    games: &Path,
    out: &Path,
) -> HashMap<usize, (String, String, String)> {
    // The attached `-e<path>` form is mandatory (DECISIONS-LEDGER.md D-007
    // V-4: the separated `-e <path>` form silently fails).
    let mut eco_arg = std::ffi::OsString::from("-e");
    eco_arg.push(eco_file);

    let status = Command::new(engine.path())
        .arg(eco_arg)
        .arg("-o")
        .arg(out)
        .arg(games)
        .status()
        .expect("the sidecar must be spawnable");
    assert!(status.success(), "engine exited with {status}");

    let text = std::fs::read_to_string(out).unwrap();
    let mut results = HashMap::new();
    let mut idx: Option<usize> = None;
    let (mut eco, mut opening, mut variation) = (String::new(), String::new(), String::new());

    let flush = |results: &mut HashMap<usize, (String, String, String)>,
                 idx: &mut Option<usize>,
                 eco: &mut String,
                 opening: &mut String,
                 variation: &mut String| {
        if let Some(i) = idx.take() {
            results.insert(
                i,
                (
                    std::mem::take(eco),
                    std::mem::take(opening),
                    std::mem::take(variation),
                ),
            );
        }
    };

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("[Event \"IDX") {
            flush(
                &mut results,
                &mut idx,
                &mut eco,
                &mut opening,
                &mut variation,
            );
            idx = rest.trim_end_matches("\"]").parse::<usize>().ok();
        } else if let Some(rest) = trimmed.strip_prefix("[ECO \"") {
            eco = rest.trim_end_matches("\"]").to_string();
        } else if let Some(rest) = trimmed.strip_prefix("[Opening \"") {
            opening = rest.trim_end_matches("\"]").to_string();
        } else if let Some(rest) = trimmed.strip_prefix("[Variation \"") {
            variation = rest.trim_end_matches("\"]").to_string();
        }
    }
    flush(
        &mut results,
        &mut idx,
        &mut eco,
        &mut opening,
        &mut variation,
    );
    results
}

#[tokio::test]
async fn the_merged_file_never_changes_a_classification_eco_pgn_already_provides() {
    let engine = resolve_engine().await;
    let tmp = tempfile::tempdir().unwrap();

    let bundled = bundled_eco_path();
    let choice = eco_merge::resolve_eco_file(&bundled, &supplement_path(), tmp.path());
    assert!(
        choice.merged,
        "the supplement resource must be present and mergeable: {:?}",
        choice.note
    );

    let bundled_entries = parse_eco_file(&std::fs::read_to_string(&bundled).unwrap());
    assert!(
        bundled_entries.len() > 1_900,
        "sanity: expected ~2014 bundled lines, parsed {}",
        bundled_entries.len()
    );

    let games = tmp.path().join("probe-games.pgn");
    write_probe_games(&games, &bundled_entries);

    let before = classify(
        &engine,
        &bundled,
        &games,
        &tmp.path().join("out-bundled.pgn"),
    );
    let after = classify(
        &engine,
        &choice.path,
        &games,
        &tmp.path().join("out-merged.pgn"),
    );

    assert_eq!(
        before.len(),
        bundled_entries.len(),
        "every probe game must come back classified"
    );

    let mut changed = Vec::new();
    for (idx, tags_before) in &before {
        let tags_after = after
            .get(idx)
            .unwrap_or_else(|| panic!("probe IDX{idx} missing from the merged run"));
        if tags_before != tags_after {
            changed.push((*idx, tags_before.clone(), tags_after.clone()));
        }
    }
    changed.sort_by_key(|(i, _, _)| *i);

    assert!(
        changed.is_empty(),
        "the supplement must never override an existing classification, but {} changed; \
         first few: {:?}",
        changed.len(),
        changed.iter().take(5).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn the_supplement_adds_classifications_the_bundled_file_alone_does_not_produce() {
    let engine = resolve_engine().await;
    let tmp = tempfile::tempdir().unwrap();

    let bundled = bundled_eco_path();
    let choice = eco_merge::resolve_eco_file(&bundled, &supplement_path(), tmp.path());
    assert!(choice.merged, "note: {:?}", choice.note);

    // Probe with the supplement's own lines: each should now resolve to a
    // more specific opening than eco.pgn alone can give.
    let supplement_entries = parse_eco_file(&std::fs::read_to_string(supplement_path()).unwrap());
    assert!(
        supplement_entries.len() > 10_000,
        "sanity: expected ~10642 supplement lines, parsed {}",
        supplement_entries.len()
    );

    // A representative slice keeps this test fast while still spanning all
    // five ECO volumes (entries are generated in ECO-code order).
    let sample: Vec<_> = supplement_entries.iter().step_by(37).cloned().collect();
    let games = tmp.path().join("probe-games.pgn");
    write_probe_games(&games, &sample);

    let before = classify(
        &engine,
        &bundled,
        &games,
        &tmp.path().join("out-bundled.pgn"),
    );
    let after = classify(
        &engine,
        &choice.path,
        &games,
        &tmp.path().join("out-merged.pgn"),
    );

    let improved = sample
        .iter()
        .enumerate()
        .filter(|(i, _)| before.get(i) != after.get(i))
        .count();

    assert!(
        improved * 10 > sample.len() * 9,
        "the supplement should refine the overwhelming majority of its own lines, but only \
         {improved} of {} changed",
        sample.len()
    );

    // And the refined name must be the one the supplement actually declares.
    for (i, entry) in sample.iter().enumerate() {
        if let Some((eco, _, _)) = after.get(&i) {
            if before.get(&i) != after.get(&i) {
                assert_eq!(
                    eco, &entry.eco,
                    "a refined line must carry the supplement's own ECO code"
                );
            }
        }
    }
}

#[tokio::test]
async fn the_bundled_eco_pgn_resource_is_not_modified_by_building_the_merged_file() {
    let tmp = tempfile::tempdir().unwrap();
    let bundled = bundled_eco_path();
    let before = std::fs::read(&bundled).unwrap();

    let choice = eco_merge::resolve_eco_file(&bundled, &supplement_path(), tmp.path());
    assert!(choice.merged, "note: {:?}", choice.note);

    assert_eq!(
        before,
        std::fs::read(&bundled).unwrap(),
        "eco.pgn is a third-party file redistributed byte-for-byte unmodified and checksummed \
         in resources/pgn-extract/SOURCE.json - nothing in this app may write to it"
    );
    assert!(
        !choice.path.starts_with(resources_dir()),
        "the merged artifact must live in the app cache, never in resources/"
    );
}
