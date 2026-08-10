// SPDX-License-Identifier: GPL-3.0-or-later
//! Criteria file generation (architecture.md §11.3, §13.4; design-02 §1.5).
//!
//! Renders [`crate::domain::FilterPlan`]'s tag rules / FEN pattern into the
//! `criteria/tags.txt` content passed as `-t<path>`, and its textual
//! variations into the `criteria/variations.txt` content passed as
//! `-v<path>`. Pure string building only — no filesystem access; the
//! orchestrator (Phase 1b) writes the returned `content` to disk.
//!
//! **Binding encoding rule:** UTF-8, no BOM, LF line endings, trailing LF.
//! A UTF-8 BOM would prepend `EF BB BF` to the first tag name, and because a
//! malformed criteria line silently terminates parsing entirely
//! (`taglines.c:239-242`, design-02 §1.5), a BOM would make the engine
//! silently run the job **unfiltered** — a correctness hazard, not a style
//! preference. This module satisfies the rule structurally: every function
//! here builds output exclusively via `String::push`/`push_str`, so there is
//! no code path that could ever emit a BOM codepoint or a `\r` byte; the
//! test module still asserts this explicitly per the task's requirement to
//! test it, not just architect it away.

use crate::domain::{FenPatternFilter, TagName, TagOp, TagRule};

use super::command_compiler::CompileError;

/// A rendered criteria file: exact bytes to write, plus their SHA-256 for
/// the manifest (design-02 §1.1 `GeneratedCriteriaFile::sha256`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedCriteriaFile {
    pub content: String,
    pub sha256: String,
}

fn sha256_hex(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Joins already-validated criteria lines into final file content: LF after
/// every line (including the last), no BOM, UTF-8 (guaranteed by `String`).
fn finalize(lines: Vec<String>) -> RenderedCriteriaFile {
    let mut content = String::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
    for line in lines {
        content.push_str(&line);
        content.push('\n');
    }
    let sha256 = sha256_hex(&content);
    RenderedCriteriaFile { content, sha256 }
}

/// Escapes a value for placement inside `"…"` in a criteria file
/// (design-02 §1.5.1, `lex.c:356-383`): `\` → `\\`, `"` → `\"`, no other
/// escapes exist. Single-pass over the *original* string's characters, so
/// there is no risk of the classic double-replacement bug that
/// find-and-replace-twice approaches have.
fn escape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

fn ensure_representable(field: &str, value: &str) -> Result<(), CompileError> {
    if value.contains(['\r', '\n', '\0']) {
        return Err(CompileError::InvalidSpec {
            field: field.to_string(),
            reason: "value contains a carriage return, line feed, or NUL byte; this cannot \
                     be represented as a single criteria-file line and must not be silently \
                     truncated or stripped"
                .to_string(),
        });
    }
    Ok(())
}

fn is_bare_year(s: &str) -> bool {
    s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit())
}

/// Recognizes a full `YYYY.MM.DD` date with plausible (not calendar-exact)
/// month/day ranges.
fn is_full_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'.' || b[7] != b'.' {
        return false;
    }
    let all_digits = |slice: &[u8]| slice.iter().all(u8::is_ascii_digit);
    if !all_digits(&b[0..4]) || !all_digits(&b[5..7]) || !all_digits(&b[8..10]) {
        return false;
    }
    // No unwrap: the all_digits checks above guarantee these slices parse,
    // but `compile` must never panic even on a hypothetical future bug here.
    let month: u32 = match s[5..7].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let day: u32 = match s[8..10].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Renders a `Date` tag rule's value, enforcing design-02's full-date rule:
