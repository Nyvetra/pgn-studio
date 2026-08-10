// SPDX-License-Identifier: GPL-3.0-or-later
//! [`RuntimeOptions`] (design-02 §4.1 `runtime`, §1.3 row 5, §2.4).

use serde::{Deserialize, Serialize};

/// Execution-engine tuning that is not a filter or cleanup choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeOptions {
    /// `-Z`: use a disk-backed duplicate table instead of in-memory hashing
    /// (design-02 row 5). Intended for very large collections; irrelevant to
    /// correctness, only to memory footprint.
    pub use_external_duplicate_table: bool,
    /// Whether the orchestrator should attempt a postflight `[Event "`
    /// streaming count of published artifacts to populate
    /// `ProcessingMetrics::output_games`/`duplicate_games` (design-02 §2.4).
    /// This does not change compiled argv at all — the engine has no flag
    /// for it — but the compiler still consults it when computing
    /// `MetricsPlan` (`engine::command_compiler`) so the plan reflects
    /// what the caller actually asked to have measured.
    pub count_output_games: bool,
}
