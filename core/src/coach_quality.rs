//! Pro Coach sentence-quality guardrails (Draft Brain 2.0).
//!
//! Pure helpers operating on primitives (`&str` / `&[String]`) so they are fully
//! decoupled from `Recommendation` — the sentence builders in `draft_brain.rs` and
//! `engine.rs` (Codex-owned) can adopt them without coupling to the struct shape.
//!
//! Purpose: enforce the project rule "no false certainty / no exaggerated
//! language", keep coaching strings non-empty + meaningful, and de-duplicate
//! lists like `why_not`. Lives in its own module so it never conflicts with the
//! files that produce the sentences; wiring is a one-line call the builders add.
#![allow(dead_code)] // adopted by the sentence builders in a follow-up (see handoff report)

use serde::Serialize;
use std::collections::HashSet;

/// Over-promising phrases that violate the "no guaranteed outcome" rule. These
/// are multi-word / unambiguous tokens to avoid false positives on legitimate
/// coaching (e.g. "kazanma koşulu", "garanti yok" must NOT trip).
const ABSOLUTE_PHRASES: &[&str] = &[
    "kesin kazan",
    "kesinlikle kazan",
    "mutlaka kazan",
    "her zaman kazan",
    "her maçı kazan",
    "kesin galibiyet",
    "garanti kazan",
    "garantili zafer",
    "garantili win",
    "garantili kazanç",
    "asla kaybet",
    "rakip hiçbir şey yapamaz",
    "rakip oynayamaz",
    "kaybetmen imkansız",
    "kesin üstünlük",
    "free win",
    "bedava win",
    "kesin carry",
    "%100",
    "100% win",
    "kesin win",
    "guaranteed",
];

/// Runaway-concatenation guard for the decision sentence (very generous — only
/// catches a builder bug that dumps every field into one line, not tight style).
const MAX_DECISION_WORDS: usize = 60;
/// A present lane/teamfight plan must be more than a bare label.
const MIN_PLAN_WORDS: usize = 2;

/// True when the text over-promises a guaranteed result.
pub fn has_absolute_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    let normalized = canonical_for_matching(text);
    ABSOLUTE_PHRASES.iter().any(|p| {
        lower.contains(&p.to_lowercase()) || normalized.contains(&canonical_for_matching(p))
    })
}

/// A coaching string is meaningful when it has real content — not blank, not a
/// bare label — i.e. at least `min_words` whitespace-separated words.
pub fn is_meaningful(text: &str, min_words: usize) -> bool {
    let t = text.trim();
    !t.is_empty() && t.split_whitespace().count() >= min_words
}

/// Case-insensitive dedup preserving first-seen order (for `why_not`, tips, …).
pub fn dedup_sentences(items: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|s| seen.insert(canonical_sentence(s)))
        .cloned()
        .collect()
}

fn canonical_sentence(text: &str) -> String {
    canonical_for_matching(text.trim().trim_end_matches(&['.', '!', '?'][..]).trim())
}

fn canonical_for_matching(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || c == '%' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// One coaching-quality issue, for QA / regression tests. `String` payload names
/// the field where the issue was found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "field")]
pub enum CoachIssue {
    Empty(String),
    AbsoluteLanguage(String),
    Duplicate(String),
    /// Decision sentence is a runaway concatenation (builder bug).
    TooLong(String),
    /// A plan field is present but too short to be useful (bare label).
    TooShort(String),
}

/// Lint the core coaching fields of one recommendation. Pure — the caller passes
/// the already-built strings (e.g. `rec.decision_sentence`, `rec.lane_plan`).
/// Returns an empty vec when the coaching reads clean.
pub fn audit_coaching(
    decision_sentence: &str,
    lane_plan: Option<&str>,
    teamfight: Option<&str>,
    why_not: &[String],
) -> Vec<CoachIssue> {
    let mut issues = Vec::new();

    if !is_meaningful(decision_sentence, 3) {
        issues.push(CoachIssue::Empty("decision_sentence".to_string()));
    } else if has_absolute_language(decision_sentence) {
        issues.push(CoachIssue::AbsoluteLanguage(
            "decision_sentence".to_string(),
        ));
    } else if decision_sentence.split_whitespace().count() > MAX_DECISION_WORDS {
        issues.push(CoachIssue::TooLong("decision_sentence".to_string()));
    }

    // A present plan must not over-promise nor be a bare one-word label.
    for (label, opt) in [("lane_plan", lane_plan), ("teamfight", teamfight)] {
        if let Some(text) = opt {
            if has_absolute_language(text) {
                issues.push(CoachIssue::AbsoluteLanguage(label.to_string()));
            } else if !is_meaningful(text, MIN_PLAN_WORDS) {
                issues.push(CoachIssue::TooShort(label.to_string()));
            }
        }
    }

    if why_not.len() != dedup_sentences(why_not).len() {
        issues.push(CoachIssue::Duplicate("why_not".to_string()));
    }
    if why_not.iter().any(|w| has_absolute_language(w)) {
        issues.push(CoachIssue::AbsoluteLanguage("why_not".to_string()));
    }
    // Each "why not X" line must carry a real reason, not a bare label.
    if why_not.iter().any(|w| !is_meaningful(w, MIN_PLAN_WORDS)) {
        issues.push(CoachIssue::TooShort("why_not".to_string()));
    }

    issues
}

/// Release-safe quality tripwire: runs [`audit_coaching`] and, on any issue, logs a
/// warning naming the champion + issues. It NEVER drops or rewrites the
/// recommendation — correct-by-construction coaching is the contract; this catches
/// builder regressions in production builds (debug builds additionally hard-assert
/// via the caller). Returns the issue count (0 = clean).
pub fn audit_soft(
    decision_sentence: &str,
    lane_plan: Option<&str>,
    teamfight: Option<&str>,
    why_not: &[String],
    champion_name: &str,
) -> usize {
    let issues = audit_coaching(decision_sentence, lane_plan, teamfight, why_not);
    if !issues.is_empty() {
        tracing::warn!(
            "coach_quality: {champion_name} coaching text failed audit ({} issue(s)): {issues:?}",
            issues.len()
        );
    }
    issues.len()
}