/// "`Date` comparisons encode `YYYY*10000 + MM*100 + DD` with month/day
/// **defaulting to 1**... the compiler renders ranges as full dates... a
/// naive rendering like `Date <= "1999"` would exclude everything after
/// Jan 1, 1999." A bare 4-digit year is only unambiguous when it names a
/// lower bound (`>=`/`>`, expand to `.01.01`) or an upper bound
/// (`<=`/`<`, expand to `.12.31`); for any other operator a bare year is
/// rejected outright rather than guessed at, since "what does an exact
/// match against a bare year mean" has no single correct answer.
fn render_date_value(op: TagOp, value: &str) -> Result<String, CompileError> {
    if is_full_date(value) {
        return Ok(value.to_string());
    }
    if is_bare_year(value) {
        return match op {
            TagOp::Ge | TagOp::Gt => Ok(format!("{value}.01.01")),
            TagOp::Le | TagOp::Lt => Ok(format!("{value}.12.31")),
            _ => Err(CompileError::InvalidSpec {
                field: "filters.tagRules[].value (Date)".to_string(),
                reason: format!(
                    "a bare year (\"{value}\") is only unambiguous as a lower bound (>=, >) \
                     or upper bound (<=, <), where it is expanded to a full date; the engine \
                     defaults a missing month/day to 01/01, so any other comparison against a \
                     bare year would silently mean \"January 1st\". Supply a full \
                     \"YYYY.MM.DD\" date instead."
                ),
            }),
        };
    }
    Err(CompileError::InvalidSpec {
        field: "filters.tagRules[].value (Date)".to_string(),
        reason: format!(
            "\"{value}\" is not a valid Date value; use \"YYYY.MM.DD\", or (for >=, >, <=, < \
             only) a bare \"YYYY\""
        ),
    })
}

/// The four pseudo/real tags whose values the engine compares **numerically**
/// (`lists.c`'s Elo-family comparison path), where all six operators work
/// correctly. Every other tag is compared as **text**.
///
/// **Task finding, generalizing DECISIONS-LEDGER.md D-010 (evidence below).**
/// D-010 empirically verified that five of the engine's six relational
/// operators (`<`, `<=`, `>`, `>=`, `=`) silently match **nothing** against
/// `ECO`, and recorded this as an ECO-specific fact ("ECO operator support").
/// Fresh empirical testing for this phase (real pinned sidecar, same
/// methodology: purpose-built fixtures, exact-count assertions) proves this
/// is **not** ECO-specific — it is a general property of every *non-numeric*
/// tag:
///
/// ```text
/// fixture: 6 games, incl. one with White = "Tal, Mikhail", Result "1-0" x3
/// White = "Tal, Mikhail"        -> 0 games matched out of 6   (silently wrong)
/// White > "M"                   -> 0 games matched out of 6   (silently wrong)
/// Site  = "Fixture Lab"         -> 0 games matched out of 6   (ALL 6 games have this exact value!)
/// Result = "1-0"                -> 0 games matched out of 6   (3 games actually have Result "1-0")
/// Result "1-0"        (no op)   -> 3 games matched out of 6   CORRECT
/// Result <> "1-0"               -> 3 games matched out of 6   CORRECT (0-1, 1/2-1/2, *)
/// White <> "Tal, Mikhail"       -> 5 games matched out of 6   CORRECT
/// WhiteElo = "2500"             -> matches correctly (numeric tag - unaffected)
/// WhiteElo >= "2600"            -> matches correctly (numeric tag - unaffected)
/// ```
///
/// This mattered in practice: before this fix, `src/state/filterMapping.ts`
/// compiled every Result checkbox (White wins / Black wins / Draw / Other /
/// Decisive-only) to `Result = "<value>"`, which — per the table above —
/// silently produced a filter that matched **zero games, always**, for
/// every job that used any Result filter. That call site is fixed alongside
/// this generalized guard (now emits the no-op/prefix form instead, which is
/// exactly equivalent for these four mutually non-prefixing literal values).
fn tag_is_numeric(tag: TagName) -> bool {
    matches!(
        tag,
        TagName::WhiteElo | TagName::BlackElo | TagName::Elo | TagName::EloDiff
    )
}

