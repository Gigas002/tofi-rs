//! Scored string collections.
#![deny(unsafe_code)]
//!
//! # Design
//!
//! Two complementary collections:
//! - [`StringVec`] — `Vec<ScoredString>` (owned `String`s, NFC-normalized on insert).
//! - [`StringRefVec`] — `Vec<ScoredStringRef<'a>>` (borrowed `&'a str`s, no copy).
//!
//! Both carry per-entry `search_score` and `history_score` (`i32`) used for
//! ranking results. Sorting is deterministic: alphabetical by default; by
//! combined score after a filter pass; by history score after history is applied.
//!
//! ## Decoupling
//!
//! - [`StringRefVec::filter`] takes a `Fn(&str) -> Option<i32>` predicate.
//! - [`StringRefVec::apply_history_scores`] takes a `&HashMap<&str, i32>` so
//!   callers build the map from any source.

use std::collections::HashMap;

use crate::unicode::utf8_normalize;

// ── Scored entry types ────────────────────────────────────────────────────────

/// An owned, NFC-normalized string with search and history scores.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredString {
    pub string: String,
    pub search_score: i32,
    pub history_score: i32,
}

/// A borrowed string reference with search and history scores.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredStringRef<'a> {
    pub string: &'a str,
    pub search_score: i32,
    pub history_score: i32,
}

// ── StringVec (owned) ─────────────────────────────────────────────────────────

/// A growable list of owned, NFC-normalized strings with per-entry scores.
#[derive(Debug, Default, Clone)]
pub struct StringVec {
    entries: Vec<ScoredString>,
}

impl StringVec {
    /// Create an empty `StringVec`.
    ///
    /// Pre-allocates capacity to match C's initial size of 128.
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(128),
        }
    }

    /// Add a string, NFC-normalizing it first.
    ///
    /// Strings that fail UTF-8 validation are silently ignored — in Rust, `&str`
    /// is always valid UTF-8, so the guard is preserved for callers passing
    /// bytes-turned-string via FFI paths. No-ops on empty strings.
    pub fn add(&mut self, s: &str) {
        self.entries.push(ScoredString {
            string: utf8_normalize(s),
            search_score: 0,
            history_score: 0,
        });
    }

    /// Sort entries alphabetically by string value (ascending).
    ///
    /// Stable so equal strings retain insertion order.
    pub fn sort_alpha(&mut self) {
        self.entries.sort_by(|a, b| a.string.cmp(&b.string));
    }

    /// Remove consecutive duplicate strings, preserving the first occurrence.
    ///
    /// Sorts alphabetically first.
    pub fn dedup(&mut self) {
        self.sort_alpha();
        self.entries.dedup_by(|a, b| a.string == b.string);
    }

    /// Binary-search for `s` in an **already sorted** vec, returning an index.
    pub fn find_sorted(&self, s: &str) -> Option<usize> {
        self.entries
            .binary_search_by(|e| e.string.as_str().cmp(s))
            .ok()
    }

    /// Returns a slice over all entries.
    pub fn entries(&self) -> &[ScoredString] {
        &self.entries
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── StringRefVec (borrowed) ───────────────────────────────────────────────────

/// A growable list of borrowed string references with per-entry scores.
#[derive(Debug, Default, Clone)]
pub struct StringRefVec<'a> {
    entries: Vec<ScoredStringRef<'a>>,
}

impl<'a> StringRefVec<'a> {
    /// Create an empty `StringRefVec`.
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(128),
        }
    }

    /// Add a string reference without copying.
    pub fn add(&mut self, s: &'a str) {
        self.entries.push(ScoredStringRef {
            string: s,
            search_score: 0,
            history_score: 0,
        });
    }

    /// Build a `StringRefVec` by splitting `buffer` on newline characters.
    ///
    /// Empty lines are skipped.
    pub fn from_lines(buffer: &'a str) -> Self {
        let mut vec = Self::new();
        for line in buffer.split('\n') {
            if !line.is_empty() {
                vec.add(line);
            }
        }
        vec
    }

    /// Shallow-clone the vec (entries share the same string references).
    pub fn clone_shallow(&self) -> Self {
        self.clone()
    }

    /// Apply history scores from `scores` (name → run_count) and sort by
    /// history score descending.
    ///
    /// Uses a `HashMap` for O(N+M) lookup. Entries not present in `scores`
    /// keep a score of 0.
    pub fn apply_history_scores(&mut self, scores: &HashMap<&str, i32>) {
        for entry in &mut self.entries {
            if let Some(&score) = scores.get(entry.string) {
                entry.history_score = score;
            }
        }
        self.entries
            .sort_by_key(|e| std::cmp::Reverse(e.history_score));
    }

    /// Filter entries using a scoring predicate and return a new `StringRefVec`
    /// sorted by combined score (search + history) descending.
    ///
    /// `predicate(s)` should return `Some(search_score)` when `s` matches, or
    /// `None` to exclude it.
    ///
    /// When `query` is empty the full vec is returned unchanged.
    pub fn filter(&self, query: &str, predicate: impl Fn(&str) -> Option<i32>) -> StringRefVec<'a> {
        if query.is_empty() {
            return self.clone_shallow();
        }

        let mut result = StringRefVec::new();
        for entry in &self.entries {
            if let Some(score) = predicate(entry.string) {
                result.entries.push(ScoredStringRef {
                    string: entry.string,
                    search_score: score,
                    history_score: entry.history_score,
                });
            }
        }
        result.entries.sort_by(|a, b| {
            let score_b = b.history_score + b.search_score;
            let score_a = a.history_score + a.search_score;
            score_b.cmp(&score_a)
        });
        result
    }

    /// Binary-search for `s` in an **already sorted** vec, returning an index.
    pub fn find_sorted(&self, s: &str) -> Option<usize> {
        self.entries.binary_search_by(|e| e.string.cmp(s)).ok()
    }

    /// Returns a slice over all entries.
    pub fn entries(&self) -> &[ScoredStringRef<'a>] {
        &self.entries
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests;
