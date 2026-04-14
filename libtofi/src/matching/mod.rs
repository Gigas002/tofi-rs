//! Matching algorithms — Rust port of `src/matching.c`.
#![deny(unsafe_code)]
//!
//! # Algorithms
//!
//! | Variant | C function | Behaviour |
//! |---|---|---|
//! | [`Normal`] | `simple_match_words` | Case-insensitive substring; all words must appear |
//! | [`Prefix`] | `prefix_match_words` | Each word must be a prefix of `str` |
//! | [`Fuzzy`]  | `fuzzy_match_words`  | fts_fuzzy_match v0.2.0–style recursive scoring |
//!
//! All algorithms:
//! - Split the query on whitespace (NFC-normalised first).
//! - Return `i32::MIN` if any word fails to match.
//! - Return a larger score for a better match (suitable for descending sort).
//!
//! # Integration with `string_table`
//!
//! ```rust,ignore
//! use libtofi_rs::matching::{MatchingAlgorithm, match_words};
//!
//! let results = candidates.filter(query, |s| {
//!     let score = match_words(MatchingAlgorithm::Fuzzy, query, s);
//!     if score == i32::MIN { None } else { Some(score) }
//! });
//! ```
//!
//! [`Normal`]: MatchingAlgorithm::Normal
//! [`Prefix`]: MatchingAlgorithm::Prefix
//! [`Fuzzy`]: MatchingAlgorithm::Fuzzy

use crate::unicode::{
    utf8_normalize, utf8_strcasestr, utf8_strlen, utf32_isalnum, utf32_islower, utf32_isupper,
};

/// The matching algorithm to use when filtering candidates.
///
/// Mirrors `enum matching_algorithm` from `src/matching.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchingAlgorithm {
    /// Case-insensitive substring match; each query word must appear anywhere
    /// in the candidate. Score = negative sum of byte offsets from string start
    /// (earlier matches score higher).
    #[default]
    Normal,

    /// Each query word must appear as a **prefix** of the candidate.
    /// Score = negative sum of unmatched suffix lengths.
    Prefix,

    /// Fuzzy matching (fts_fuzzy_match v0.2.0, public domain).
    /// All query characters must appear in order; score rewards adjacency,
    /// separator and CamelCase transitions.
    Fuzzy,
}

/// Dispatch to the appropriate algorithm and return its score.
///
/// Returns `i32::MIN` if the candidate does not match.
/// Larger values are better.
///
/// Mirrors `match_words` from `src/matching.c`.
pub fn match_words(algorithm: MatchingAlgorithm, patterns: &str, s: &str) -> i32 {
    match algorithm {
        MatchingAlgorithm::Normal => simple_match_words(patterns, s),
        MatchingAlgorithm::Prefix => prefix_match_words(patterns, s),
        MatchingAlgorithm::Fuzzy => fuzzy_match_words(patterns, s),
    }
}

// ── Normal (simple substring) ─────────────────────────────────────────────────

/// Returns the negative sum of byte offsets at which each query word appears
/// in `s` (all words must be found), or `i32::MIN` on any miss.
///
/// Mirrors `simple_match_words`.
fn simple_match_words(patterns: &str, s: &str) -> i32 {
    let normalised = utf8_normalize(patterns);
    let mut score: i32 = 0;
    for word in normalised.split_ascii_whitespace() {
        match utf8_strcasestr(s, word) {
            Some(offset) => {
                score = score.saturating_sub(offset as i32);
            }
            None => return i32::MIN,
        }
    }
    score
}

// ── Prefix ────────────────────────────────────────────────────────────────────

/// Each query word must appear at position 0 of `s`; score = negative sum of
/// unmatched suffix codepoint counts, or `i32::MIN` on any miss.
///
/// Mirrors `prefix_match_words`.
fn prefix_match_words(patterns: &str, s: &str) -> i32 {
    let normalised = utf8_normalize(patterns);
    let mut score: i32 = 0;
    for word in normalised.split_ascii_whitespace() {
        match utf8_strcasestr(s, word) {
            Some(0) => {
                let penalty = (utf8_strlen(s) as i32) - (utf8_strlen(word) as i32);
                score = score.saturating_sub(penalty);
            }
            _ => return i32::MIN,
        }
    }
    score
}

// ── Fuzzy ─────────────────────────────────────────────────────────────────────