/// For any non-numeric tag (see `tag_is_numeric`) other than `Date` — which
/// has its own dedicated date-arithmetic comparison path, handled entirely
/// by `render_date_value`, and is never routed through this function — only
/// a prefix match (no operator), `<>` (not-equal), and `=~` (regex) are safe;
/// `<`, `<=`, `>`, `>=`, `=` all silently compile to a filter that matches
/// nothing (see `tag_is_numeric`'s doc comment for the empirical evidence).
fn ensure_relational_op_safe_for_text_tag(tag: TagName, op: TagOp) -> Result<(), CompileError> {
    if tag_is_numeric(tag) {
        return Ok(());
    }
    match op {
        TagOp::Prefix | TagOp::Regex | TagOp::Ne => Ok(()),
        _ => Err(CompileError::InvalidSpec {
            field: format!("filters.tagRules[].op ({})", tag.as_engine_str()),
            reason: format!(
                "{} is compared as text, not a number: the engine's five relational/equality \
                 operators (<, <=, >, >=, =) silently match nothing against it (empirically \
                 verified against the real engine binary; DECISIONS-LEDGER.md D-010 recorded \
                 the same fact for ECO specifically, but it is a general property of every \
                 non-numeric tag - see `tag_is_numeric`'s doc comment). Use a prefix match (no \
                 operator), <> (not-equal), or =~ (regex) instead.",
                tag.as_engine_str()
            ),
        }),
    }
}

const ALLOWED_RESULT_VALUES: [&str; 4] = ["1-0", "0-1", "1/2-1/2", "*"];

/// Value-shape check only; operator safety is `ensure_relational_op_safe_for_text_tag`'s
/// job and is checked separately (and first) by the caller. By the time this
/// runs, `op` is already known to be `Prefix`, `Ne`, or `Regex`.
fn ensure_result_value_allowed(op: TagOp, value: &str) -> Result<(), CompileError> {
    if op == TagOp::Regex || ALLOWED_RESULT_VALUES.contains(&value) {
        Ok(())
    } else {
        Err(CompileError::InvalidSpec {
            field: "filters.tagRules[].value (Result)".to_string(),
            reason: format!(
                "\"{value}\" is not a valid Result value; use one of 1-0, 0-1, 1/2-1/2, * \
                 (or operator =~ with a regex for anything else)"
            ),
        })
    }
}

fn render_tag_rule_line(rule: &TagRule) -> Result<String, CompileError> {
    ensure_representable("filters.tagRules[].value", &rule.value)?;
    let rendered_value = match rule.tag {
        TagName::Date => render_date_value(rule.op, &rule.value)?,
        TagName::Eco => {
            ensure_relational_op_safe_for_text_tag(rule.tag, rule.op)?;
            rule.value.clone()
        }
        TagName::Result => {
            ensure_relational_op_safe_for_text_tag(rule.tag, rule.op)?;
            ensure_result_value_allowed(rule.op, &rule.value)?;
            rule.value.clone()
        }
        _ => {
            ensure_relational_op_safe_for_text_tag(rule.tag, rule.op)?;
            rule.value.clone()
        }
    };
    let escaped = escape_value(&rendered_value);
    let mut line = String::new();
    line.push_str(rule.tag.as_engine_str());
    if let Some(token) = rule.op.as_engine_token() {
        line.push(' ');
        line.push_str(token);
    }
    line.push_str(" \"");
    line.push_str(&escaped);
    line.push('"');
    Ok(line)
}

/// FEN-pattern charset per design-02 §1.5.1: "non-empty, ≤ 200 bytes,
/// contain no whitespace or `"`; charset `[A-Za-z0-8/?*!-]`" (transcribed
/// verbatim, including the `0-8` — not `0-9` — digit range given there).
fn validate_fen_pattern(pattern: &str) -> Result<(), CompileError> {
    if pattern.is_empty() {
        return Err(CompileError::InvalidSpec {
            field: "filters.fenPattern.pattern".to_string(),
            reason: "must not be empty".to_string(),
        });
    }
    if pattern.len() > 200 {
        return Err(CompileError::InvalidSpec {
            field: "filters.fenPattern.pattern".to_string(),
            reason: format!("must be at most 200 bytes, got {}", pattern.len()),
        });
    }
    let charset_ok = pattern.bytes().all(|b| {
        b.is_ascii_alphabetic()
            || (b'0'..=b'8').contains(&b)
            || matches!(b, b'/' | b'?' | b'*' | b'!' | b'-')
    });
    if !charset_ok {
        return Err(CompileError::InvalidSpec {
            field: "filters.fenPattern.pattern".to_string(),
            reason: "must contain only letters, digits 0-8, and the characters / ? * ! - \
                      (no whitespace or quotes)"
                .to_string(),
        });
    }
    Ok(())
}

