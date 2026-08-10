// SPDX-License-Identifier: GPL-3.0-or-later
//! Filtering: [`FilterPlan`], [`TagRule`], [`TagName`], [`TagOp`],
//! [`MoveBounds`], [`SetupPolicy`], [`FenPatternFilter`] (architecture.md
//! §9.2, §13.4; design-02 §1.3 rows 19-26, §1.5.1, §4.1, D-10, D-20).

use serde::{Deserialize, Serialize};
use specta::Type;

/// Filter criteria for a job (architecture.md §9.2 `FilterPlan`; design-02
/// §4.1 `filters`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilterPlan {
    /// Tag/metadata criteria, rendered into `criteria/tags.txt` and passed
    /// as `-t<path>` (design-02 row 19, §1.5.1). Different tags AND
    /// together; multiple entries for the *same* tag OR together — this is
    /// how both "either of these players" and "a date range" are expressed,
    /// and it is the engine's semantics, not something this compiler
    /// re-implements. `React never composes criteria syntax` (design-02
    /// §1.5): a "year range" or "decisive result" UI control is expected to
    /// already have expanded itself into the appropriate list of
    /// [`TagRule`] entries (e.g. two `Result` rules, or two `Date` bound
    /// rules) before this reaches Rust; the compiler's job is to render
    /// each entry safely, not to invent the expansion.
    pub tag_rules: Vec<TagRule>,
    pub move_bounds: Option<MoveBounds>,
    /// `--checkmate` (design-02 row 25).
    pub checkmate_only: bool,
    pub setup_policy: SetupPolicy,
    /// Rendered as a `FENPattern`/`FENPatternI` line inside the *same*
    /// `criteria/tags.txt` file as `tag_rules` (design-02 row 23: "chosen
    /// route" is the criteria file, not the standalone `--fenpattern` flag),
    /// so `-t` is emitted whenever either this or `tag_rules` is non-empty.
    pub fen_pattern: Option<FenPatternFilter>,
    /// Opening-line filters, rendered into `criteria/variations.txt` and
    /// passed as `-v<path>` (design-02 row 21, §1.5.2). Engine defaults
    /// apply (match from move 1, permutations allowed; Decision D-12) — V1
    /// does not expose `-P`/`--vanywhere`.
    pub textual_variations: Vec<String>,
    /// Reserved for V1.1 (design-02 D-20). Must be empty in V1; the
    /// compiler rejects any non-empty value as
    /// `CompileError::UnsupportedOption` rather than attempting to interpret
    /// raw engine arguments. `String` is a placeholder element type — V1.1
    /// will replace this with a real validated-argument type when the
    /// advanced-argument editor is designed.
    pub advanced_args: Vec<String>,
}

/// One tag/metadata criterion line (design-02 §1.5.1 grammar:
/// `<TagName> [<op>] "<value>"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TagRule {
    pub tag: TagName,
    pub op: TagOp,
    pub value: String,
}

/// Recognized tag/pseudo-tag names for criteria rules (design-02 §1.5.1).
///
/// Judgment call / known gap: design-02 states the engine recognizes "the 45
/// predefined tags" but only names a subset by example
/// (`Event, Site, Date, Round, White, Black, Result, WhiteElo, BlackElo,
/// ECO, TimeControl, …`) plus the pseudo-tags `Player`, `Elo`, `EloDiff`
/// (`FEN`/`FENPattern`/`FENPatternI` are handled separately via
/// [`super::FenPatternFilter`], not as a `TagName`, per the "FilterPlan
/// rendering map" table). This enum contains exactly the names design-02
/// verifies by citation — no more. Extending it to the full 45-tag set
/// requires a fresh, source-verified read of `lex.c:121-170`, which was not
/// part of the material available for this task; inventing the remaining
/// ~31 names to fill out the enum would violate the project's
/// never-invent-never-placeholder rule (DECISIONS-LEDGER.md header). A free
/// `String` field was deliberately rejected in favor of this closed enum
/// because design-02 §4.1 explicitly calls `TagRuleDto` "typed: {tag, op,
/// value} closed enums" and because inbound enums must be closed with no
/// `other` catch-all (design-02 §4.3) — so the honest resolution is a
/// smaller-but-real closed enum, not a falsely-complete one or an
/// escape-hatch string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "PascalCase")]
pub enum TagName {
    Event,
    Site,
    Date,
    Round,
    White,
    Black,
    Result,
    WhiteElo,
    BlackElo,
    Eco,
    TimeControl,
    /// Pseudo-tag: matches if *either* color's player satisfies the rule.
    Player,
    /// Pseudo-tag: matches if *either* color's Elo satisfies the rule.
    Elo,
    /// Pseudo-tag: `|WhiteElo - BlackElo|`.
    EloDiff,
}