/// Sums `fuzzy_match` over each query word; returns `i32::MIN` if any fails.
///
/// Mirrors `fuzzy_match_words`.
fn fuzzy_match_words(patterns: &str, s: &str) -> i32 {
    let normalised = utf8_normalize(patterns);
    let mut total: i32 = 0;
    for word in normalised.split_ascii_whitespace() {
        let word_score = fuzzy_match(word, s);
        if word_score == i32::MIN {
            return i32::MIN;
        }
        total = total.saturating_add(word_score);
    }
    total
}

/// Fuzzy-match a single `pattern` word against `s`.
///
/// Mirrors `fuzzy_match` (fts_fuzzy_match v0.2.0, public domain).
fn fuzzy_match(pattern: &str, s: &str) -> i32 {
    const UNMATCHED_LETTER_PENALTY: i32 = -1;

    if pattern.is_empty() {
        return 0;
    }

    let slen = utf8_strlen(s);
    let plen = utf8_strlen(pattern);

    if slen < plen {
        return i32::MIN;
    }

    // Penalise all unmatched characters upfront.
    let base_score = UNMATCHED_LETTER_PENALTY * (slen - plen) as i32;

    // For long strings, only find the first match (not the best) to avoid
    // combinatorial explosion — matches C's `first_match_only = slen > 100`.
    let first_match_only = slen > 100;

    fuzzy_match_recurse(pattern, s, None, base_score, first_match_only, true)
}

/// Recursively match `pattern` against `haystack`, accumulating scores.
///
/// - `prev_char`: the character immediately before the start of `haystack`
///   in the original string (needed for separator/CamelCase scoring).
/// - `score`: the score earned by the character match that led here.
/// - `first_char`: true for the first character of the pattern.
///
/// Mirrors `fuzzy_match_recurse`.
fn fuzzy_match_recurse(
    pattern: &str,
    haystack: &str,
    prev_char: Option<char>,
    score: i32,
    first_match_only: bool,
    first_char: bool,
) -> i32 {
    // Base case: entire pattern consumed — this branch matched.
    let Some(pattern_first) = pattern.chars().next() else {
        return score;
    };
    let pattern_rest = &pattern[pattern_first.len_utf8()..];

    let mut best_score = i32::MIN;
    let mut prev = prev_char;

    for (jump, (byte_idx, ch)) in (0_i32..).zip(haystack.char_indices()) {
        // Case-insensitive codepoint comparison (mirrors `utf8_strcasechr`).
        let ch_lower = ch.to_lowercase().next().unwrap_or(ch);
        let pf_lower = pattern_first.to_lowercase().next().unwrap_or(pattern_first);

        if ch_lower == pf_lower {
            let char_score = compute_score(jump, first_char, ch, prev);
            let haystack_after = &haystack[byte_idx + ch.len_utf8()..];
            let subscore = fuzzy_match_recurse(
                pattern_rest,
                haystack_after,
                Some(ch),
                char_score,
                first_match_only,
                false,
            );
            if subscore != i32::MIN {
                best_score = best_score.max(subscore);
            }
            if first_match_only {
                break;
            }
        }

        prev = Some(ch);
    }

    if best_score == i32::MIN {
        i32::MIN
    } else {
        score.saturating_add(best_score)
    }
}

/// Score a single character match within a fuzzy match pass.
///
/// Scoring system from fts_fuzzy_match v0.2.0 (public domain, Forrest Smith).
/// Mirrors `compute_score`.
fn compute_score(jump: i32, first_char: bool, cur: char, prev: Option<char>) -> i32 {
    const ADJACENCY_BONUS: i32 = 15;
    const SEPARATOR_BONUS: i32 = 30;
    const CAMEL_BONUS: i32 = 30;
    const FIRST_LETTER_BONUS: i32 = 15;
    const LEADING_LETTER_PENALTY: i32 = -5;
    const MAX_LEADING_LETTER_PENALTY: i32 = -15;

    let mut score: i32 = 0;

    // Consecutive match bonus.
    if !first_char && jump == 0 {
        score += ADJACENCY_BONUS;
    }

    // CamelCase / separator bonuses (requires knowing the previous char).
    if let Some(p) = prev.filter(|_| !first_char || jump > 0) {
        if utf32_isupper(cur) && utf32_islower(p) {
            score += CAMEL_BONUS;
        }
        if utf32_isalnum(cur) && !utf32_isalnum(p) {
            score += SEPARATOR_BONUS;
        }
    }

    // Match at the very start of the string.
    if first_char && jump == 0 {
        score += FIRST_LETTER_BONUS;
    }

    // Penalty for skipped leading characters.
    if first_char {
        score += (LEADING_LETTER_PENALTY * jump).max(MAX_LEADING_LETTER_PENALTY);
    }

    score
}

#[cfg(test)]
mod tests;