fn render_fen_pattern_line(filter: &FenPatternFilter) -> Result<String, CompileError> {
    validate_fen_pattern(&filter.pattern)?;
    // The charset check above already excludes '\' and '"'; escaping is a
    // harmless no-op safety net kept for consistency with tag-rule values.
    let escaped = escape_value(&filter.pattern);
    let name = if filter.both_sides {
        "FENPatternI"
    } else {
        "FENPattern"
    };
    Ok(format!("{name} \"{escaped}\""))
}

/// Renders `criteria/tags.txt`. Returns `Ok(None)` when there is nothing to
/// render (no tag rules and no FEN pattern), signaling the caller to omit
/// `-t` entirely (design-02 §1.5.1: "The file is emitted only when it
/// contains ≥ 1 line; otherwise `-t` is omitted entirely").
pub fn render_tags_file(
    tag_rules: &[TagRule],
    fen_pattern: Option<&FenPatternFilter>,
) -> Result<Option<RenderedCriteriaFile>, CompileError> {
    let mut lines = Vec::with_capacity(tag_rules.len() + 1);
    for rule in tag_rules {
        lines.push(render_tag_rule_line(rule)?);
    }
    if let Some(fen) = fen_pattern {
        lines.push(render_fen_pattern_line(fen)?);
    }
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(finalize(lines)))
}

/// Move-token grammar per design-02 §1.5.2: "printable ASCII `[\x21-\x7E]`,
/// ≤ 15 bytes, none of `" { } % ;`".
fn validate_variation_token(token: &str) -> Result<(), CompileError> {
    if token.len() > 15 {
        return Err(CompileError::InvalidSpec {
            field: "filters.textualVariations[]".to_string(),
            reason: format!("move token \"{token}\" exceeds 15 bytes"),
        });
    }
    let ok = token
        .bytes()
        .all(|b| (0x21..=0x7E).contains(&b) && !matches!(b, b'"' | b'{' | b'}' | b'%' | b';'));
    if !ok {
        return Err(CompileError::InvalidSpec {
            field: "filters.textualVariations[]".to_string(),
            reason: format!(
                "move token \"{token}\" must be printable ASCII and must not contain \" {{ }} % ;"
            ),
        });
    }
    Ok(())
}

/// Tokenizes one variation entry on whitespace (matching the engine's own
/// `strtok(line, " ")`, design-02 §1.5.2) and validates each token. An
/// entry that is empty/all-whitespace renders nothing ("blank lines
/// ignored") rather than an error.
fn render_variation_line(variation: &str) -> Result<Option<String>, CompileError> {
    let tokens: Vec<&str> = variation.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(None);
    }
    for token in &tokens {
        validate_variation_token(token)?;
    }
    Ok(Some(tokens.join(" ")))
}