impl TagName {
    /// The exact spelling the engine's tag-file lexer expects. The lexer is
    /// case-insensitive (design-02 §1.5.1), but the compiler always emits
    /// canonical spelling.
    pub fn as_engine_str(self) -> &'static str {
        match self {
            TagName::Event => "Event",
            TagName::Site => "Site",
            TagName::Date => "Date",
            TagName::Round => "Round",
            TagName::White => "White",
            TagName::Black => "Black",
            TagName::Result => "Result",
            TagName::WhiteElo => "WhiteElo",
            TagName::BlackElo => "BlackElo",
            TagName::Eco => "ECO",
            TagName::TimeControl => "TimeControl",
            TagName::Player => "Player",
            TagName::Elo => "Elo",
            TagName::EloDiff => "EloDiff",
        }
    }
}

/// A tag-rule comparison operator, or the absence of one (design-02 §1.5.1:
/// `op ∈ { < <= <> > >= = =~ }`, no op = prefix match).
///
/// UI wording for [`TagOp::Prefix`] must say "starts with", not "equals" —
/// `strncmp`-based prefix matching means `Tal` matches `Talbot` (design-02
/// §1.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TagOp {
    /// No operator: prefix ("starts with") match.
    Prefix,
    Lt,
    Le,
    /// `<>`: not-equal.
    Ne,
    Gt,
    Ge,
    Eq,
    /// `=~`: POSIX basic regular expression, matched anywhere in the value.
    Regex,
}

impl TagOp {
    /// The literal grammar token, or `None` for [`TagOp::Prefix`] (which has
    /// no token at all — it is the absence of an operator).
    pub fn as_engine_token(self) -> Option<&'static str> {
        match self {
            TagOp::Prefix => None,
            TagOp::Lt => Some("<"),
            TagOp::Le => Some("<="),
            TagOp::Ne => Some("<>"),
            TagOp::Gt => Some(">"),
            TagOp::Ge => Some(">="),
            TagOp::Eq => Some("="),
            TagOp::Regex => Some("=~"),
        }
    }
}

/// Move-count filter bounds, in whole moves (not ply) — design-02 row 24
/// scopes V1 to `--maxmoves`/`--minmoves` only, not the ply variants.
///
/// This type only *carries* the bounds; the safety-critical rule that
/// `--maxmoves` must be emitted before `--minmoves` lives in
/// `engine::command_compiler` (design-02 §0 finding 3, D-007 V-3, canonical
/// order rule O-6b), because it is a property of argv construction, not of
/// this data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveBounds {
    pub min: Option<u32>,
    pub max: Option<u32>,
}

/// SetUp/FEN starting-position policy (design-02 row 26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SetupPolicy {
    /// Neither flag: games may start from the standard position or a
    /// `SetUp`/`FEN` tag.
    Any,
    /// `--nosetuptags`: only games starting from the standard position.
    StandardStartOnly,
    /// `--onlysetuptags`: only games with a `SetUp`/`FEN` tag.
    SetupOnly,
}

/// A FEN-pattern position filter (design-02 row 23, §1.5.1 rendering map).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FenPatternFilter {
    pub pattern: String,
    /// `false` → rendered as `FENPattern "<pattern>"`; `true` → rendered as
    /// `FENPatternI "<pattern>"` (design-02 §1.5.1 rendering-map row: "FEN
    /// pattern | `FENPattern \"<pattern>\"` (or `FENPatternI` when
    /// side-agnostic requested)").
    pub both_sides: bool,
}
