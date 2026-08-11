// SPDX-License-Identifier: GPL-3.0-or-later
//! Filesystem safety (architecture.md §7.1, §11; design-02 §3): source
//! immutability, file-identity-based aliasing detection, path validation,
//! per-job workspaces, atomic output publication, and conflict-policy
//! resolution. Every direct filesystem operation the rest of the app needs
//! lives behind this module - the rest of the app (and definitely the
//! frontend) never touches paths directly.

pub mod duplicate_audit;
pub mod eco_merge;
pub mod export;
pub mod folder_scan;
pub mod identity;
pub mod manifest;
pub mod publish;
pub mod validate;
pub mod workspace;

mod platform;

use std::io::{BufRead, BufReader};
use std::path::Path;

/// Cheap streaming count of games in a published PGN artifact, by counting
/// lines that start with `[Event "` (design-02 §2.4: "postflight streaming
/// count of lines starting `[Event \"` in the published artifacts (cheap
/// single pass, 64 KiB buffer; correct because V1 never emits
/// `--notags`/`-7`)"). Never loads the file into memory at once
/// (architecture.md §19.2).
pub fn count_games_in_file(path: &Path) -> std::io::Result<u64> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut count: u64 = 0;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }
        if line.starts_with("[Event \"") {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_event_tag_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("games.pgn");
        std::fs::write(
            &path,
            "[Event \"A\"]\n[Site \"x\"]\n\n1. e4 1-0\n\n[Event \"B\"]\n\n1. d4 1-0\n",
        )
        .unwrap();
        assert_eq!(count_games_in_file(&path).unwrap(), 2);
    }

    #[test]
    fn counts_zero_for_empty_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("empty.pgn");
        std::fs::write(&path, b"").unwrap();
        assert_eq!(count_games_in_file(&path).unwrap(), 0);
    }
}