/// Renders `criteria/variations.txt`. Returns `Ok(None)` when every entry is
/// blank (or the list itself is empty), signaling the caller to omit `-v`.
pub fn render_variations_file(
    variations: &[String],
) -> Result<Option<RenderedCriteriaFile>, CompileError> {
    let mut lines = Vec::with_capacity(variations.len());
    for variation in variations {
        if let Some(line) = render_variation_line(variation)? {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(finalize(lines)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- BOM / LF encoding (task section C: "Test this explicitly") ---

    #[test]
    fn rendered_content_has_no_utf8_bom() {
        let rules = vec![TagRule {
            tag: TagName::Player,
            op: TagOp::Prefix,
            value: "Tal".to_string(),
        }];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert!(
            !rendered.content.starts_with('\u{FEFF}'),
            "content must not start with a UTF-8 BOM: {:?}",
            rendered.content
        );
        assert_eq!(
            rendered.content.as_bytes()[0],
            b'P',
            "first byte must be the literal tag text, not a BOM"
        );
    }

    #[test]
    fn rendered_content_uses_lf_only_no_cr() {
        let rules = vec![
            TagRule {
                tag: TagName::White,
                op: TagOp::Prefix,
                value: "Tal".to_string(),
            },
            TagRule {
                tag: TagName::Black,
                op: TagOp::Prefix,
                value: "Botvinnik".to_string(),
            },
        ];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert!(
            !rendered.content.contains('\r'),
            "content must never contain CR: {:?}",
            rendered.content
        );
        assert!(
            rendered.content.ends_with('\n'),
            "content must end with a trailing LF"
        );
    }

    #[test]
    fn empty_filter_omits_the_file_entirely() {
        assert_eq!(render_tags_file(&[], None).unwrap(), None);
        assert_eq!(render_variations_file(&[]).unwrap(), None);
        assert_eq!(render_variations_file(&["   ".to_string()]).unwrap(), None);
    }

    // --- Escaping ---

    #[test]
    fn escapes_backslash_and_quote() {
        let rules = vec![TagRule {
            tag: TagName::Event,
            op: TagOp::Prefix,
            value: r#"Ci\"ty Open"#.to_string(),
        }];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert_eq!(rendered.content, "Event \"Ci\\\\\\\"ty Open\"\n");
    }

    #[test]
    fn rejects_cr_lf_nul_in_value() {
        for bad in ["a\rb", "a\nb", "a\0b"] {
            let rules = vec![TagRule {
                tag: TagName::Event,
                op: TagOp::Prefix,
                value: bad.to_string(),
            }];
            let err = render_tags_file(&rules, None).unwrap_err();
            assert!(matches!(err, CompileError::InvalidSpec { .. }));
        }
    }

    // --- Date full-date safety rule ---

    #[test]
    fn date_range_renders_full_dates_from_bare_years() {
        let rules = vec![
            TagRule {
                tag: TagName::Date,
                op: TagOp::Ge,
                value: "1960".to_string(),
            },
            TagRule {
                tag: TagName::Date,
                op: TagOp::Le,
                value: "1969".to_string(),
            },
        ];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert_eq!(
            rendered.content,
            "Date >= \"1960.01.01\"\nDate <= \"1969.12.31\"\n"
        );
    }

    #[test]
    fn date_full_value_passes_through_unchanged() {
        let rules = vec![TagRule {
            tag: TagName::Date,
            op: TagOp::Ge,
            value: "1960.06.15".to_string(),
        }];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert_eq!(rendered.content, "Date >= \"1960.06.15\"\n");
    }

    #[test]
    fn date_bare_year_rejected_for_equality() {
        let rules = vec![TagRule {
            tag: TagName::Date,
            op: TagOp::Eq,
            value: "1999".to_string(),
        }];
        let err = render_tags_file(&rules, None).unwrap_err();
        assert!(matches!(err, CompileError::InvalidSpec { .. }));
    }

    #[test]
    fn date_never_silently_truncates_to_a_bare_year() {
        // Regression guard for the exact hazard design-02 calls out: no code
        // path may produce `Date <= "1999"` (would exclude nearly the whole
        // year). Either it is expanded to a full date, or compilation fails.
        let rules = vec![TagRule {
            tag: TagName::Date,
            op: TagOp::Le,
            value: "1999".to_string(),
        }];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert!(!rendered.content.contains("\"1999\""));
        assert_eq!(rendered.content, "Date <= \"1999.12.31\"\n");
    }

    // --- ECO never-matches guard ---

    #[test]
    fn eco_numeric_relational_operators_are_rejected() {
        // Empirically verified (DECISIONS-LEDGER.md D-010): =, >, >=, <, <=
        // all match nothing against ECO. <> is deliberately excluded from
        // this list - see `eco_not_equal_is_allowed` below.
        for op in [TagOp::Lt, TagOp::Le, TagOp::Gt, TagOp::Ge, TagOp::Eq] {
            let rules = vec![TagRule {
                tag: TagName::Eco,
                op,
                value: "B10".to_string(),
            }];
            let err = render_tags_file(&rules, None).unwrap_err();
            assert!(
                matches!(err, CompileError::InvalidSpec { .. }),
                "op {op:?} should be rejected for ECO"
            );
        }
    }

    #[test]
    fn eco_prefix_and_regex_are_allowed() {
        let rules = vec![
            TagRule {
                tag: TagName::Eco,
                op: TagOp::Prefix,
                value: "B1".to_string(),
            },
            TagRule {
                tag: TagName::Eco,
                op: TagOp::Regex,
                value: "^B1".to_string(),
            },
        ];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert_eq!(rendered.content, "ECO \"B1\"\nECO =~ \"^B1\"\n");
    }

    #[test]
    fn eco_not_equal_is_allowed() {
        // DECISIONS-LEDGER.md D-010: unlike the other five relational
        // operators, `<>` (not-equal) is empirically verified to WORK
        // against ECO (`ECO <> "B10"` matched B90/A00 in the fixture). This
        // is a correction to Phase 1a, which rejected all six operators.
        let rules = vec![TagRule {
            tag: TagName::Eco,
            op: TagOp::Ne,
            value: "B10".to_string(),
        }];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert_eq!(rendered.content, "ECO <> \"B10\"\n");
    }

    #[test]
    fn eco_range_enumerates_explicit_codes_not_a_relational_range() {
        // Design-02 §1.5.1: "B10-B47 -> emit explicit codes B10,B11,...,B47,
        // one line each". Expanding a *range* into that enumeration is a
        // UI-layer job (it produces many TagRule entries); the compiler's
        // job, verified here, is simply that each resulting prefix line
        // renders safely and no relational operator ever sneaks in.
        let codes = ["B10", "B11", "B12"];
        let rules: Vec<TagRule> = codes
            .iter()
            .map(|c| TagRule {
                tag: TagName::Eco,
                op: TagOp::Prefix,
                value: c.to_string(),
            })
            .collect();
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert_eq!(rendered.content, "ECO \"B10\"\nECO \"B11\"\nECO \"B12\"\n");
    }

    // --- Result value restriction, and the generalized text-tag operator guard ---

    #[test]
    fn result_rejects_nonsense_values() {
        // Uses Prefix (the only operator that ever reaches the value check
        // in practice) so this test isolates the *value*-legality check from
        // the separate *operator*-legality check exercised below.
        let rules = vec![TagRule {
            tag: TagName::Result,
            op: TagOp::Prefix,
            value: "banana".to_string(),
        }];
        let err = render_tags_file(&rules, None).unwrap_err();
        assert!(matches!(err, CompileError::InvalidSpec { .. }));
    }

    /// **Correction, with evidence (Phase 5 task).** This test used to
    /// assert that decisive results compile to `Result = "1-0"` /
    /// `Result = "0-1"` (`TagOp::Eq`). Fresh empirical testing against the
    /// real pinned engine proved that claim false: `Result = "1-0"` matches
    /// **zero** games even when three of the six games in a purpose-built
    /// fixture genuinely have `Result "1-0"` — see `tag_is_numeric`'s doc
    /// comment for the exact commands/counts. `Result` hits the same
    /// "numeric gate" as `ECO` (DECISIONS-LEDGER.md D-010); no test had
    /// exercised it empirically before now because a pure-Rust unit test of
    /// this renderer cannot catch this class of bug — the renderer
    /// faithfully stringifies `TagOp::Eq` to `"="`, and the bug only exists
    /// in the *engine's* interpretation of that string, which only a
    /// real-engine integration test (see `phase5_filters_integration.rs`)
    /// can observe. The old assertion is corrected here to the no-op
    /// (prefix) form, which integration testing proved gives the correct
    /// 3/6 and 4/6 match counts, and which is exactly equivalent to equality
    /// for these four mutually non-prefixing literal values (`1-0`, `0-1`,
    /// `1/2-1/2`, `*` — none is a textual prefix of another). This is also
    /// what `src/state/filterMapping.ts` now emits (it used to emit `"eq"`,
    /// which shipped with the same bug: every Result checkbox on the
    /// Filters screen — White wins/Black wins/Draw/Other/Decisive-only —
    /// compiled to a filter that silently matched zero games, always).
    #[test]
    fn result_decisive_is_two_ored_lines() {
        let rules = vec![
            TagRule {
                tag: TagName::Result,
                op: TagOp::Prefix,
                value: "1-0".to_string(),
            },
            TagRule {
                tag: TagName::Result,
                op: TagOp::Prefix,
                value: "0-1".to_string(),
            },
        ];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert_eq!(rendered.content, "Result \"1-0\"\nResult \"0-1\"\n");
    }

    #[test]
    fn result_numeric_relational_operators_are_rejected() {
        // Generalizes `eco_numeric_relational_operators_are_rejected` to
        // Result — empirically verified: `Result = "1-0"` matches nothing
        // even for a fixture where it is textually true for 3/6 games (see
        // `tag_is_numeric`'s doc comment).
        for op in [TagOp::Lt, TagOp::Le, TagOp::Gt, TagOp::Ge, TagOp::Eq] {
            let rules = vec![TagRule {
                tag: TagName::Result,
                op,
                value: "1-0".to_string(),
            }];
            let err = render_tags_file(&rules, None).unwrap_err();
            assert!(
                matches!(err, CompileError::InvalidSpec { .. }),
                "op {op:?} should be rejected for Result"
            );
        }
    }

    #[test]
    fn result_not_equal_is_allowed() {
        // Empirically verified: `Result <> "1-0"` correctly matched the 3
        // non-"1-0" games out of 6 in the fixture.
        let rules = vec![TagRule {
            tag: TagName::Result,
            op: TagOp::Ne,
            value: "1-0".to_string(),
        }];
        let rendered = render_tags_file(&rules, None).unwrap().unwrap();
        assert_eq!(rendered.content, "Result <> \"1-0\"\n");
    }

    // --- Generalized text-tag operator guard (beyond ECO/Result) ---

    /// The task's core new finding: the "numeric gate" is not ECO-specific
    /// (DECISIONS-LEDGER.md D-010's own framing) — it is a general property
    /// of every non-numeric tag. Empirically reconfirmed directly against
    /// the real engine for `White` and `Site` (see `tag_is_numeric`'s doc
    /// comment for the exact commands/counts, including the striking
    /// `Site = "Fixture Lab"` case: matches ZERO games even though every
    /// single game in the fixture has that exact Site value). This test
    /// pins the compiler-level guard for the tags most likely to be
    /// (mis)used this way: `White`, `Black`, `Player`, `Event`.
    #[test]
    fn text_tags_reject_numeric_relational_operators() {
        for tag in [
            TagName::White,
            TagName::Black,
            TagName::Player,
            TagName::Event,
        ] {
            for op in [TagOp::Lt, TagOp::Le, TagOp::Gt, TagOp::Ge, TagOp::Eq] {
                let rules = vec![TagRule {
                    tag,
                    op,
                    value: "Somebody".to_string(),
                }];
                let err = render_tags_file(&rules, None).unwrap_err();
                assert!(
                    matches!(err, CompileError::InvalidSpec { .. }),
                    "tag {tag:?} op {op:?} should be rejected (text tag, numeric gate)"
                );
            }
        }
    }

    #[test]
    fn text_tags_allow_prefix_not_equal_and_regex() {
        for tag in [TagName::White, TagName::Black, TagName::Player] {
            for op in [TagOp::Prefix, TagOp::Ne, TagOp::Regex] {
                let rules = vec![TagRule {
                    tag,
                    op,
                    value: "Somebody".to_string(),
                }];
                assert!(
                    render_tags_file(&rules, None).is_ok(),
                    "tag {tag:?} op {op:?} should be allowed"
                );
            }
        }
    }

    /// The boundary of the new guard: the four numeric (Elo-family) tags
    /// must stay fully permissive of every operator, since they take the
    /// engine's *numeric* comparison path — empirically confirmed working
    /// (`WhiteElo = "2500"`, `WhiteElo >= "2600"` both matched correctly).
    #[test]
    fn elo_family_tags_permit_every_relational_operator() {
        for tag in [
            TagName::WhiteElo,
            TagName::BlackElo,
            TagName::Elo,
            TagName::EloDiff,
        ] {
            for op in [
                TagOp::Prefix,
                TagOp::Lt,
                TagOp::Le,
                TagOp::Ne,
                TagOp::Gt,
                TagOp::Ge,
                TagOp::Eq,
                TagOp::Regex,
            ] {
                let rules = vec![TagRule {
                    tag,
                    op,
                    value: "2000".to_string(),
                }];
                assert!(
                    render_tags_file(&rules, None).is_ok(),
                    "numeric tag {tag:?} op {op:?} must remain permitted"
                );
            }
        }
    }

    // --- FEN pattern ---

    #[test]
    fn fen_pattern_both_sides_uses_i_suffix() {
        let f = FenPatternFilter {
            pattern: "8/8/8/8/8/8/8/8".to_string(),
            both_sides: true,
        };
        let rendered = render_tags_file(&[], Some(&f)).unwrap().unwrap();
        assert_eq!(rendered.content, "FENPatternI \"8/8/8/8/8/8/8/8\"\n");
    }

    #[test]
    fn fen_pattern_single_side_has_no_suffix() {
        let f = FenPatternFilter {
            pattern: "r?bqkb?r".to_string(),
            both_sides: false,
        };
        let rendered = render_tags_file(&[], Some(&f)).unwrap().unwrap();
        assert_eq!(rendered.content, "FENPattern \"r?bqkb?r\"\n");
    }

    #[test]
    fn fen_pattern_rejects_bad_charset() {
        let f = FenPatternFilter {
            pattern: "has space".to_string(),
            both_sides: false,
        };
        let err = render_tags_file(&[], Some(&f)).unwrap_err();
        assert!(matches!(err, CompileError::InvalidSpec { .. }));

        let f9 = FenPatternFilter {
            pattern: "9".to_string(),
            both_sides: false,
        };
        let err9 = render_tags_file(&[], Some(&f9)).unwrap_err();
        assert!(
            matches!(err9, CompileError::InvalidSpec { .. }),
            "digit 9 is outside the verified 0-8 charset"
        );
    }

    #[test]
    fn fen_pattern_rejects_empty_and_oversize() {
        let empty = FenPatternFilter {
            pattern: String::new(),
            both_sides: false,
        };
        assert!(render_tags_file(&[], Some(&empty)).is_err());

        let oversize = FenPatternFilter {
            pattern: "a".repeat(201),
            both_sides: false,
        };
        assert!(render_tags_file(&[], Some(&oversize)).is_err());
    }

    // --- Textual variations ---

    #[test]
    fn variations_render_one_line_each_normalized_whitespace() {
        let variations = vec![
            "c4  Nf6   Nc3 e6 d4 Bb4".to_string(),
            "f3 e5 g4 Qh4 0-1".to_string(),
        ];
        let rendered = render_variations_file(&variations).unwrap().unwrap();
        assert_eq!(rendered.content, "c4 Nf6 Nc3 e6 d4 Bb4\nf3 e5 g4 Qh4 0-1\n");
    }

    #[test]
    fn variations_reject_forbidden_characters_and_oversize_tokens() {
        assert!(render_variations_file(&["e4 e5 % comment".to_string()]).is_err());
        assert!(render_variations_file(&["e4 \"quoted\"".to_string()]).is_err());
        assert!(render_variations_file(&["a".repeat(16)]).is_err());
    }

    #[test]
    fn blank_variation_entries_are_ignored_not_errors() {
        let variations = vec!["".to_string(), "   ".to_string(), "e4 e5".to_string()];
        let rendered = render_variations_file(&variations).unwrap().unwrap();
        assert_eq!(rendered.content, "e4 e5\n");
    }

    // --- Deterministic sha256 ---

    #[test]
    fn sha256_is_deterministic_and_content_dependent() {
        let rules_a = vec![TagRule {
            tag: TagName::Player,
            op: TagOp::Prefix,
            value: "Tal".to_string(),
        }];
        let rules_b = vec![TagRule {
            tag: TagName::Player,
            op: TagOp::Prefix,
            value: "Fischer".to_string(),
        }];
        let a1 = render_tags_file(&rules_a, None).unwrap().unwrap();
        let a2 = render_tags_file(&rules_a, None).unwrap().unwrap();
        let b = render_tags_file(&rules_b, None).unwrap().unwrap();
        assert_eq!(a1.sha256, a2.sha256);
        assert_ne!(a1.sha256, b.sha256);
        assert_eq!(a1.sha256.len(), 64);
    }
}
